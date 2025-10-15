//! Dynamic API for runtime polymorphism of camera operations.
//!
//! This module provides trait object compatibility for VISCA camera operations,
//! enabling runtime polymorphism without requiring downstream projects to implement
//! large wrapper traits.
//!
//! # Feature Gate
//!
//! This module is only available when the `dyn-api` feature is enabled:
//!
//! ```toml
//! [dependencies]
//! grafton-visca = { version = "*", features = ["dyn-api"] }
//! ```
//!
//! # Overview
//!
//! The dynamic API provides object-safe trait interfaces that allow cameras with
//! different profiles and transports to be used through a single `dyn` trait type.
//! This is useful for:
//!
//! - Building APIs that accept any camera type at runtime
//! - Storing heterogeneous collections of cameras
//! - Implementing plugin systems or dynamic camera selection
//! - Avoiding large amounts of monomorphization in generic code
//!
//! # Architecture
//!
//! The API is organized around capability-based traits:
//!
//! - **`CameraControl`**: Core trait providing basic operations and capability discovery
//! - **Capability traits**: Domain-specific traits for pan/tilt, zoom, focus, etc.
//! - **Dynamic constructors**: Functions like `open_udp_dynamic` that return trait objects
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! # #[cfg(feature = "dyn-api")] {
//! use grafton_visca::dynapi::{CameraControl, open_udp_dynamic};
//! use grafton_visca::camera::profiles::ProfileId;
//! use grafton_visca::runtime::TokioRuntime;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), grafton_visca::Error> {
//! let runtime = TokioRuntime;
//! let camera: Arc<dyn CameraControl> =
//!     open_udp_dynamic(ProfileId::SonyFr7, "192.168.1.50:52381", runtime).await?;
//!
//! camera.power_on().await?;
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Capability-Based API
//!
//! ```rust,no_run
//! # #[cfg(feature = "dyn-api")] {
//! # use grafton_visca::dynapi::CameraControl;
//! # use grafton_visca::types::{PanSpeed, TiltSpeed};
//! # use std::sync::Arc;
//! # async fn example(camera: Arc<dyn CameraControl>) -> Result<(), grafton_visca::Error> {
//! // Check if camera supports pan/tilt operations
//! if let Some(pan_tilt) = camera.as_pan_tilt() {
//!     pan_tilt.pan_tilt_home().await?;
//! }
//!
//! // Check if camera supports zoom operations
//! if let Some(zoom) = camera.as_zoom() {
//!     let pos = grafton_visca::types::ZoomPosition::from_raw(512);
//!     zoom.zoom_absolute(pos).await?;
//! }
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! ## Profile-Based Construction
//!
//! ```rust,no_run
//! # #[cfg(feature = "dyn-api")] {
//! # use grafton_visca::dynapi::open_udp_dynamic;
//! # use grafton_visca::camera::profiles::{ProfileId, ProfileGroup};
//! # use grafton_visca::runtime::TokioRuntime;
//! # async fn example() -> Result<(), grafton_visca::Error> {
//! let profile = ProfileId::PtzOpticsG2;
//! let group = profile.profile_group();
//!
//! // Construct camera based on profile group
//! match group {
//!     ProfileGroup::GenericVisca |
//!     ProfileGroup::PtzOpticsG2 |
//!     ProfileGroup::SonyProfessional => {
//!         let camera = open_udp_dynamic(profile, "192.168.1.50:5678", TokioRuntime).await?;
//!         // Use camera...
//!     }
//! }
//! # Ok(())
//! # }
//! # }
//! ```

use crate::{
    camera::{convenience::Connect, movement::PanTiltPosition, profiles::ProfileId, CameraSession},
    command::{
        exposure::ExposureMode,
        image::{ImageFlipMode, SharpnessMode},
        nd_filter::NdFilterMode,
        resolution::PictureEffectMode,
        white_balance::{AutoWhiteBalanceSensitivity, WhiteBalanceMode},
    },
    executor::Executor,
    mode::Async,
    profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    transport::AsyncTransport,
    types::{
        GainLevel, IrisLevel, NoiseReduction2DLevel, NoiseReduction3DLevel, SharpnessLevel,
        SpeedLevel, ZoomPosition,
    },
    units::{Degrees, Normalized},
    Error, PresetNumber,
};
use std::{future::Future, pin::Pin, sync::Arc};

