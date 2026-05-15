//! Inquiry control implementation for PTZ cameras.
//!
//! This module provides comprehensive camera status inquiry functionality including:
//! - Current camera settings and modes (focus, exposure, white balance, etc.)
//! - Position information (pan, tilt, zoom, focus positions)
//! - Image processing settings (sharpness, saturation, noise reduction)
//! - System status (power, version, menu state)
//! - Color settings (gain, tuning, temperature)
//! - Special features (tally lights, digital PTZ, defog)
//!
//! Inquiry operations allow you to read the current state of various camera
//! parameters without changing them. This is essential for building user
//! interfaces, monitoring camera status, and synchronizing settings.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::{
        focus::AutoFocusSensitivity, ExposureMode, FocusMode, FocusZone, SharpnessMode,
        WhiteBalanceMode,
    },
    mode::Mode,
    Error,
};

/// Inquiry operations for PTZ cameras.
///
/// This trait provides comprehensive inquiry methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Inquiry Categories
///
/// - **Position**: Current pan/tilt/zoom/focus positions
/// - **Exposure**: Iris, shutter, gain, exposure mode, compensation
/// - **Color**: White balance, color temperature, red/blue gain and tuning
/// - **Image**: Sharpness, saturation, hue, noise reduction, picture effects
/// - **System**: Power state, version, menu status, tally lights
/// - **Special**: digital PTZ, defog
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// let zoom_pos = camera.zoom_position()?;
/// let exp_mode = camera.exposure_mode()?;
/// let wb_mode = camera.white_balance_mode()?;
/// let version = camera.version()?;
/// ```
///
/// ## Async mode
/// ```ignore
/// let zoom_pos = camera.zoom_position().await?;
/// let exp_mode = camera.exposure_mode().await?;
/// let wb_mode = camera.white_balance_mode().await?;
/// let version = camera.version().await?;
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait InquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current power state of the camera.
    ///
    /// Returns the camera's power status.
    ///
    /// # Returns
    /// - `true` if the camera is powered on and operational
    /// - `false` if the camera is in standby mode
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn power_state(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current zoom position.
    ///
    /// Returns the current zoom level as a position value.
    /// The range depends on the camera model and zoom capabilities.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn zoom_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ZoomPosition, Error>>;

    /// Get the current focus position.
    ///
    /// Returns the current focus position as a typed value.
    /// Higher values typically indicate focus on more distant objects.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::FocusPosition, Error>>;

    /// Get the current exposure mode.
    ///
    /// Returns the active exposure mode (auto, manual, shutter priority, etc.).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn exposure_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<ExposureMode, Error>>;

    /// Get the current shutter speed.
    ///
    /// Returns the current shutter speed setting as a typed value.
    /// The exact interpretation depends on the camera model.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn shutter(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ShutterSpeed, Error>>;

    /// Get the current gain value.
    ///
    /// Returns the current sensor gain level. Higher values increase
    /// image brightness but also increase noise.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::GainLevel, Error>>;

    /// Get the gain limit value.
    ///
    /// Returns the maximum gain level that auto exposure is allowed to use.
    /// This helps control noise in low light conditions.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn gain_limit(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::GainLimit, Error>>;

    /// Get the white balance mode.
    ///
    /// Returns the current white balance mode (auto, indoor, outdoor, etc.).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn white_balance_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<WhiteBalanceMode, Error>>;

    /// Get the brightness level.
    ///
    /// Returns the current brightness adjustment level.
    /// This is separate from exposure brightness.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn brightness(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::BrightnessLevel, Error>>;

    /// Get the sharpness mode.
    ///
    /// Returns the current sharpness mode setting (auto or manual).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn sharpness_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<SharpnessMode, Error>>;

    /// Get the sharpness level.
    ///
    /// Returns the current sharpness level setting.
    /// Higher values increase edge enhancement/sharpening.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn sharpness_level(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::SharpnessLevel, Error>>;

    /// Get the contrast level.
    ///
    /// Returns the current contrast level setting.
    /// Higher values increase the difference between light and dark areas.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn contrast(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ContrastLevel, Error>>;

    /// Get the current video resolution mode.
    ///
    /// Returns the current video resolution setting.
    /// Common modes include 1080p60, 1080p30, 720p60, etc.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn resolution(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::ResolutionMode, Error>>;

    /// Get the camera version information.
    ///
    /// Returns detailed version information including model name,
    /// firmware version, and other identifying information.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn version(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::VersionInfo, Error>>;

    /// Get the current focus mode.
    ///
    /// Returns whether the camera is in auto focus or manual focus mode.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<FocusMode, Error>>;

    /// Get the menu open/close status.
    ///
    /// Returns whether the camera's on-screen menu is currently displayed.
    ///
    /// # Returns
    /// - `true` if the menu is open/visible
    /// - `false` if the menu is closed/hidden
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn menu_status(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the night/day mode status.
    ///
    /// Returns whether the camera is in night mode (for low light conditions)
    /// or day mode (for normal lighting conditions).
    ///
    /// # Returns
    /// - `true` if night mode is active
    /// - `false` if day mode is active
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn night_day_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the standby mode status.
    ///
    /// Returns whether the camera is in standby mode.
    ///
    /// # Returns
    /// - `true` if standby mode is enabled
    /// - `false` if standby mode is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn standby_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the defog level.
    ///
    /// Returns the current defog processing level which helps
    /// improve visibility in foggy or hazy conditions.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn defog_level(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::DefogLevel, Error>>;

    /// Get the digital PTZ mode status.
    ///
    /// Returns whether digital pan/tilt/zoom is enabled.
    /// Digital PTZ uses image cropping instead of mechanical movement.
    ///
    /// # Returns
    /// - `true` if digital PTZ is enabled
    /// - `false` if digital PTZ is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn digital_ptz_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the auto trace mode status.
    ///
    /// Returns whether automatic subject tracking is enabled.
    /// Auto trace attempts to follow moving subjects automatically.
    ///
    /// # Returns
    /// - `true` if auto trace is enabled
    /// - `false` if auto trace is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn auto_trace_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the focus unlock state.
    ///
    /// Returns whether focus unlock is enabled, which may affect
    /// how focus controls respond.
    ///
    /// # Returns
    /// - `true` if focus unlock is enabled
    /// - `false` if focus unlock is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_unlock(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the broadcast domain setting.
    ///
    /// Returns the current broadcast domain configuration
    /// which may affect video output standards.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn broadcast_domain(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::BroadcastDomain, Error>>;

    /// Get the USB audio state.
    ///
    /// Returns whether USB audio is enabled for cameras
    /// that support audio over USB connections.
    ///
    /// # Returns
    /// - `true` if USB audio is enabled
    /// - `false` if USB audio is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn usb_audio_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the two tone mode state.
    ///
    /// Returns whether two tone mode is enabled, which may
    /// affect color reproduction or contrast.
    ///
    /// # Returns
    /// - `true` if two tone mode is enabled
    /// - `false` if two tone mode is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn two_tone_mode_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the digital mode state.
    ///
    /// Returns whether digital mode is enabled, which may affect
    /// image processing or output characteristics.
    ///
    /// # Returns
    /// - `true` if digital mode is enabled
    /// - `false` if digital mode is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn digital_mode_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the anti-flicker mode setting.
    ///
    /// Returns the current flicker reduction mode (Off, 50Hz, or 60Hz).
    ///
    /// **Vendor-Specific**: PTZOptics cameras only.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn flicker_mode(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::exposure::AntiFlickerMode, Error>>;
}

/// ND filter-specific inquiry operations for cameras with typed ND filter support.
#[grafton_visca_macros::delegate_to_session]
pub trait NdFilterInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current ND filter position.
    fn nd_filter_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::NdFilterPosition, Error>>;

    /// Get the ND filter preset setting.
    fn nd_filter_preset(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::NdFilterPreset, Error>>;
}

/// Focus near-limit inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait FocusNearLimitInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the focus near limit position.
    fn focus_near_limit(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::FocusPosition, Error>>;
}

/// Focus zone inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait FocusZoneInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current focus zone.
    fn focus_zone(&self) -> <Self::Mode as Mode>::Fut<'_, Result<FocusZone, Error>>;
}

