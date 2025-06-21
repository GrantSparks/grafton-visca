//! Camera profile system for type-safe, model-specific VISCA control.

use std::fmt::Display;
use std::ops::RangeInclusive;

use crate::error::Error;
#[cfg(not(feature = "async"))]
use crate::transport::blocking::{
    Transport as BlockingTransport, ViscaTransport as BlockingViscaTransport,
};
#[cfg(feature = "async")]
use crate::transport::AsyncTransport;
use crate::transport::ViscaTransport;
use crate::{Command, Response};

pub mod builder;
pub mod command_builder;
pub mod commands;
pub mod extensions;
pub mod inquiry;
pub mod profiles;

// Re-export commonly used types
pub use builder::{CustomProfile, CustomProfileBuilder, CustomProfileTypedBuilder};
pub use command_builder::CommandBuilderExt;
pub use extensions::CameraExtension;
pub use inquiry::{CameraState, Exposure, ImageSettings, Optics, Position, WhiteBalance};
pub use profiles::{GenericVisca, PTZOptics30X, PTZOpticsG2, SonyEVID70};

/// Core camera abstraction with compile-time profile information.
/// Camera control interface with type-safe profile support.
#[derive(Debug)]
pub struct Camera<P: CameraProfile, T = ()> {
    profile: P,
    transport: ViscaTransport<T>,
}

/// Trait for blocking transports that can send VISCA commands.
#[cfg(not(feature = "async"))]
pub trait BlockingCameraTransport: Send + Sync {
    /// Send a VISCA command and wait for response.
    fn send_command(&mut self, command: &dyn Command) -> Result<Response, Error>;
}

#[cfg(not(feature = "async"))]
impl<T: BlockingTransport> BlockingCameraTransport for BlockingViscaTransport<T> {
    fn send_command(&mut self, command: &dyn Command) -> Result<Response, Error> {
        self.send_command(command)
    }
}

/// Trait defining camera-specific capabilities and conversions.
pub trait CameraProfile: Default + Send + Sync + std::fmt::Debug {
    /// Camera model name for identification.
    const MODEL_NAME: &'static str;

    /// Valid pan position range in VISCA units.
    const PAN_RANGE: RangeInclusive<i16>;

    /// Valid tilt position range in VISCA units.
    const TILT_RANGE: RangeInclusive<i16>;

    /// Valid zoom position range in VISCA units.
    const ZOOM_RANGE: RangeInclusive<u16>;

    /// Valid focus position range in VISCA units.
    const FOCUS_RANGE: RangeInclusive<u16>;

    /// Whether this camera supports digital zoom.
    const DIGITAL_ZOOM_SUPPORTED: bool = false;

    /// Maximum pan speed supported by this camera.
    const MAX_PAN_SPEED: u8 = 24;

    /// Maximum tilt speed supported by this camera.
    const MAX_TILT_SPEED: u8 = 20;

    /// Convert pan position from VISCA units to degrees.
    fn pan_units_to_degrees(&self, units: i16) -> f32;

    /// Convert tilt position from VISCA units to degrees.
    fn tilt_units_to_degrees(&self, units: i16) -> f32;

    /// Convert pan position from degrees to VISCA units.
    fn pan_degrees_to_units(&self, degrees: f32) -> i16;

    /// Convert tilt position from degrees to VISCA units.
    fn tilt_degrees_to_units(&self, degrees: f32) -> i16;

    /// Get the model name (allows instance override).
    fn model_name(&self) -> &str {
        Self::MODEL_NAME
    }

    /// Get the pan range (allows instance override).
    fn pan_range(&self) -> RangeInclusive<i16> {
        Self::PAN_RANGE
    }

    /// Get the tilt range (allows instance override).
    fn tilt_range(&self) -> RangeInclusive<i16> {
        Self::TILT_RANGE
    }

    /// Get the zoom range (allows instance override).
    fn zoom_range(&self) -> RangeInclusive<u16> {
        Self::ZOOM_RANGE
    }