/// Type alias for boxed futures used in object-safe trait methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Core camera control trait for dynamic dispatch.
///
/// This trait provides the primary interface for runtime-polymorphic camera operations.
/// It combines basic camera operations with capability discovery through `as_*` methods.
///
/// # Object Safety
///
/// All methods in this trait are object-safe, returning `BoxFuture` instead of generic
/// `impl Future` types. This allows the trait to be used as `dyn CameraControl`.
///
/// # Capabilities
///
/// Rather than providing all possible methods directly, this trait follows a capability-based
/// design. Call the appropriate `as_*` method to get access to specific functionality:
///
/// - [`as_pan_tilt`](Self::as_pan_tilt): Pan/tilt movement operations
/// - [`as_zoom`](Self::as_zoom): Zoom control operations
/// - [`as_focus`](Self::as_focus): Focus control operations
/// - [`as_power`](Self::as_power): Power control operations
///
/// Methods return `None` if the capability is not supported by the camera profile.
pub trait CameraControl: Send + Sync {
    /// Power on the camera.
    ///
    /// Transitions the camera from standby mode to operational mode.
    ///
    /// # Errors
    ///
    /// Returns an error if the command fails to send or the camera does not respond.
    fn power_on(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Power off the camera.
    ///
    /// Transitions the camera to standby mode. The camera will still respond to
    /// power-on commands but stops video output.
    ///
    /// # Errors
    ///
    /// Returns an error if the command fails to send or the camera does not respond.
    fn power_off(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Get pan/tilt control capability.
    ///
    /// Returns a reference to the pan/tilt control interface if supported.
    fn as_pan_tilt(&self) -> Option<&dyn PanTiltControl> {
        None
    }

    /// Get zoom control capability.
    ///
    /// Returns a reference to the zoom control interface if supported.
    fn as_zoom(&self) -> Option<&dyn ZoomControl> {
        None
    }

    /// Get focus control capability.
    ///
    /// Returns a reference to the focus control interface if supported.
    fn as_focus(&self) -> Option<&dyn FocusControl> {
        None
    }

    /// Get power control capability.
    ///
    /// Returns a reference to the power control interface if supported.
    fn as_power(&self) -> Option<&dyn PowerControl> {
        None
    }

    /// Get exposure control capability.
    ///
    /// Returns a reference to the exposure control interface if supported.
    fn as_exposure(&self) -> Option<&dyn ExposureControl> {
        None
    }

    /// Get white balance control capability.
    ///
    /// Returns a reference to the white balance control interface if supported.
    fn as_white_balance(&self) -> Option<&dyn WhiteBalanceControl> {
        None
    }

    /// Get image processing control capability.
    ///
    /// Returns a reference to the image processing control interface if supported.
    fn as_image(&self) -> Option<&dyn ImageControl> {
        None
    }

    /// Get ND filter control capability.
    ///
    /// Returns a reference to the ND filter control interface if supported.
    fn as_nd_filter(&self) -> Option<&dyn NdFilterControl> {
        None
    }

    /// Get preset control capability.
    ///
    /// Returns a reference to the preset control interface if supported.
    fn as_presets(&self) -> Option<&dyn PresetControl> {
        None
    }
}

/// Pan/tilt control operations (object-safe).
///
/// This trait provides object-safe versions of pan/tilt control methods.
/// All operations are available through the `dyn PanTiltControl` trait object.
///
/// **Note**: This trait does not include `_op` variants (like `pan_tilt_home_op`) because
/// the `InFlight` type has generic parameters that make it incompatible with object safety.
/// For fine-grained control over long-running operations, use the generic API directly.
pub trait PanTiltControl: Send + Sync {
    /// Move camera to home position.
    ///
    /// Returns the camera to its default home position (typically centered).
    /// This operation waits for completion before returning.
    fn pan_tilt_home(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Reset pan/tilt mechanism.
    ///
    /// Performs a reset/recalibration of the pan/tilt motors.
    /// This operation waits for completion before returning.
    fn pan_tilt_reset(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Move to absolute pan/tilt position.
    ///
    /// This operation waits for completion before returning.
    ///
    /// # Parameters
    /// - `pan`: Pan position in degrees
    /// - `tilt`: Tilt position in degrees
    /// - `speed`: Movement speed level
    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Get current pan/tilt position.
    fn pan_tilt_position(&self) -> BoxFuture<'_, Result<PanTiltPosition, Error>>;
}

/// Zoom control operations (object-safe).
///
/// This trait provides object-safe versions of zoom control methods.
///
/// **Note**: This trait does not include `_op` variants (like `zoom_absolute_op`) because
/// the `InFlight` type has generic parameters that make it incompatible with object safety.
/// For fine-grained control over long-running operations, use the generic API directly.
pub trait ZoomControl: Send + Sync {
    /// Set zoom to absolute position.
    ///
    /// This operation waits for completion before returning.
    ///
    /// # Parameters
    /// - `position`: Target zoom position (normalized 0.0-1.0)
    fn zoom_absolute(&self, position: Normalized) -> BoxFuture<'_, Result<(), Error>>;

    /// Get current zoom position.
    fn zoom_position(&self) -> BoxFuture<'_, Result<ZoomPosition, Error>>;
}

/// Focus control operations (object-safe).
///
/// This trait provides object-safe versions of focus control methods.
pub trait FocusControl: Send + Sync {
    /// Set focus mode to auto.
    fn focus_auto(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Set focus mode to manual.
    fn focus_manual(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Trigger one-push auto focus.
    fn focus_one_push(&self) -> BoxFuture<'_, Result<(), Error>>;
}

/// Power control operations (object-safe).
///
/// This trait provides object-safe versions of power control methods.
pub trait PowerControl: Send + Sync {
    /// Power on the camera.
    fn power_on(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Power off the camera.
    fn power_off(&self) -> BoxFuture<'_, Result<(), Error>>;
}

/// Exposure control operations (object-safe).
///
/// This trait provides object-safe versions of exposure control methods.
pub trait ExposureControl: Send + Sync {
    /// Set exposure mode.
    fn set_exposure_mode(&self, mode: ExposureMode) -> BoxFuture<'_, Result<(), Error>>;

    /// Set iris (aperture) level.
    fn set_iris(&self, level: IrisLevel) -> BoxFuture<'_, Result<(), Error>>;

    /// Set gain level.
    fn set_gain(&self, level: GainLevel) -> BoxFuture<'_, Result<(), Error>>;
}

/// White balance control operations (object-safe).
///
/// This trait provides object-safe versions of white balance control methods.
pub trait WhiteBalanceControl: Send + Sync {
    /// Set white balance mode.
    fn set_mode(&self, mode: WhiteBalanceMode) -> BoxFuture<'_, Result<(), Error>>;

    /// Set auto white balance sensitivity.
    fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Trigger one-push white balance.
    fn one_push_trigger(&self) -> BoxFuture<'_, Result<(), Error>>;
}

/// Image processing control operations (object-safe).
///
/// This trait provides object-safe versions of image processing methods.
pub trait ImageControl: Send + Sync {
    /// Set image flip mode.
    fn set_flip_mode(&self, mode: ImageFlipMode) -> BoxFuture<'_, Result<(), Error>>;

    /// Enable image freeze.
    fn enable_freeze(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Disable image freeze.
    fn disable_freeze(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Set sharpness level.
    fn set_sharpness(&self, level: SharpnessLevel) -> BoxFuture<'_, Result<(), Error>>;

    /// Set sharpness mode.
    fn set_sharpness_mode(&self, mode: SharpnessMode) -> BoxFuture<'_, Result<(), Error>>;

    /// Set 2D noise reduction level.
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Disable 2D noise reduction.
    fn disable_noise_reduction_2d(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Set 3D noise reduction level.
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> BoxFuture<'_, Result<(), Error>>;

    /// Disable 3D noise reduction.
    fn disable_noise_reduction_3d(&self) -> BoxFuture<'_, Result<(), Error>>;

    /// Set picture effect mode.
    ///
    /// # Parameters
    /// - `mode`: Picture effect mode, or `None` to disable
    fn set_picture_effect(
        &self,
        mode: Option<PictureEffectMode>,
    ) -> BoxFuture<'_, Result<(), Error>>;
}

/// ND filter control operations (object-safe).
///
/// This trait provides object-safe versions of ND filter control methods.
pub trait NdFilterControl: Send + Sync {
    /// Set ND filter mode.
    fn set_mode(&self, mode: NdFilterMode) -> BoxFuture<'_, Result<(), Error>>;
}

/// Preset control operations (object-safe).
///
/// This trait provides object-safe versions of preset control methods.
pub trait PresetControl: Send + Sync {
    /// Recall a preset.
    ///
    /// Moves the camera to a previously stored preset position.
    /// This operation waits for completion before returning.
    fn recall(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>>;

    /// Store current position to a preset.
    fn store(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>>;

    /// Clear a preset.
    fn clear(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>>;
}

// Implementation of dynamic traits for CameraSession<Async, P, Tr, Exec>
impl<P, Tr, Exec> CameraControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn power_on(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::power::PowerControl;
            PowerControl::power_on(self).await
        })
    }

    fn power_off(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::power::PowerControl;
            PowerControl::power_off(self).await
        })
    }

    fn as_pan_tilt(&self) -> Option<&dyn PanTiltControl> {
        Some(self)
    }

    fn as_zoom(&self) -> Option<&dyn ZoomControl> {
        Some(self)
    }

    fn as_focus(&self) -> Option<&dyn FocusControl> {
        Some(self)
    }

    fn as_power(&self) -> Option<&dyn PowerControl> {
        Some(self)
    }

    fn as_exposure(&self) -> Option<&dyn ExposureControl> {
        Some(self)
    }

    fn as_white_balance(&self) -> Option<&dyn WhiteBalanceControl> {
        Some(self)
    }

    fn as_image(&self) -> Option<&dyn ImageControl> {
        Some(self)
    }

    fn as_presets(&self) -> Option<&dyn PresetControl> {
        Some(self)
    }
}

impl<P, Tr, Exec> PanTiltControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn pan_tilt_home(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::pan_tilt::PanTiltControl;
            PanTiltControl::pan_tilt_home(self).await
        })
    }

    fn pan_tilt_reset(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::pan_tilt::PanTiltControl;
            PanTiltControl::pan_tilt_reset(self).await
        })
    }

    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::pan_tilt::PanTiltControl;
            PanTiltControl::pan_tilt_absolute(self, pan, tilt, speed).await
        })
    }

