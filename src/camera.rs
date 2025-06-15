//! Camera profile system for type-safe, model-specific VISCA control.

use std::fmt::Display;
use std::ops::RangeInclusive;

use crate::error::Error as ViscaError;
use crate::transport::Transport;

pub mod commands;
pub mod profiles;

/// Core camera abstraction with compile-time profile information.
pub struct Camera<P: CameraProfile> {
    profile: P,
    transport: Box<dyn Transport>,
}

impl<P: CameraProfile> std::fmt::Debug for Camera<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Camera")
            .field("profile", &self.profile)
            .field("model", &P::MODEL_NAME)
            .finish()
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

    /// Get the total pan range in degrees.
    fn pan_degree_range(&self) -> RangeInclusive<f32> {
        let min = self.pan_units_to_degrees(*Self::PAN_RANGE.start());
        let max = self.pan_units_to_degrees(*Self::PAN_RANGE.end());
        min..=max
    }

    /// Get the total tilt range in degrees.
    fn tilt_degree_range(&self) -> RangeInclusive<f32> {
        let min = self.tilt_units_to_degrees(*Self::TILT_RANGE.start());
        let max = self.tilt_units_to_degrees(*Self::TILT_RANGE.end());
        min..=max
    }

    /// Associated type for camera-specific preset IDs.
    type PresetId: Into<u8> + TryFrom<u8, Error = ViscaError> + Copy + Display;

    /// Associated type for camera-specific gain values.
    type GainValue: Into<u8> + TryFrom<u8, Error = ViscaError> + Copy + Display;

    /// Get the maximum preset ID for this camera.
    fn max_preset_id() -> u8;
}

/// Runtime camera capabilities for discovery and validation.
#[derive(Debug, Clone)]
pub struct CameraCapabilities {
    /// Camera model name.
    pub model_name: &'static str,
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

impl<P: CameraProfile> Camera<P> {
    /// Create a new camera with the given transport.
    pub fn new(transport: impl Transport + 'static) -> Self {
        Self {
            profile: P::default(),
            transport: Box::new(transport),
        }
    }

    /// Create a camera with a custom profile instance.
    pub fn with_profile(transport: impl Transport + 'static, profile: P) -> Self {
        Self {
            profile,
            transport: Box::new(transport),
        }
    }

    /// Get the camera's capabilities.
    pub fn capabilities(&self) -> CameraCapabilities {
        CameraCapabilities {
            model_name: P::MODEL_NAME,
            pan_range_degrees: self.profile.pan_degree_range(),
            tilt_range_degrees: self.profile.tilt_degree_range(),
            zoom_steps: P::ZOOM_RANGE.clone().count(),
            focus_steps: P::FOCUS_RANGE.clone().count(),
            preset_count: P::max_preset_id(),
            supports_digital_zoom: P::DIGITAL_ZOOM_SUPPORTED,
            max_pan_speed: P::MAX_PAN_SPEED,
            max_tilt_speed: P::MAX_TILT_SPEED,
        }
    }

    /// Get a reference to the transport.
    pub fn transport(&self) -> &dyn Transport {
        &*self.transport
    }

    /// Get a mutable reference to the transport.
    pub fn transport_mut(&mut self) -> &mut dyn Transport {
        &mut *self.transport
    }

    /// Get the camera profile.
    pub fn profile(&self) -> &P {
        &self.profile
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
