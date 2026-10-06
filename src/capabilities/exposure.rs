//! Exposure capability trait and associated types.

use crate::{
    capabilities::{CapabilityDomain, CapabilityRange, SupportedRange, ValidationError},
    command::exposure::ExposureMode,
    units::Fraction,
};

/// The five shared AE modes (`04 39`): auto, manual, shutter priority, iris
/// priority, and bright. This is the [`Exposure::EXPOSURE_MODES`] default and
/// the inventory of every built-in profile that supports the full family.
pub(crate) const STANDARD_EXPOSURE_MODES: &[ExposureMode] = &[
    ExposureMode::Auto,
    ExposureMode::Manual,
    ExposureMode::Shutter,
    ExposureMode::Iris,
    ExposureMode::Bright,
];

/// Trait for cameras that support exposure control.
///
/// This trait defines the constants and capabilities for exposure settings
/// including exposure modes, iris, shutter speed, gain, and exposure compensation.
pub trait Exposure {
    /// Supported exposure modes for this camera profile.
    const EXPOSURE_MODES: &'static [ExposureMode] = STANDARD_EXPOSURE_MODES;

    /// Iris positions in VISCA units, if iris control is supported: the
    /// bounds of the profile's iris table minus any position it does not list.
    const IRIS_RANGE: Option<CapabilityDomain<u16>>;

    /// Supported shutter speeds as VISCA values.
    /// Each camera model has specific supported speeds.
    const SHUTTER_SPEEDS: &'static [ShutterSpeedEntry];

    /// Valid range for gain values.
    const GAIN_RANGE: CapabilityRange<u8>;

    /// VISCA exposure bright positions, if supported: the bounds of the
    /// profile's Bright table minus any position it does not list.
    ///
    /// This is the exposure bright control (`0x04 0x0D` / `0x04 0x4D`), not
    /// image luminance (`0x04 0xA1`).
    const BRIGHTNESS_RANGE: Option<CapabilityDomain<u8>> = None;

    /// Whether camera supports backlight compensation.
    const SUPPORTS_BACKLIGHT_COMP: bool;

    /// Exposure-compensation levels, or `None` when exposure compensation is
    /// not supported. Typically -7 to +7.
    const EXPOSURE_COMP_RANGE: Option<CapabilityRange<i8>> = None;

    /// Whether camera supports wide dynamic range.
    const SUPPORTS_WDR: bool = false;
}

/// Extension trait that adds validation methods to cameras with exposure support.
pub trait ExposureExt: Exposure {
    /// Returns true when the profile supports the requested exposure mode.
    fn supports_exposure_mode(&self, mode: ExposureMode) -> bool {
        Self::EXPOSURE_MODES.contains(&mode)
    }

    /// Returns true when the profile supports direct iris control.
    fn supports_iris_control(&self) -> bool {
        Self::IRIS_RANGE.is_some()
    }

    /// Validate iris value is within range.
    fn validate_iris(&self, iris: u16) -> Result<u16, ValidationError> {
        Self::IRIS_RANGE.validate_supported("iris", iris)
    }

    /// Validate gain value is within range.
    fn validate_gain(&self, gain: u8) -> Result<u8, ValidationError> {
        Self::GAIN_RANGE.validate("gain", gain)
    }

    /// Validate exposure brightness value is within range.
    fn validate_brightness(&self, brightness: u8) -> Result<u8, ValidationError> {
        Self::BRIGHTNESS_RANGE.validate_supported("exposure brightness", brightness)
    }

    /// Validate shutter speed is supported.
    fn validate_shutter_speed(&self, value: u8) -> Result<u8, ValidationError> {
        if Self::SHUTTER_SPEEDS.iter().any(|s| s.value == value) {
            Ok(value)
        } else {
            Err(ValidationError::invalid_value(
                "shutter speed",
                format!("Unsupported shutter speed value: {value}"),
            ))
        }
    }

    /// Validate exposure compensation value.
    ///
    /// Note: This method should only be called on profiles that support exposure compensation.
    /// The compile-time check is enforced by requiring HasExposureCompensation marker trait
    /// on the methods that use exposure compensation.
    fn validate_exposure_comp(&self, value: i8) -> Result<i8, ValidationError> {
        Self::EXPOSURE_COMP_RANGE.validate_supported("exposure compensation", value)
    }
}

// Automatic implementation for all types that support exposure
impl<T: Exposure> ExposureExt for T {}