/// Auto-focus sensitivity inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait AutoFocusSensitivityInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the auto focus sensitivity.
    fn auto_focus_sensitivity(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<AutoFocusSensitivity, Error>>;
}

/// Iris value inquiry for profiles with documented iris support.
#[grafton_visca_macros::delegate_to_session]
pub trait IrisInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the iris control mode.
    fn iris_control(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current iris value.
    fn iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::IrisLevel, Error>>;
}

/// Exposure-compensation inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ExposureCompensationInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get exposure compensation value.
    fn exposure_compensation(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ExposureCompensationLevel, Error>>;

    /// Check whether exposure compensation is enabled.
    fn exposure_compensation_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get exposure compensation position.
    fn exposure_compensation_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ExposureCompensationPosition, Error>>;
}

/// Backlight compensation inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait BacklightCompensationInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Check whether backlight compensation is enabled.
    fn backlight_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;
}

/// Wide dynamic range inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait WideDynamicRangeInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get dynamic range level.
    fn dynamic_range(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::DynamicRangeLevel, Error>>;
}

/// Color-temperature inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ColorTemperatureInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get color temperature.
    fn color_temperature(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ColorTemp, Error>>;
}

/// RGB gain inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait RgbGainInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get red gain.
    fn red_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::RedChannel, Error>>;

    /// Get blue gain.
    fn blue_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::BlueChannel, Error>>;
}