    fn pan_tilt_position(&self) -> BoxFuture<'_, Result<PanTiltPosition, Error>> {
        Box::pin(async move {
            use crate::PanTiltInquiryControl;
            PanTiltInquiryControl::pan_tilt_position(self).await
        })
    }
}

impl<P, Tr, Exec> ZoomControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn zoom_absolute(&self, position: Normalized) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::zoom::ZoomControl;
            ZoomControl::zoom_absolute(self, position).await
        })
    }

    fn zoom_position(&self) -> BoxFuture<'_, Result<ZoomPosition, Error>> {
        Box::pin(async move {
            use crate::camera::controls::inquiry::InquiryControl;
            InquiryControl::zoom_position(self).await
        })
    }
}

impl<P, Tr, Exec> FocusControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn focus_auto(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::focus::FocusControl;
            FocusControl::focus_auto(self).await
        })
    }

    fn focus_manual(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::focus::FocusControl;
            FocusControl::focus_manual(self).await
        })
    }

    fn focus_one_push(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::focus::FocusControl;
            FocusControl::focus_one_push(self).await
        })
    }
}

impl<P, Tr, Exec> PowerControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn power_on(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::power::PowerControl;
            PowerControl::power_on(self).await
        })
    }

    fn power_off(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::power::PowerControl;
            PowerControl::power_off(self).await
        })
    }
}

