//! Builder pattern for creating custom camera profiles.

use std::marker::PhantomData;
use std::ops::RangeInclusive;

use super::{profiles::GenericGain, profiles::GenericPresetId, CameraProfile};

/// Builder for creating custom camera profiles with specific capabilities.
///
/// This allows runtime configuration of camera capabilities when the exact
/// model is unknown or when working with custom VISCA implementations.
///
/// # Example
/// ```no_run
/// # use grafton_visca::camera::CustomProfileBuilder;
/// let profile = CustomProfileBuilder::new("Custom PTZ Camera")
///     .pan_range(-2000..=2000)
///     .tilt_range(-500..=1500)
///     .zoom_range(0x0000..=0x8000)
///     .focus_range(0x1000..=0xF000)
///     .pan_speed(20)
///     .tilt_speed(16)
///     .digital_zoom(true)
///     .pan_degrees_range(-170.0..=170.0)
///     .tilt_degrees_range(-30.0..=90.0)
///     .max_preset_id(99)
///     .build();
/// ```
#[derive(Debug, Clone)]
pub struct CustomProfileBuilder {
    model_name: String,
    pan_range: RangeInclusive<i16>,
    tilt_range: RangeInclusive<i16>,
    zoom_range: RangeInclusive<u16>,
    focus_range: RangeInclusive<u16>,
    digital_zoom_supported: bool,
    max_pan_speed: u8,
    max_tilt_speed: u8,
    pan_degrees_range: Option<RangeInclusive<f32>>,
    tilt_degrees_range: Option<RangeInclusive<f32>>,
    max_preset_id: u8,
}

impl CustomProfileBuilder {
    /// Create a new custom profile builder with the given model name.
    pub fn new(model_name: impl Into<String>) -> Self {
        Self {
            model_name: model_name.into(),
            // Default to generic VISCA ranges
            pan_range: -32768..=32767,
            tilt_range: -32768..=32767,
            zoom_range: 0x0000..=0xFFFF,
            focus_range: 0x0000..=0xFFFF,
            digital_zoom_supported: false,
            max_pan_speed: 24,
            max_tilt_speed: 20,
            pan_degrees_range: None,
            tilt_degrees_range: None,
            max_preset_id: 255,
        }
    }

    /// Set the pan range in VISCA units.
    pub fn pan_range(mut self, range: RangeInclusive<i16>) -> Self {
        self.pan_range = range;
        self
    }

    /// Set the tilt range in VISCA units.
    pub fn tilt_range(mut self, range: RangeInclusive<i16>) -> Self {
        self.tilt_range = range;
        self
    }

    /// Set the zoom range in VISCA units.
    pub fn zoom_range(mut self, range: RangeInclusive<u16>) -> Self {
        self.zoom_range = range;
        self
    }

    /// Set the focus range in VISCA units.
    pub fn focus_range(mut self, range: RangeInclusive<u16>) -> Self {
        self.focus_range = range;
        self
    }

    /// Set whether digital zoom is supported.
    pub fn digital_zoom(mut self, supported: bool) -> Self {
        self.digital_zoom_supported = supported;
        self
    }

    /// Set the maximum pan speed (1-24).
    pub fn pan_speed(mut self, speed: u8) -> Self {
        self.max_pan_speed = speed.clamp(1, 24);
        self
    }

    /// Set the maximum tilt speed (1-20).
    pub fn tilt_speed(mut self, speed: u8) -> Self {
        self.max_tilt_speed = speed.clamp(1, 20);
        self
    }

    /// Set the pan range in degrees for accurate conversion.
    ///
    /// If not set, assumes ±180 degrees.
    pub fn pan_degrees_range(mut self, range: RangeInclusive<f32>) -> Self {
        self.pan_degrees_range = Some(range);
        self
    }

    /// Set the tilt range in degrees for accurate conversion.
    ///
    /// If not set, assumes ±90 degrees.
    pub fn tilt_degrees_range(mut self, range: RangeInclusive<f32>) -> Self {
        self.tilt_degrees_range = Some(range);
        self
    }

    /// Set the maximum preset ID (0-255).
    pub fn max_preset_id(mut self, max_id: u8) -> Self {
        self.max_preset_id = max_id;
        self
    }