/// RGB tuning inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait RgbTuningInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get red tuning.
    fn red_tuning(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::RedTuning, Error>>;

    /// Get blue tuning.
    fn blue_tuning(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::BlueTuning, Error>>;
}

/// Saturation inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait SaturationInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get saturation level.
    fn saturation(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::SaturationLevel, Error>>;
}

/// Hue inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait HueInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get hue level.
    fn hue(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::HueLevel, Error>>;
}

/// Luminance inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait LuminanceInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get luminance level.
    fn luminance(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::LuminanceLevel, Error>>;
}

/// Gamma inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait GammaInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get gamma level.
    fn gamma(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::GammaLevel, Error>>;
}

/// Image flip inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ImageFlipInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get image flip settings.
    fn image_flip(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::FlipState, Error>>;

    /// Get combined flip mode.
    fn flip_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::FlipState, Error>>;
}

/// Aggregate noise-reduction inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait NoiseReductionInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get aggregate noise reduction level.
    fn noise_reduction_level(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::NoiseReductionLevel, Error>>;

    /// Get noise reduction mode.
    fn noise_reduction_mode(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::NoiseReductionMode, Error>>;
}

/// 2D noise-reduction inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait NoiseReduction2DInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get 2D noise-reduction level.
    fn noise_reduction_2d(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::NoiseReduction2DLevel, Error>>;
}

/// 3D noise-reduction inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait NoiseReduction3DInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get 3D noise-reduction level.
    fn noise_reduction_3d(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::NoiseReduction3DLevel, Error>>;
}

/// Picture-effect inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait PictureEffectInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Check whether black-and-white mode is enabled.
    fn black_white(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get black-and-white mode.
    fn black_white_mode(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::BlackWhiteMode, Error>>;

    /// Get picture effect mode.
    fn picture_effect(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::PictureEffectMode, Error>>;
}

/// Pan/tilt-specific inquiry operations for cameras.
///
/// This trait provides pan/tilt position inquiry methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// let position = camera.pan_tilt_position()?;
/// println!("Pan: {}, Tilt: {}", position.pan, position.tilt);
/// ```
///
/// ## Async mode
/// ```ignore
/// let position = camera.pan_tilt_position().await?;
/// println!("Pan: {}, Tilt: {}", position.pan, position.tilt);
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait PanTiltInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current pan and tilt position.
    ///
    /// Returns the current pan and tilt position coordinates.
    /// Values are typically in degrees or camera-specific units.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn pan_tilt_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::camera::PanTiltPosition, Error>>;
}

macro_rules! impl_builtin_camera_inquiry_accessors {
    (
        queryable { $($query_entries:tt)* }
        decode_only { $($decode_entries:tt)* }
        accessors { $($accessor_groups:tt)* }
    ) => {
        impl_builtin_camera_inquiry_accessors!(@groups $($accessor_groups)*);
    };
    (@groups) => {};
    (@groups
        TallyControl {
            gate: none;
            $($entries:tt)*
        }
        $($rest:tt)*
    ) => {
        impl_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@groups
        TallyControl {
            gate: $profile_gate:path;
            $($entries:tt)*
        }
        $($rest:tt)*
    ) => {
        impl_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@groups
        NdFilterControl {
            gate: none;
            $($entries:tt)*
        }
        $($rest:tt)*
    ) => {
        impl_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@groups
        NdFilterControl {
            gate: $profile_gate:path;
            $($entries:tt)*
        }
        $($rest:tt)*
    ) => {
        impl_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@groups
        MotionSyncControl {
            gate: none;
            $($entries:tt)*
        }
        $($rest:tt)*
    ) => {
        impl_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@groups
        MotionSyncControl {
            gate: $profile_gate:path;
            $($entries:tt)*
        }
        $($rest:tt)*
    ) => {
        impl_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@groups
        $trait_name:ident {
            gate: none;
            $(
                $command:ident => $method:ident : $response_ty:ty;
            )*
        }
        $($rest:tt)*
    ) => {
        impl<M, P, Tr, Exec> $trait_name for crate::camera::Camera<M, P, Tr, Exec>
        where
            M: Mode,
            P: crate::capabilities::Profile + Default,
            Self: ViscaClient<M>,
            Exec: crate::executor::Executor,
        {
            type Mode = M;

            $(
                fn $method(&self) -> M::Fut<'_, Result<$response_ty, Error>> {
                    self.query(crate::command::inquiry_structs::$command)
                }
            )*
        }

        impl_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@groups
        $trait_name:ident {
            gate: $profile_gate:path;
            $(
                $command:ident => $method:ident : $response_ty:ty;
            )*
        }
        $($rest:tt)*
    ) => {
        impl<M, P, Tr, Exec> $trait_name for crate::camera::Camera<M, P, Tr, Exec>
        where
            M: Mode,
            P: crate::capabilities::Profile + Default + $profile_gate,
            Self: ViscaClient<M>,
            Exec: crate::executor::Executor,
        {
            type Mode = M;

            $(
                fn $method(&self) -> M::Fut<'_, Result<$response_ty, Error>> {
                    self.query(crate::command::inquiry_structs::$command)
                }
            )*
        }

        impl_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
}

crate::command::inquiry_structs::builtin_inquiry_table!(impl_builtin_camera_inquiry_accessors);