impl<P, Tr, Exec> ExposureControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn set_exposure_mode(&self, mode: ExposureMode) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::exposure::ExposureControl;
            ExposureControl::set_exposure_mode(self, mode).await
        })
    }

    fn set_iris(&self, level: IrisLevel) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::exposure::ExposureControl;
            ExposureControl::set_iris(self, level).await
        })
    }

    fn set_gain(&self, level: GainLevel) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::exposure::ExposureControl;
            ExposureControl::set_gain(self, level).await
        })
    }
}

impl<P, Tr, Exec> WhiteBalanceControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn set_mode(&self, mode: WhiteBalanceMode) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::white_balance::WhiteBalanceControl;
            WhiteBalanceControl::set_white_balance_mode(self, mode).await
        })
    }

    fn set_awb_sensitivity(
        &self,
        sensitivity: AutoWhiteBalanceSensitivity,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::white_balance::WhiteBalanceControl;
            WhiteBalanceControl::set_awb_sensitivity(self, sensitivity).await
        })
    }

    fn one_push_trigger(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::white_balance::WhiteBalanceControl;
            WhiteBalanceControl::white_balance_one_push(self).await
        })
    }
}

impl<P, Tr, Exec> ImageControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn set_flip_mode(&self, mode: ImageFlipMode) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::set_image_flip(self, mode).await
        })
    }

    fn enable_freeze(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::enable_freeze(self).await
        })
    }

    fn disable_freeze(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::disable_freeze(self).await
        })
    }

    fn set_sharpness(&self, level: SharpnessLevel) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::set_sharpness(self, level).await
        })
    }

    fn set_sharpness_mode(&self, mode: SharpnessMode) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::set_sharpness_mode(self, mode).await
        })
    }

    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::set_noise_reduction_2d(self, level).await
        })
    }

    fn disable_noise_reduction_2d(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::disable_noise_reduction_2d(self).await
        })
    }

    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::set_noise_reduction_3d(self, level).await
        })
    }

    fn disable_noise_reduction_3d(&self) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            ImageProcessingControl::disable_noise_reduction_3d(self).await
        })
    }

    fn set_picture_effect(
        &self,
        mode: Option<PictureEffectMode>,
    ) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::image_processing::ImageProcessingControl;
            let effect_mode = mode.unwrap_or(PictureEffectMode::Off);
            ImageProcessingControl::set_picture_effect(self, effect_mode).await
        })
    }
}