    /// Get the focus range (allows instance override).
    fn focus_range(&self) -> RangeInclusive<u16> {
        Self::FOCUS_RANGE
    }

    /// Check if digital zoom is supported (allows instance override).
    fn digital_zoom_supported(&self) -> bool {
        Self::DIGITAL_ZOOM_SUPPORTED
    }

    /// Get the maximum pan speed (allows instance override).
    fn max_pan_speed(&self) -> u8 {
        Self::MAX_PAN_SPEED
    }

    /// Get the maximum tilt speed (allows instance override).
    fn max_tilt_speed(&self) -> u8 {
        Self::MAX_TILT_SPEED
    }

    /// Get the total pan range in degrees.
    fn pan_degree_range(&self) -> RangeInclusive<f32> {
        let min = self.pan_units_to_degrees(*self.pan_range().start());
        let max = self.pan_units_to_degrees(*self.pan_range().end());
        min..=max
    }

    /// Get the total tilt range in degrees.
    fn tilt_degree_range(&self) -> RangeInclusive<f32> {
        let min = self.tilt_units_to_degrees(*self.tilt_range().start());
        let max = self.tilt_units_to_degrees(*self.tilt_range().end());
        min..=max
    }

    /// Associated type for camera-specific preset IDs.
    type PresetId: Into<u8> + TryFrom<u8, Error = Error> + Copy + Display;

    /// Associated type for camera-specific gain values.
    type GainValue: Into<u8> + TryFrom<u8, Error = Error> + Copy + Display;

    /// Get the maximum preset ID for this camera.
    fn max_preset_id() -> u8;

    // Capability query methods

    /// Check if the camera supports auto-focus.
    fn supports_auto_focus(&self) -> bool {
        true // Most VISCA cameras support auto-focus
    }

    /// Check if the camera supports manual focus.
    fn supports_manual_focus(&self) -> bool {
        true // Most VISCA cameras support manual focus
    }

    /// Check if the camera supports auto-exposure.
    fn supports_auto_exposure(&self) -> bool {
        true // Most VISCA cameras support auto-exposure
    }

    /// Check if the camera supports manual exposure.
    fn supports_manual_exposure(&self) -> bool {
        true // Most VISCA cameras support manual exposure
    }

    /// Check if the camera supports white balance adjustment.
    fn supports_white_balance(&self) -> bool {
        true // Most VISCA cameras support white balance
    }

    /// Check if the camera supports image flip (horizontal/vertical).
    fn supports_image_flip(&self) -> bool {
        true // Most modern VISCA cameras support image flip
    }

    /// Check if the camera supports backlight compensation.
    fn supports_backlight_compensation(&self) -> bool {
        true // Common feature in VISCA cameras
    }

    /// Check if the camera supports wide dynamic range (WDR).
    fn supports_wide_dynamic_range(&self) -> bool {
        false // Not all cameras support this
    }

    /// Check if the camera supports image stabilization.
    fn supports_image_stabilization(&self) -> bool {
        false // Camera-specific feature
    }

    /// Check if the camera supports low-light mode.
    fn supports_low_light_mode(&self) -> bool {
        false // Camera-specific feature
    }

    /// Check if the camera supports color control (saturation, hue).
    fn supports_color_control(&self) -> bool {
        true // Most VISCA cameras support basic color control
    }

    /// Check if the camera supports gain control.
    fn supports_gain_control(&self) -> bool {
        true // Most VISCA cameras support gain control
    }

    /// Check if the camera supports shutter speed control.
    fn supports_shutter_control(&self) -> bool {
        true // Most VISCA cameras support shutter control
    }

    /// Check if the camera supports iris control.
    fn supports_iris_control(&self) -> bool {
        true // Most VISCA cameras support iris control
    }

    /// Check if the camera supports noise reduction.
    fn supports_noise_reduction(&self) -> bool {
        false // Camera-specific feature
    }