    /// Build the custom camera profile.
    pub fn build(self) -> CustomProfile {
        CustomProfile {
            model_name: self.model_name,
            pan_range: self.pan_range,
            tilt_range: self.tilt_range,
            zoom_range: self.zoom_range,
            focus_range: self.focus_range,
            digital_zoom_supported: self.digital_zoom_supported,
            max_pan_speed: self.max_pan_speed,
            max_tilt_speed: self.max_tilt_speed,
            pan_degrees_range: self.pan_degrees_range.unwrap_or(-180.0..=180.0),
            tilt_degrees_range: self.tilt_degrees_range.unwrap_or(-90.0..=90.0),
            max_preset_id: self.max_preset_id,
        }
    }
}

/// A custom camera profile created at runtime.
///
/// This profile can be used with the `Camera` struct to control cameras
/// with custom or unknown specifications.
#[derive(Debug, Clone)]
pub struct CustomProfile {
    model_name: String,
    pan_range: RangeInclusive<i16>,
    tilt_range: RangeInclusive<i16>,
    zoom_range: RangeInclusive<u16>,
    focus_range: RangeInclusive<u16>,
    digital_zoom_supported: bool,
    max_pan_speed: u8,
    max_tilt_speed: u8,
    pan_degrees_range: RangeInclusive<f32>,
    tilt_degrees_range: RangeInclusive<f32>,
    max_preset_id: u8,
}

impl Default for CustomProfile {
    fn default() -> Self {
        Self {
            model_name: "Custom VISCA Camera".to_string(),
            pan_range: -32768..=32767,
            tilt_range: -32768..=32767,
            zoom_range: 0x0000..=0xFFFF,
            focus_range: 0x0000..=0xFFFF,
            digital_zoom_supported: false,
            max_pan_speed: 24,
            max_tilt_speed: 20,
            pan_degrees_range: -180.0..=180.0,
            tilt_degrees_range: -90.0..=90.0,
            max_preset_id: 255,
        }
    }
}

impl CustomProfile {
    /// Create a new builder for a custom profile.
    pub fn builder(model_name: impl Into<String>) -> CustomProfileBuilder {
        CustomProfileBuilder::new(model_name)
    }
}

impl CameraProfile for CustomProfile {
    const MODEL_NAME: &'static str = "Custom";
    const PAN_RANGE: RangeInclusive<i16> = -32768..=32767;
    const TILT_RANGE: RangeInclusive<i16> = -32768..=32767;
    const ZOOM_RANGE: RangeInclusive<u16> = 0x0000..=0xFFFF;
    const FOCUS_RANGE: RangeInclusive<u16> = 0x0000..=0xFFFF;
    const DIGITAL_ZOOM_SUPPORTED: bool = false;
    const MAX_PAN_SPEED: u8 = 24;
    const MAX_TILT_SPEED: u8 = 20;

    type PresetId = GenericPresetId;
    type GainValue = GenericGain;

    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn pan_range(&self) -> RangeInclusive<i16> {
        self.pan_range.clone()
    }

    fn tilt_range(&self) -> RangeInclusive<i16> {
        self.tilt_range.clone()
    }

    fn zoom_range(&self) -> RangeInclusive<u16> {
        self.zoom_range.clone()
    }

    fn focus_range(&self) -> RangeInclusive<u16> {
        self.focus_range.clone()
    }

    fn digital_zoom_supported(&self) -> bool {
        self.digital_zoom_supported
    }

    fn max_pan_speed(&self) -> u8 {
        self.max_pan_speed
    }

    fn max_tilt_speed(&self) -> u8 {
        self.max_tilt_speed
    }

    fn pan_units_to_degrees(&self, units: i16) -> f32 {
        let pan_range_units = *self.pan_range.end() - *self.pan_range.start();
        let pan_range_degrees = *self.pan_degrees_range.end() - *self.pan_degrees_range.start();
        let normalized = (units - self.pan_range.start()) as f32 / pan_range_units as f32;
        normalized * pan_range_degrees + *self.pan_degrees_range.start()
    }