// Additional implementation for profiles that support ND filter
impl<P, Tr, Exec> CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + crate::capabilities::nd_filter::NdFilter
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    /// Get ND filter control capability for profiles that support it.
    pub fn as_nd_filter_dyn(&self) -> &dyn NdFilterControl {
        self
    }
}

impl<P, Tr, Exec> NdFilterControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + crate::capabilities::nd_filter::NdFilter
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn set_mode(&self, mode: NdFilterMode) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::nd_filter::NdFilterControl;
            NdFilterControl::set_nd_filter_mode(self, mode).await
        })
    }
}

impl<P, Tr, Exec> PresetControl for CameraSession<Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile
        + crate::capabilities::ProfileMetadata
        + Default
        + Send
        + Sync
        + 'static,
    Tr: AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + 'static,
{
    fn recall(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::presets::PresetsControl;
            PresetsControl::preset_recall(self, preset).await
        })
    }

    fn store(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::presets::PresetsControl;
            PresetsControl::preset_set(self, preset).await
        })
    }

    fn clear(&self, preset: PresetNumber) -> BoxFuture<'_, Result<(), Error>> {
        Box::pin(async move {
            use crate::camera::controls::presets::PresetsControl;
            PresetsControl::preset_reset(self, preset).await
        })
    }
}