    /// Check if the camera supports privacy zones.
    fn supports_privacy_zones(&self) -> bool {
        false // Not a standard VISCA feature
    }

    /// Check if the camera supports motion detection.
    fn supports_motion_detection(&self) -> bool {
        false // Not a standard VISCA feature
    }

    /// Get the number of available white balance modes.
    fn white_balance_mode_count(&self) -> u8 {
        5 // Typical: Auto, Indoor, Outdoor, One-Push, Manual
    }

    /// Get the number of available exposure modes.
    fn exposure_mode_count(&self) -> u8 {
        4 // Typical: Auto, Manual, Shutter Priority, Iris Priority
    }

    /// Check if the camera supports a specific zoom speed.
    fn supports_zoom_speed(&self, speed: u8) -> bool {
        speed <= 7 // Most VISCA cameras support zoom speeds 0-7
    }

    /// Check if the camera supports a specific focus speed.
    fn supports_focus_speed(&self, speed: u8) -> bool {
        speed <= 7 // Most VISCA cameras support focus speeds 0-7
    }

    /// Get the supported shutter speed range (if applicable).
    /// Returns None if shutter control is not supported.
    fn shutter_speed_range(&self) -> Option<RangeInclusive<u8>> {
        if self.supports_shutter_control() {
            Some(0..=21) // Typical VISCA shutter speed range
        } else {
            None
        }
    }

    /// Get the supported iris range (if applicable).
    /// Returns None if iris control is not supported.
    fn iris_range(&self) -> Option<RangeInclusive<u8>> {
        if self.supports_iris_control() {
            Some(0..=20) // Typical VISCA iris range
        } else {
            None
        }
    }

    /// Get the supported gain range (if applicable).
    /// Returns None if gain control is not supported.
    fn gain_range(&self) -> Option<RangeInclusive<u8>> {
        if self.supports_gain_control() {
            Some(0..=15) // Typical VISCA gain range
        } else {
            None
        }
    }

    /// Check if the camera supports continuous pan/tilt movement.
    fn supports_continuous_movement(&self) -> bool {
        true // Most VISCA cameras support this
    }

    /// Check if the camera supports absolute position commands.
    fn supports_absolute_positioning(&self) -> bool {
        true // Standard VISCA feature
    }

    /// Check if the camera supports relative position commands.
    fn supports_relative_positioning(&self) -> bool {
        true // Standard VISCA feature
    }

    /// Get a list of supported features as a capability summary.
    fn capability_summary(&self) -> CapabilitySummary {
        CapabilitySummary {
            model: self.model_name().to_string(),
            movement: MovementCapabilities {
                continuous: self.supports_continuous_movement(),
                absolute: self.supports_absolute_positioning(),
                relative: self.supports_relative_positioning(),
                pan_range: self.pan_degree_range(),
                tilt_range: self.tilt_degree_range(),
                max_pan_speed: self.max_pan_speed(),
                max_tilt_speed: self.max_tilt_speed(),
            },
            zoom: ZoomCapabilities {
                optical_range: self.zoom_range(),
                digital_zoom: self.digital_zoom_supported(),
                speed_levels: 8, // Standard VISCA zoom speed levels
            },
            focus: FocusCapabilities {
                auto_focus: self.supports_auto_focus(),
                manual_focus: self.supports_manual_focus(),
                range: self.focus_range(),
                speed_levels: 8, // Standard VISCA focus speed levels
            },
            exposure: ExposureCapabilities {
                auto_exposure: self.supports_auto_exposure(),
                manual_exposure: self.supports_manual_exposure(),
                shutter_control: self.supports_shutter_control(),
                iris_control: self.supports_iris_control(),
                gain_control: self.supports_gain_control(),
                backlight_comp: self.supports_backlight_compensation(),
                wide_dynamic_range: self.supports_wide_dynamic_range(),
            },
            image: ImageCapabilities {
                white_balance: self.supports_white_balance(),
                color_control: self.supports_color_control(),
                image_flip: self.supports_image_flip(),
                image_stabilization: self.supports_image_stabilization(),
                noise_reduction: self.supports_noise_reduction(),
                low_light_mode: self.supports_low_light_mode(),
            },
            presets: PresetCapabilities {
                count: Self::max_preset_id() + 1,
                speed_support: true, // Most VISCA cameras support preset recall speed
            },
        }
    }
}