    fn tilt_units_to_degrees(&self, units: i16) -> f32 {
        let tilt_range_units = *self.tilt_range.end() - *self.tilt_range.start();
        let tilt_range_degrees = *self.tilt_degrees_range.end() - *self.tilt_degrees_range.start();
        let normalized = (units - self.tilt_range.start()) as f32 / tilt_range_units as f32;
        normalized * tilt_range_degrees + *self.tilt_degrees_range.start()
    }

    fn pan_degrees_to_units(&self, degrees: f32) -> i16 {
        let pan_range_degrees = *self.pan_degrees_range.end() - *self.pan_degrees_range.start();
        let pan_range_units = *self.pan_range.end() - *self.pan_range.start();
        let normalized = (degrees - *self.pan_degrees_range.start()) / pan_range_degrees;
        (normalized * pan_range_units as f32 + *self.pan_range.start() as f32).round() as i16
    }

    fn tilt_degrees_to_units(&self, degrees: f32) -> i16 {
        let tilt_range_degrees = *self.tilt_degrees_range.end() - *self.tilt_degrees_range.start();
        let tilt_range_units = *self.tilt_range.end() - *self.tilt_range.start();
        let normalized = (degrees - *self.tilt_degrees_range.start()) / tilt_range_degrees;
        (normalized * tilt_range_units as f32 + *self.tilt_range.start() as f32).round() as i16
    }

    fn max_preset_id() -> u8 {
        255 // Will be overridden by instance method
    }
}

// Since we need instance data for max_preset_id, we add this extension
impl CustomProfile {
    /// Get the maximum preset ID for this custom profile.
    pub fn get_max_preset_id(&self) -> u8 {
        self.max_preset_id
    }
}

/// A typed builder state for ensuring required fields are set.
#[derive(Debug)]
pub struct CustomProfileTypedBuilder<Model, Pan, Tilt, Zoom, Focus> {
    model_name: Model,
    pan_range: Pan,
    tilt_range: Tilt,
    zoom_range: Zoom,
    focus_range: Focus,
    digital_zoom_supported: bool,
    max_pan_speed: u8,
    max_tilt_speed: u8,
    pan_degrees_range: Option<RangeInclusive<f32>>,
    tilt_degrees_range: Option<RangeInclusive<f32>>,
    max_preset_id: u8,
    _phantom: PhantomData<(Model, Pan, Tilt, Zoom, Focus)>,
}

/// Marker type for unset fields
#[derive(Debug, Clone, Copy)]
pub struct Unset;
/// Marker type for set fields
#[derive(Debug)]
pub struct Set<T>(T);

impl CustomProfileTypedBuilder<Unset, Unset, Unset, Unset, Unset> {
    /// Create a new typed builder.
    pub fn new() -> Self {
        Self {
            model_name: Unset,
            pan_range: Unset,
            tilt_range: Unset,
            zoom_range: Unset,
            focus_range: Unset,
            digital_zoom_supported: false,
            max_pan_speed: 24,
            max_tilt_speed: 20,
            pan_degrees_range: None,
            tilt_degrees_range: None,
            max_preset_id: 255,
            _phantom: PhantomData,
        }
    }
}

impl Default for CustomProfileTypedBuilder<Unset, Unset, Unset, Unset, Unset> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Model, Pan, Tilt, Zoom, Focus> CustomProfileTypedBuilder<Model, Pan, Tilt, Zoom, Focus> {
    /// Set the model name (required).
    pub fn model_name(
        self,
        name: impl Into<String>,
    ) -> CustomProfileTypedBuilder<Set<String>, Pan, Tilt, Zoom, Focus> {
        CustomProfileTypedBuilder {
            model_name: Set(name.into()),
            pan_range: self.pan_range,
            tilt_range: self.tilt_range,
            zoom_range: self.zoom_range,
            focus_range: self.focus_range,
            digital_zoom_supported: self.digital_zoom_supported,
            max_pan_speed: self.max_pan_speed,
            max_tilt_speed: self.max_tilt_speed,
            pan_degrees_range: self.pan_degrees_range,
            tilt_degrees_range: self.tilt_degrees_range,
            max_preset_id: self.max_preset_id,
            _phantom: PhantomData,
        }
    }

