//! Image processing capability trait and associated types.

use std::ops::Range;

use crate::capabilities::ValidationError;

/// Trait for cameras that support image processing adjustments.
///
/// This trait defines the constants and capabilities for image quality settings
/// including brightness, contrast, sharpness, saturation, and image orientation.
pub trait ImageProcessing {
    /// Valid range for brightness adjustment.
    const BRIGHTNESS_RANGE: Range<u8>;

    /// Valid range for contrast adjustment.
    const CONTRAST_RANGE: Range<u8>;

    /// Valid range for sharpness adjustment.
    const SHARPNESS_RANGE: Range<u8>;

    /// Valid range for saturation adjustment.
    /// None if not supported.
    const SATURATION_RANGE: Option<Range<u8>>;

    /// Whether camera supports image flip (vertical).
    const SUPPORTS_FLIP: bool;

    /// Whether camera supports image mirror (horizontal).
    const SUPPORTS_MIRROR: bool;

    /// Whether camera supports hue adjustment.
    const SUPPORTS_HUE: bool = false;

    /// Hue adjustment range if supported.
    const HUE_RANGE: Option<Range<u8>> = None;

    /// Whether camera supports noise reduction.
    const SUPPORTS_NOISE_REDUCTION: bool = false;

    /// Whether camera supports 2D noise reduction.
    const SUPPORTS_2D_NR: bool = false;

    /// Whether camera supports 3D noise reduction.
    const SUPPORTS_3D_NR: bool = false;

    /// Whether camera supports luminance control.
    const SUPPORTS_LUMINANCE: bool = false;

    /// Luminance range if supported.
    const LUMINANCE_RANGE: Option<Range<u8>> = None;
}

/// Extension trait that adds validation methods to cameras with image processing support.
pub trait ImageProcessingExt: ImageProcessing {
    /// Validate brightness value.
    fn validate_brightness(&self, value: u8) -> Result<u8, ValidationError> {
        if Self::BRIGHTNESS_RANGE.contains(&value) {
            Ok(value)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "brightness",
                value: value as f64,
                min: Self::BRIGHTNESS_RANGE.start as f64,
                max: (Self::BRIGHTNESS_RANGE.end - 1) as f64,
            })
        }
    }

    /// Validate contrast value.
    fn validate_contrast(&self, value: u8) -> Result<u8, ValidationError> {
        if Self::CONTRAST_RANGE.contains(&value) {
            Ok(value)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "contrast",
                value: value as f64,
                min: Self::CONTRAST_RANGE.start as f64,
                max: (Self::CONTRAST_RANGE.end - 1) as f64,
            })
        }
    }

    /// Validate sharpness value.
    fn validate_sharpness(&self, value: u8) -> Result<u8, ValidationError> {
        if Self::SHARPNESS_RANGE.contains(&value) {
            Ok(value)
        } else {
            Err(ValidationError::OutOfRange {
                parameter: "sharpness",
                value: value as f64,
                min: Self::SHARPNESS_RANGE.start as f64,
                max: (Self::SHARPNESS_RANGE.end - 1) as f64,
            })
        }
    }

    /// Validate saturation value.
    fn validate_saturation(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::SATURATION_RANGE {
            Some(ref range) if range.contains(&value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "saturation",
                value: value as f64,
                min: range.start as f64,
                max: (range.end - 1) as f64,
            }),
            None => Err(ValidationError::NotSupported("saturation")),
        }
    }

    /// Validate hue value.
    fn validate_hue(&self, value: u8) -> Result<u8, ValidationError> {
        if !Self::SUPPORTS_HUE {
            return Err(ValidationError::NotSupported("hue"));
        }

        match Self::HUE_RANGE {
            Some(ref range) if range.contains(&value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "hue",
                value: value as f64,
                min: range.start as f64,
                max: (range.end - 1) as f64,
            }),
            None => Err(ValidationError::NotSupported("hue")),
        }
    }

    /// Validate luminance value.
    fn validate_luminance(&self, value: u8) -> Result<u8, ValidationError> {
        if !Self::SUPPORTS_LUMINANCE {
            return Err(ValidationError::NotSupported("luminance"));
        }

        match Self::LUMINANCE_RANGE {
            Some(ref range) if range.contains(&value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "luminance",
                value: value as f64,
                min: range.start as f64,
                max: (range.end - 1) as f64,
            }),
            None => Err(ValidationError::NotSupported("luminance")),
        }
    }

    /// Check if flip is supported.
    fn can_flip(&self) -> bool {
        Self::SUPPORTS_FLIP
    }

    /// Check if mirror is supported.
    fn can_mirror(&self) -> bool {
        Self::SUPPORTS_MIRROR
    }
}

// Automatic implementation for all types that support image processing
impl<T: ImageProcessing> ImageProcessingExt for T {}

// Note: ImageFlipMode, SharpnessMode, and NoiseReductionLevel enums are defined
// in the command module and re-exported from the crate root. This avoids duplication.

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCamera;

    impl ImageProcessing for TestCamera {
        const BRIGHTNESS_RANGE: Range<u8> = 0..16;
        const CONTRAST_RANGE: Range<u8> = 0..16;
        const SHARPNESS_RANGE: Range<u8> = 0..16;
        const SATURATION_RANGE: Option<Range<u8>> = Some(0..16);
        const SUPPORTS_FLIP: bool = true;
        const SUPPORTS_MIRROR: bool = true;
        const SUPPORTS_HUE: bool = true;
        const HUE_RANGE: Option<Range<u8>> = Some(0..15);
    }

    #[test]
    fn test_brightness_validation() {
        let camera = TestCamera;

        assert!(camera.validate_brightness(0).is_ok());
        assert!(camera.validate_brightness(15).is_ok());
        assert!(camera.validate_brightness(16).is_err());
    }

    #[test]
    fn test_saturation_validation() {
        let camera = TestCamera;

        assert!(camera.validate_saturation(0).is_ok());
        assert!(camera.validate_saturation(15).is_ok());
        assert!(camera.validate_saturation(16).is_err());
    }

    #[test]
    fn test_flip_support() {
        let camera = TestCamera;

        assert!(camera.can_flip());
        assert!(camera.can_mirror());
    }
}