/// Summary of all camera capabilities.
#[derive(Debug, Clone)]
pub struct CapabilitySummary {
    /// Camera model name.
    pub model: String,
    /// Movement capabilities.
    pub movement: MovementCapabilities,
    /// Zoom capabilities.
    pub zoom: ZoomCapabilities,
    /// Focus capabilities.
    pub focus: FocusCapabilities,
    /// Exposure capabilities.
    pub exposure: ExposureCapabilities,
    /// Image processing capabilities.
    pub image: ImageCapabilities,
    /// Preset capabilities.
    pub presets: PresetCapabilities,
}

/// Movement-related capabilities.
#[derive(Debug, Clone)]
pub struct MovementCapabilities {
    /// Supports continuous pan/tilt movement.
    pub continuous: bool,
    /// Supports absolute positioning.
    pub absolute: bool,
    /// Supports relative positioning.
    pub relative: bool,
    /// Pan range in degrees.
    pub pan_range: RangeInclusive<f32>,
    /// Tilt range in degrees.
    pub tilt_range: RangeInclusive<f32>,
    /// Maximum pan speed.
    pub max_pan_speed: u8,
    /// Maximum tilt speed.
    pub max_tilt_speed: u8,
}

/// Zoom-related capabilities.
#[derive(Debug, Clone)]
pub struct ZoomCapabilities {
    /// Optical zoom range.
    pub optical_range: RangeInclusive<u16>,
    /// Digital zoom support.
    pub digital_zoom: bool,
    /// Number of zoom speed levels.
    pub speed_levels: u8,
}

/// Focus-related capabilities.
#[derive(Debug, Clone)]
pub struct FocusCapabilities {
    /// Auto-focus support.
    pub auto_focus: bool,
    /// Manual focus support.
    pub manual_focus: bool,
    /// Focus position range.
    pub range: RangeInclusive<u16>,
    /// Number of focus speed levels.
    pub speed_levels: u8,
}

/// Exposure-related capabilities.
#[derive(Debug, Clone, Copy)]
pub struct ExposureCapabilities {
    /// Auto-exposure support.
    pub auto_exposure: bool,
    /// Manual exposure support.
    pub manual_exposure: bool,
    /// Shutter speed control.
    pub shutter_control: bool,
    /// Iris control.
    pub iris_control: bool,
    /// Gain control.
    pub gain_control: bool,
    /// Backlight compensation.
    pub backlight_comp: bool,
    /// Wide dynamic range.
    pub wide_dynamic_range: bool,
}

/// Image processing capabilities.
#[derive(Debug, Clone, Copy)]
pub struct ImageCapabilities {
    /// White balance control.
    pub white_balance: bool,
    /// Color control (saturation, hue).
    pub color_control: bool,
    /// Image flip support.
    pub image_flip: bool,
    /// Image stabilization.
    pub image_stabilization: bool,
    /// Noise reduction.
    pub noise_reduction: bool,
    /// Low-light mode.
    pub low_light_mode: bool,
}

/// Preset-related capabilities.
#[derive(Debug, Clone, Copy)]
pub struct PresetCapabilities {
    /// Number of available presets.
    pub count: u8,
    /// Preset recall speed support.
    pub speed_support: bool,
}