    /// Set the pan range (required).
    pub fn pan_range(
        self,
        range: RangeInclusive<i16>,
    ) -> CustomProfileTypedBuilder<Model, Set<RangeInclusive<i16>>, Tilt, Zoom, Focus> {
        CustomProfileTypedBuilder {
            model_name: self.model_name,
            pan_range: Set(range),
            tilt_range: self.tilt_range,
            zoom_range: self.zoom_range,
            focus_range: self.focus_range,
            digital_zoom_supported: self.digital_zoom_supported,
            max_pan_speed: self.max_pan_speed,
            max_tilt_speed: self.max_tilt_speed,
            pan_degrees_range: self.pan_degrees_range,
            tilt_degrees_range: self.tilt_degrees_range,
            max_preset_id: self.max_preset_id,
            _phantom: PhantomData,
        }
    }

    /// Set the tilt range (required).
    pub fn tilt_range(
        self,
        range: RangeInclusive<i16>,
    ) -> CustomProfileTypedBuilder<Model, Pan, Set<RangeInclusive<i16>>, Zoom, Focus> {
        CustomProfileTypedBuilder {
            model_name: self.model_name,
            pan_range: self.pan_range,
            tilt_range: Set(range),
            zoom_range: self.zoom_range,
            focus_range: self.focus_range,
            digital_zoom_supported: self.digital_zoom_supported,
            max_pan_speed: self.max_pan_speed,
            max_tilt_speed: self.max_tilt_speed,
            pan_degrees_range: self.pan_degrees_range,
            tilt_degrees_range: self.tilt_degrees_range,
            max_preset_id: self.max_preset_id,
            _phantom: PhantomData,
        }
    }

    /// Set the zoom range (required).
    pub fn zoom_range(
        self,
        range: RangeInclusive<u16>,
    ) -> CustomProfileTypedBuilder<Model, Pan, Tilt, Set<RangeInclusive<u16>>, Focus> {
        CustomProfileTypedBuilder {
            model_name: self.model_name,
            pan_range: self.pan_range,
            tilt_range: self.tilt_range,
            zoom_range: Set(range),
            focus_range: self.focus_range,
            digital_zoom_supported: self.digital_zoom_supported,
            max_pan_speed: self.max_pan_speed,
            max_tilt_speed: self.max_tilt_speed,
            pan_degrees_range: self.pan_degrees_range,
            tilt_degrees_range: self.tilt_degrees_range,
            max_preset_id: self.max_preset_id,
            _phantom: PhantomData,
        }
    }

    /// Set the focus range (required).
    pub fn focus_range(
        self,
        range: RangeInclusive<u16>,
    ) -> CustomProfileTypedBuilder<Model, Pan, Tilt, Zoom, Set<RangeInclusive<u16>>> {
        CustomProfileTypedBuilder {
            model_name: self.model_name,
            pan_range: self.pan_range,
            tilt_range: self.tilt_range,
            zoom_range: self.zoom_range,
            focus_range: Set(range),
            digital_zoom_supported: self.digital_zoom_supported,
            max_pan_speed: self.max_pan_speed,
            max_tilt_speed: self.max_tilt_speed,
            pan_degrees_range: self.pan_degrees_range,
            tilt_degrees_range: self.tilt_degrees_range,
            max_preset_id: self.max_preset_id,
            _phantom: PhantomData,
        }
    }
}

// Optional setters available at any stage
impl<Model, Pan, Tilt, Zoom, Focus> CustomProfileTypedBuilder<Model, Pan, Tilt, Zoom, Focus> {
    /// Set whether digital zoom is supported.
    pub fn digital_zoom(mut self, supported: bool) -> Self {
        self.digital_zoom_supported = supported;
        self
    }

    /// Set the maximum pan speed (1-24).
    pub fn pan_speed(mut self, speed: u8) -> Self {
        self.max_pan_speed = speed.clamp(1, 24);
        self
    }

    /// Set the maximum tilt speed (1-20).
    pub fn tilt_speed(mut self, speed: u8) -> Self {
        self.max_tilt_speed = speed.clamp(1, 20);
        self
    }

    /// Set the pan range in degrees for accurate conversion.
    pub fn pan_degrees_range(mut self, range: RangeInclusive<f32>) -> Self {
        self.pan_degrees_range = Some(range);
        self
    }