/// Open a UDP connection to a camera and return a dynamic trait object.
///
/// This function provides runtime polymorphism by returning a boxed trait object
/// that can represent any camera profile. The specific profile implementation is
/// selected based on the `ProfileGroup` associated with the provided `ProfileId`.
///
/// # Parameters
///
/// - `profile`: The camera profile identifier
/// - `address`: UDP address in the format "ip:port" (e.g., "192.168.1.50:52381")
/// - `executor`: The async executor to use for async operations
///
/// # Returns
///
/// An `Arc<dyn CameraControl>` that can be used polymorphically regardless of the
/// underlying camera profile.
///
/// # Errors
///
/// Returns an error if the connection fails or if the address is invalid.
///
/// # Examples
///
/// ```rust,no_run
/// # #[cfg(feature = "dyn-api")] {
/// use grafton_visca::dynapi::open_udp_dynamic;
/// use grafton_visca::camera::profiles::ProfileId;
/// use grafton_visca::runtime::TokioRuntime;
///
/// # async fn example() -> Result<(), grafton_visca::Error> {
/// let camera = open_udp_dynamic(
///     ProfileId::SonyFr7,
///     "192.168.1.50:52381",
///     TokioRuntime
/// ).await?;
///
/// camera.power_on().await?;
/// # Ok(())
/// # }
/// # }
/// ```
pub async fn open_udp_dynamic<R>(
    profile: ProfileId,
    address: &str,
    runtime: R,
) -> Result<Arc<dyn CameraControl>, Error>
where
    R: crate::runtime::Runtime,
    R::TcpTransport: Sync,
    R::UdpTransport: Sync,
{
    use crate::camera::profiles::ProfileGroup;

    match profile.profile_group() {
        ProfileGroup::GenericVisca => {
            let cam = Connect::open_udp_async::<GenericVisca, _>(address, runtime).await?;
            Ok(Arc::new(cam))
        }
        ProfileGroup::PtzOpticsG2 => {
            let cam = Connect::open_udp_async::<PtzOpticsG2, _>(address, runtime).await?;
            Ok(Arc::new(cam))
        }
        ProfileGroup::SonyProfessional => {
            let cam = Connect::open_udp_async::<SonyFR7, _>(address, runtime).await?;
            Ok(Arc::new(cam))
        }
    }
}

/// Open a TCP connection to a camera and return a dynamic trait object.
///
/// This function provides runtime polymorphism by returning a boxed trait object
/// that can represent any camera profile. The specific profile implementation is
/// selected based on the `ProfileGroup` associated with the provided `ProfileId`.
///
/// # Parameters
///
/// - `profile`: The camera profile identifier
/// - `address`: TCP address in the format "ip:port" (e.g., "192.168.1.50:5678")
/// - `executor`: The async executor to use for async operations
///
/// # Returns
///
/// An `Arc<dyn CameraControl>` that can be used polymorphically regardless of the
/// underlying camera profile.
///
/// # Errors
///
/// Returns an error if the connection fails or if the address is invalid.
///
/// # Examples
///
/// ```rust,no_run
/// # #[cfg(feature = "dyn-api")] {
/// use grafton_visca::dynapi::open_tcp_dynamic;
/// use grafton_visca::camera::profiles::ProfileId;
/// use grafton_visca::runtime::TokioRuntime;
///
/// # async fn example() -> Result<(), grafton_visca::Error> {
/// let camera = open_tcp_dynamic(
///     ProfileId::PtzOpticsG2,
///     "192.168.1.50:5678",
///     TokioRuntime
/// ).await?;
///
/// camera.power_on().await?;
/// # Ok(())
/// # }
/// # }
/// ```
pub async fn open_tcp_dynamic<R>(
    profile: ProfileId,
    address: &str,
    runtime: R,
) -> Result<Arc<dyn CameraControl>, Error>
where
    R: crate::runtime::Runtime,
    R::TcpTransport: Sync,
    R::UdpTransport: Sync,
{
    use crate::camera::profiles::ProfileGroup;

    match profile.profile_group() {
        ProfileGroup::GenericVisca => {
            let cam = Connect::open_tcp_async::<GenericVisca, _>(address, runtime).await?;
            Ok(Arc::new(cam))
        }
        ProfileGroup::PtzOpticsG2 => {
            let cam = Connect::open_tcp_async::<PtzOpticsG2, _>(address, runtime).await?;
            Ok(Arc::new(cam))
        }
        ProfileGroup::SonyProfessional => {
            let cam = Connect::open_tcp_async::<SonyFR7, _>(address, runtime).await?;
            Ok(Arc::new(cam))
        }
    }
}