/// Runtime camera capabilities for discovery and validation.
#[derive(Debug, Clone)]
pub struct CameraCapabilities {
    /// Camera model name.
    pub model_name: String,
    /// Pan range in degrees.
    pub pan_range_degrees: RangeInclusive<f32>,
    /// Tilt range in degrees.
    pub tilt_range_degrees: RangeInclusive<f32>,
    /// Total number of zoom steps.
    pub zoom_steps: usize,
    /// Total number of focus steps.
    pub focus_steps: usize,
    /// Number of available presets.
    pub preset_count: u8,
    /// Whether digital zoom is supported.
    pub supports_digital_zoom: bool,
    /// Maximum pan speed.
    pub max_pan_speed: u8,
    /// Maximum tilt speed.
    pub max_tilt_speed: u8,
}

// Note: Default implementation removed as Camera now requires a transport

/// Trait for async transports that can send VISCA commands.
#[cfg(feature = "async")]
pub trait CameraTransport: Send + Sync + std::fmt::Debug {
    /// Send a VISCA command and wait for response.
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, Error>> + Send + 'a>>;
}

#[cfg(feature = "async")]
impl<T: AsyncTransport> CameraTransport for ViscaTransport<T> {
    fn send_command<'a>(
        &'a mut self,
        command: &'a dyn Command,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, Error>> + Send + 'a>>
    {
        // We need to extract the command data since we can't borrow it across the async boundary
        let command_bytes = match command.to_bytes() {
            Ok(bytes) => bytes,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let response_type = command.response_type();
        let category = command.command_category();

        // Create an owned command that we can send across the async boundary
        struct OwnedCommand {
            bytes: Vec<u8>,
            response_type: Option<crate::command::ResponseType>,
            category: crate::timeout::CommandCategory,
        }

        impl Command for OwnedCommand {
            fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                Ok(self.bytes.clone())
            }

            fn response_type(&self) -> Option<crate::command::ResponseType> {
                self.response_type
            }

            fn command_category(&self) -> crate::timeout::CommandCategory {
                self.category
            }
        }

        let owned_command = OwnedCommand {
            bytes: command_bytes,
            response_type,
            category,
        };

        Box::pin(async move { self.send_command(&owned_command).await })
    }
}

impl<P: CameraProfile, T> Camera<P, T> {
    /// Create a new camera with a transport.
    pub fn new(transport: T) -> Self {
        Self {
            profile: P::default(),
            transport: ViscaTransport::new(transport),
        }
    }

    /// Create a camera with a custom profile instance.
    pub fn with_profile(transport: T, profile: P) -> Self {
        Self {
            profile,
            transport: ViscaTransport::new(transport),
        }
    }

    /// Get the camera profile.
    pub fn profile(&self) -> &P {
        &self.profile
    }

    /// Get the camera's capabilities.
    pub fn capabilities(&self) -> CameraCapabilities {
        CameraCapabilities {
            model_name: self.profile.model_name().to_string(),
            pan_range_degrees: self.profile.pan_degree_range(),
            tilt_range_degrees: self.profile.tilt_degree_range(),
            zoom_steps: self.profile.zoom_range().count(),
            focus_steps: self.profile.focus_range().count(),
            preset_count: P::max_preset_id(),
            supports_digital_zoom: self.profile.digital_zoom_supported(),
            max_pan_speed: self.profile.max_pan_speed(),
            max_tilt_speed: self.profile.max_tilt_speed(),
        }
    }

    /// Get the full capability summary for this camera.
    pub fn capability_summary(&self) -> CapabilitySummary {
        self.profile.capability_summary()
    }

    /// Get the underlying transport.
    pub fn transport(&self) -> &ViscaTransport<T> {
        &self.transport
    }