    /// Set the tilt range in degrees for accurate conversion.
    pub fn tilt_degrees_range(mut self, range: RangeInclusive<f32>) -> Self {
        self.tilt_degrees_range = Some(range);
        self
    }

    /// Set the maximum preset ID (0-255).
    pub fn max_preset_id(mut self, max_id: u8) -> Self {
        self.max_preset_id = max_id;
        self
    }
}

// Build is only available when all required fields are set
impl
    CustomProfileTypedBuilder<
        Set<String>,
        Set<RangeInclusive<i16>>,
        Set<RangeInclusive<i16>>,
        Set<RangeInclusive<u16>>,
        Set<RangeInclusive<u16>>,
    >
{
    /// Build the custom camera profile.
    pub fn build(self) -> CustomProfile {
        CustomProfile {
            model_name: self.model_name.0,
            pan_range: self.pan_range.0,
            tilt_range: self.tilt_range.0,
            zoom_range: self.zoom_range.0,
            focus_range: self.focus_range.0,
            digital_zoom_supported: self.digital_zoom_supported,
            max_pan_speed: self.max_pan_speed,
            max_tilt_speed: self.max_tilt_speed,
            pan_degrees_range: self.pan_degrees_range.unwrap_or(-180.0..=180.0),
            tilt_degrees_range: self.tilt_degrees_range.unwrap_or(-90.0..=90.0),
            max_preset_id: self.max_preset_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_profile_builder() {
        let profile = CustomProfileBuilder::new("Test Camera")
            .pan_range(-1000..=1000)
            .tilt_range(-500..=500)
            .zoom_range(0x0000..=0x4000)
            .focus_range(0x1000..=0x8000)
            .digital_zoom(true)
            .pan_speed(18)
            .tilt_speed(14)
            .pan_degrees_range(-170.0..=170.0)
            .tilt_degrees_range(-30.0..=90.0)
            .max_preset_id(99)
            .build();

        assert_eq!(profile.model_name(), "Test Camera");
        assert_eq!(profile.pan_range(), -1000..=1000);
        assert_eq!(profile.tilt_range(), -500..=500);
        assert_eq!(profile.zoom_range(), 0x0000..=0x4000);
        assert_eq!(profile.focus_range(), 0x1000..=0x8000);
        assert!(profile.digital_zoom_supported());
        assert_eq!(profile.max_pan_speed(), 18);
        assert_eq!(profile.max_tilt_speed(), 14);
        assert_eq!(profile.get_max_preset_id(), 99);
    }

    #[test]
    fn test_typed_builder() {
        let profile = CustomProfileTypedBuilder::new()
            .model_name("Typed Test Camera")
            .pan_range(-2000..=2000)
            .tilt_range(-1000..=1000)
            .zoom_range(0x0000..=0x8000)
            .focus_range(0x2000..=0xC000)
            .digital_zoom(false)
            .pan_speed(20)
            .tilt_speed(16)
            .build();

        assert_eq!(profile.model_name(), "Typed Test Camera");
        assert_eq!(profile.pan_range(), -2000..=2000);
        assert!(!profile.digital_zoom_supported());
    }

    #[test]
    fn test_unit_conversion() {
        let profile = CustomProfileBuilder::new("Conversion Test")
            .pan_range(-1000..=1000)
            .tilt_range(-500..=500)
            .pan_degrees_range(-90.0..=90.0)
            .tilt_degrees_range(-45.0..=45.0)
            .build();

        // Test pan conversion
        assert_eq!(profile.pan_units_to_degrees(0), 0.0);
        assert_eq!(profile.pan_units_to_degrees(1000), 90.0);
        assert_eq!(profile.pan_units_to_degrees(-1000), -90.0);

        // Test reverse conversion
        assert_eq!(profile.pan_degrees_to_units(0.0), 0);
        assert_eq!(profile.pan_degrees_to_units(90.0), 1000);
        assert_eq!(profile.pan_degrees_to_units(-90.0), -1000);

        // Test tilt conversion
        assert_eq!(profile.tilt_units_to_degrees(0), 0.0);
        assert_eq!(profile.tilt_units_to_degrees(500), 45.0);
        assert_eq!(profile.tilt_units_to_degrees(-500), -45.0);
    }
}
