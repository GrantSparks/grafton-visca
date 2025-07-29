//! White balance capability trait and associated types.

use std::ops::Range;

use crate::capabilities::ValidationError;
use crate::WhiteBalanceMode;

/// Trait for cameras that support white balance control.
///
/// This trait defines the constants and capabilities for white balance settings
/// including mode selection, color temperature, and RGB gain control.
pub trait WhiteBalance {
    /// Supported white balance modes.
    const WB_MODES: &'static [WhiteBalanceMode];

    /// Whether camera supports one-push white balance.
    /// This samples current scene and sets optimal white balance.
    const SUPPORTS_ONE_PUSH_WB: bool;

    /// Red/Green tuning range for manual adjustment.
    /// None if not supported.
    const RG_TUNING_RANGE: Option<Range<i8>>;

    /// Blue/Green tuning range for manual adjustment.
    /// None if not supported.
    const BG_TUNING_RANGE: Option<Range<i8>>;

    /// Whether camera supports direct color temperature setting.
    const SUPPORTS_COLOR_TEMP: bool = false;

    /// Color temperature range in Kelvin if supported.
    const COLOR_TEMP_RANGE: Option<Range<u16>> = None;

    /// Whether camera supports manual RGB gain control.
    const SUPPORTS_RGB_GAIN: bool = false;

    /// Red gain range if supported.
    const RED_GAIN_RANGE: Option<Range<u8>> = None;

    /// Blue gain range if supported.
    const BLUE_GAIN_RANGE: Option<Range<u8>> = None;
}

/// Extension trait that adds validation methods to cameras with white balance support.
pub trait WhiteBalanceExt: WhiteBalance {
    /// Check if a white balance mode is supported.
    fn supports_wb_mode(&self, mode: WhiteBalanceMode) -> bool {
        Self::WB_MODES.contains(&mode)
    }

    /// Validate white balance mode.
    fn validate_wb_mode(
        &self,
        mode: WhiteBalanceMode,
    ) -> Result<WhiteBalanceMode, ValidationError> {
        if self.supports_wb_mode(mode) {
            Ok(mode)
        } else {
            Err(ValidationError::InvalidValue {
                parameter: "white balance mode",
                message: format!("Mode {:?} not supported", mode),
            })
        }
    }

    /// Validate RG tuning value.
    fn validate_rg_tuning(&self, value: i8) -> Result<i8, ValidationError> {
        match Self::RG_TUNING_RANGE {
            Some(ref range) if range.contains(&value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "RG tuning",
                value: value as f64,
                min: range.start as f64,
                max: (range.end - 1) as f64,
            }),
            None => Err(ValidationError::NotSupported("RG tuning")),
        }
    }

    /// Validate BG tuning value.
    fn validate_bg_tuning(&self, value: i8) -> Result<i8, ValidationError> {
        match Self::BG_TUNING_RANGE {
            Some(ref range) if range.contains(&value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "BG tuning",
                value: value as f64,
                min: range.start as f64,
                max: (range.end - 1) as f64,
            }),
            None => Err(ValidationError::NotSupported("BG tuning")),
        }
    }

    /// Validate color temperature in Kelvin.
    fn validate_color_temp(&self, kelvin: u16) -> Result<u16, ValidationError> {
        if !Self::SUPPORTS_COLOR_TEMP {
            return Err(ValidationError::NotSupported("color temperature"));
        }

        match Self::COLOR_TEMP_RANGE {
            Some(ref range) if range.contains(&kelvin) => Ok(kelvin),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "color temperature",
                value: kelvin as f64,
                min: range.start as f64,
                max: (range.end - 1) as f64,
            }),
            None => Err(ValidationError::NotSupported("color temperature")),
        }
    }

    /// Validate red gain value.
    fn validate_red_gain(&self, gain: u8) -> Result<u8, ValidationError> {
        if !Self::SUPPORTS_RGB_GAIN {
            return Err(ValidationError::NotSupported("RGB gain"));
        }

        match Self::RED_GAIN_RANGE {
            Some(ref range) if range.contains(&gain) => Ok(gain),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "red gain",
                value: gain as f64,
                min: range.start as f64,
                max: (range.end - 1) as f64,
            }),
            None => Err(ValidationError::NotSupported("red gain")),
        }
    }

    /// Validate blue gain value.
    fn validate_blue_gain(&self, gain: u8) -> Result<u8, ValidationError> {
        if !Self::SUPPORTS_RGB_GAIN {
            return Err(ValidationError::NotSupported("RGB gain"));
        }

        match Self::BLUE_GAIN_RANGE {
            Some(ref range) if range.contains(&gain) => Ok(gain),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "blue gain",
                value: gain as f64,
                min: range.start as f64,
                max: (range.end - 1) as f64,
            }),
            None => Err(ValidationError::NotSupported("blue gain")),
        }
    }
}

// Automatic implementation for all types that support white balance
impl<T: WhiteBalance> WhiteBalanceExt for T {}

// Note: WhiteBalanceMode enum is defined in the command module
// and re-exported from the crate root. This avoids duplication.

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_WB_MODES: &[WhiteBalanceMode] = &[
        WhiteBalanceMode::Auto,
        WhiteBalanceMode::Indoor,
        WhiteBalanceMode::Outdoor,
        WhiteBalanceMode::OnePush,
        WhiteBalanceMode::Manual,
    ];

    struct TestCamera;

    impl WhiteBalance for TestCamera {
        const WB_MODES: &'static [WhiteBalanceMode] = TEST_WB_MODES;
        const SUPPORTS_ONE_PUSH_WB: bool = true;
        const RG_TUNING_RANGE: Option<Range<i8>> = Some(-7..8);
        const BG_TUNING_RANGE: Option<Range<i8>> = Some(-7..8);
        const SUPPORTS_COLOR_TEMP: bool = true;
        const COLOR_TEMP_RANGE: Option<Range<u16>> = Some(2800..7500);
    }

    #[test]
    fn test_wb_mode_validation() {
        let camera = TestCamera;

        assert!(camera.validate_wb_mode(WhiteBalanceMode::Auto).is_ok());
        assert!(camera.validate_wb_mode(WhiteBalanceMode::Manual).is_ok());
        // ColorTemperature mode is not in TEST_WB_MODES, so it should fail
        assert!(camera
            .validate_wb_mode(WhiteBalanceMode::ColorTemperature)
            .is_err());
    }

    #[test]
    fn test_tuning_validation() {
        let camera = TestCamera;

        assert!(camera.validate_rg_tuning(0).is_ok());
        assert!(camera.validate_rg_tuning(7).is_ok());
        assert!(camera.validate_rg_tuning(-7).is_ok());
        assert!(camera.validate_rg_tuning(8).is_err());
        assert!(camera.validate_rg_tuning(-8).is_err());
    }

    #[test]
    fn test_color_temp_validation() {
        let camera = TestCamera;

        assert!(camera.validate_color_temp(3200).is_ok());
        assert!(camera.validate_color_temp(5600).is_ok());
        assert!(camera.validate_color_temp(2799).is_err());
        assert!(camera.validate_color_temp(7500).is_err());
    }
}