    /// Get a mutable reference to the underlying transport.
    pub fn transport_mut(&mut self) -> &mut ViscaTransport<T> {
        &mut self.transport
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P: CameraProfile, T> Camera<P, T>
where
    T: crate::transport::blocking::Transport,
{
    /// Send a VISCA command and wait for response.
    ///
    /// This is a lower-level method for sending custom commands. For standard operations,
    /// prefer the high-level methods like `zoom_in()`, `pan_tilt_home()`, etc.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Camera, Command, Response, Error};
    /// # struct MyCustomCommand;
    /// # impl Command for MyCustomCommand {
    /// #     fn to_bytes(&self) -> Result<Vec<u8>, Error> { Ok(vec![]) }
    /// #     fn response_type(&self) -> Option<grafton_visca::command::ResponseType> { None }
    /// #     fn command_category(&self) -> grafton_visca::timeout::CommandCategory {
    /// #         grafton_visca::timeout::CommandCategory::Movement
    /// #     }
    /// # }
    /// # fn example(camera: &mut Camera<grafton_visca::profiles::PTZOpticsG2>) -> Result<(), Error> {
    /// let custom_command = MyCustomCommand;
    /// let response = camera.send_command(&custom_command)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn send_command(&mut self, command: &dyn Command) -> Result<Response, Error> {
        self.transport.send_command(command)
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P: CameraProfile, T> Camera<P, T>
where
    T: AsyncTransport,
{
    /// Send a VISCA command and wait for response.
    ///
    /// This is a lower-level method for sending custom commands. For standard operations,
    /// prefer the high-level methods like `zoom_in()`, `pan_tilt_home()`, etc.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Camera, Command, Response, Error};
    /// # struct MyCustomCommand;
    /// # impl Command for MyCustomCommand {
    /// #     fn to_bytes(&self) -> Result<Vec<u8>, Error> { Ok(vec![]) }
    /// #     fn response_type(&self) -> Option<grafton_visca::command::ResponseType> { None }
    /// #     fn command_category(&self) -> grafton_visca::timeout::CommandCategory {
    /// #         grafton_visca::timeout::CommandCategory::Movement
    /// #     }
    /// # }
    /// # async fn example(camera: &Camera<grafton_visca::profiles::PTZOpticsG2>) -> Result<(), Error> {
    /// let custom_command = MyCustomCommand;
    /// let response = camera.send_command(&custom_command).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_command(&self, command: &dyn Command) -> Result<Response, Error> {
        self.transport.send_command(command).await
    }
}

impl<P: CameraProfile, T> Camera<P, T> {
    /// Send a command and wait for completion.
    #[cfg(not(feature = "async"))]
    fn send_and_wait(&mut self, command: &dyn Command) -> Result<(), Error> {
        match self.send_command(command)? {
            Response::Completion => Ok(()),
            Response::Ack => {
                // ACK should not be returned as final response with new API
                // Transport handles waiting for completion
                Ok(())
            }
            response => Err(Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }
}

/// Position units for type-safe coordinate handling.
pub mod units {
    /// Position in degrees.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Degrees<T>(pub T);

    /// Position in VISCA protocol units.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct ViscaUnits<T>(pub T);

    /// Normalized position (-1.0 to 1.0).
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Normalized<T>(pub T);

    impl<T> Degrees<T> {
        /// Create a new position in degrees.
        pub fn new(value: T) -> Self {
            Self(value)
        }

        /// Get the inner value.
        pub fn value(&self) -> &T {
            &self.0
        }

        /// Consume and return the inner value.
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> ViscaUnits<T> {
        /// Create a new position in VISCA units.
        pub fn new(value: T) -> Self {
            Self(value)
        }

        /// Get the inner value.
        pub fn value(&self) -> &T {
            &self.0
        }

        /// Consume and return the inner value.
        pub fn into_inner(self) -> T {
            self.0
        }
    }

    impl<T> Normalized<T> {
        /// Create a new normalized position.
        pub fn new(value: T) -> Self {
            Self(value)
        }

        /// Get the inner value.
        pub fn value(&self) -> &T {
            &self.0
        }

        /// Consume and return the inner value.
        pub fn into_inner(self) -> T {
            self.0
        }
    }
}