/// One entry of a profile's shutter-speed table: an exposure time and the
/// profile-specific wire code that selects it.
///
/// Compile-time profiles list these in [`Exposure::SHUTTER_SPEEDS`] and
/// runtime [`Capabilities::shutter_speeds`](crate::capabilities::Capabilities::shutter_speeds)
/// holds the same entries. The wire code itself is the
/// [`crate::types::ShutterSpeed`] value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ShutterSpeedEntry {
    /// Exposure time selected by this code, such as `1/60`; serialized as
    /// `"1/60"`.
    pub exposure: Fraction,
    /// VISCA shutter code.
    pub value: u8,
}

impl ShutterSpeedEntry {
    /// Creates a table entry.
    #[must_use]
    pub const fn new(exposure: Fraction, value: u8) -> Self {
        Self { exposure, value }
    }
}

// Note: ExposureMode and DynamicRangeLevel enums are defined in the command module
// and re-exported from the crate root. This avoids duplication.

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[allow(clippy::panic)]
    const fn entry(denominator: u32, value: u8) -> ShutterSpeedEntry {
        match Fraction::new(1, denominator) {
            Some(exposure) => ShutterSpeedEntry::new(exposure, value),
            None => panic!("shutter denominators are nonzero"),
        }
    }

    const TEST_SHUTTER_SPEEDS: &[ShutterSpeedEntry] = &[
        entry(30, 0x00),
        entry(60, 0x01),
        entry(100, 0x02),
        entry(250, 0x03),
        entry(500, 0x04),
        entry(1000, 0x05),
    ];

    struct TestCamera;

    impl Exposure for TestCamera {
        const IRIS_RANGE: Option<CapabilityDomain<u16>> = Some(CapabilityDomain::<u16>::with_gaps(
            0x00,
            0x1C,
            &[0x01, 0x02],
        ));
        const SHUTTER_SPEEDS: &'static [ShutterSpeedEntry] = TEST_SHUTTER_SPEEDS;
        const GAIN_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(0, 15);
        const BRIGHTNESS_RANGE: Option<CapabilityDomain<u8>> =
            Some(CapabilityDomain::<u8>::new(0, 17));
        const SUPPORTS_BACKLIGHT_COMP: bool = true;
        const EXPOSURE_COMP_RANGE: Option<CapabilityRange<i8>> =
            Some(CapabilityRange::<i8>::new(-7, 7));
    }

    struct NoIrisCamera;

    impl Exposure for NoIrisCamera {
        const EXPOSURE_MODES: &'static [ExposureMode] = &[ExposureMode::Auto, ExposureMode::Manual];
        const IRIS_RANGE: Option<CapabilityDomain<u16>> = None;
        const SHUTTER_SPEEDS: &'static [ShutterSpeedEntry] = TEST_SHUTTER_SPEEDS;
        const GAIN_RANGE: CapabilityRange<u8> = CapabilityRange::<u8>::new(0, 15);
        const BRIGHTNESS_RANGE: Option<CapabilityDomain<u8>> = None;
        const SUPPORTS_BACKLIGHT_COMP: bool = true;
    }

    #[test]
    fn test_iris_validation() {
        let camera = TestCamera;

        assert!(camera.validate_iris(0x00).is_ok());
        assert!(camera.validate_iris(0x03).is_ok());
        assert!(camera.validate_iris(0x1C).is_ok());
        assert!(camera.validate_iris(0x1D).is_err());
        // A gap in the iris table is refused even though it is inside the bounds.
        assert!(matches!(
            camera.validate_iris(0x01),
            Err(ValidationError::InvalidValue { .. })
        ));
    }

    #[test]
    fn test_iris_validation_rejects_unsupported_profiles() {
        let camera = NoIrisCamera;

        assert_eq!(
            camera.validate_iris(0x00),
            Err(ValidationError::NotSupported("iris"))
        );
    }

    #[test]
    fn test_exposure_mode_support() {
        let camera = TestCamera;
        assert!(camera.supports_exposure_mode(ExposureMode::Iris));
        assert!(camera.supports_iris_control());

        let camera = NoIrisCamera;
        assert!(!camera.supports_exposure_mode(ExposureMode::Iris));
        assert!(!camera.supports_iris_control());
    }

    #[test]
    fn test_brightness_validation() {
        let camera = TestCamera;

        assert!(camera.validate_brightness(0).is_ok());
        assert!(camera.validate_brightness(17).is_ok());
        assert!(camera.validate_brightness(18).is_err());

        let camera = NoIrisCamera;
        assert_eq!(
            camera.validate_brightness(0),
            Err(ValidationError::NotSupported("exposure brightness"))
        );
    }

    #[test]
    fn test_shutter_speed_validation() {
        let camera = TestCamera;

        assert_eq!(camera.validate_shutter_speed(0x01), Ok(0x01));
        assert!(camera.validate_shutter_speed(0x10).is_err());
    }
}
