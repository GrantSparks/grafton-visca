//! Image processing capability trait and associated types.

use crate::capabilities::{CapabilityRange, ValidationError};

/// Trait for cameras that report image-processing metadata.
///
/// This trait defines runtime metadata for image quality settings including
/// contrast, sharpness, saturation, and image orientation. Optional ranges use
/// `None` for unsupported profile surfaces; supported ranges must be non-empty.
pub trait ImageProcessing {
    /// Valid range for contrast adjustment, if supported.
    const CONTRAST_RANGE: Option<CapabilityRange<u8>>;

    /// Valid range for sharpness adjustment, if supported.
    const SHARPNESS_RANGE: Option<CapabilityRange<u8>>;

    /// Valid range for saturation adjustment.
    /// None if not supported.
    const SATURATION_RANGE: Option<CapabilityRange<u8>>;

    /// Whether camera supports image flip (vertical).
    const SUPPORTS_FLIP: bool;

    /// Whether camera supports image mirror (horizontal).
    const SUPPORTS_MIRROR: bool;

    /// Whether camera supports hue adjustment.
    const SUPPORTS_HUE: bool = false;

    /// Hue adjustment range if supported.
    const HUE_RANGE: Option<CapabilityRange<u8>> = None;

    /// Whether camera supports noise reduction.
    const SUPPORTS_NOISE_REDUCTION: bool = false;

    /// Whether camera supports 2D noise reduction.
    const SUPPORTS_2D_NR: bool = false;

    /// Whether camera supports 3D noise reduction.
    const SUPPORTS_3D_NR: bool = false;

    /// Whether camera supports luminance control.
    const SUPPORTS_LUMINANCE: bool = false;

    /// Whether camera supports source-backed picture effects (Off and Black & White).
    /// Model-specific values remain available through `PictureEffectMode::Unknown`.
    const SUPPORTS_PICTURE_EFFECT: bool = false;

    /// Luminance range if supported.
    const LUMINANCE_RANGE: Option<CapabilityRange<u8>> = None;

    /// Whether camera uses the combined flip command (0xA4) instead of legacy commands (0x61/0x66).
    ///
    /// PTZOptics G2/G3/30X cameras use the combined command which sets both horizontal
    /// and vertical flip in a single operation. Legacy Sony cameras use separate commands.
    const USES_COMBINED_FLIP_COMMAND: bool = false;

    /// Whether camera requires Settings Save (0xA5) after flip changes to persist them.
    ///
    /// PTZOptics cameras may require this command after changing flip settings to ensure
    /// the changes persist across power cycles.
    const REQUIRES_SETTINGS_SAVE_FOR_FLIP: bool = false;

    /// Whether camera supports gamma curve control via VISCA command `0x5B`.
    ///
    /// When supported, the camera accepts direct gamma curve selection
    /// (0=Standard, 1-4=different gamma curves depending on model).
    const SUPPORTS_GAMMA: bool = false;

    /// Valid range for gamma curve selection, if supported.
    const GAMMA_RANGE: Option<CapabilityRange<u8>> = None;

    /// Whether the profile permits the base typed image noun.
    ///
    /// This is the runtime counterpart of
    /// [`HasImageProcessing`](crate::capabilities::HasImageProcessing), not a
    /// substitute for its compile-time marker. Built-in profiles override this
    /// from the source-backed profile registry, which also emits the marker.
    /// A downstream profile defaults to `false`; it must opt in here and
    /// implement `HasImageProcessing` together for a documented base image
    /// surface such as a freeze-only implementation. Image metadata alone is
    /// never fallback permission for the typed or dynamic surface.
    const SUPPORTS_IMAGE_PROCESSING: bool = false;
}

/// Extension trait that adds validation methods to cameras with image processing support.
pub trait ImageProcessingExt: ImageProcessing {
    /// Validate contrast value.
    fn validate_contrast(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::CONTRAST_RANGE {
            Some(range) if range.contains(value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "contrast",
                value: value as f64,
                min: range.min() as f64,
                max: range.max() as f64,
            }),
            None => Err(ValidationError::NotSupported("contrast")),
        }
    }

    /// Validate sharpness value.
    fn validate_sharpness(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::SHARPNESS_RANGE {
            Some(range) if range.contains(value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "sharpness",
                value: value as f64,
                min: range.min() as f64,
                max: range.max() as f64,
            }),
            None => Err(ValidationError::NotSupported("sharpness")),
        }
    }

    /// Validate saturation value.
    fn validate_saturation(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::SATURATION_RANGE {
            Some(range) if range.contains(value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "saturation",
                value: value as f64,
                min: range.min() as f64,
                max: range.max() as f64,
            }),
            None => Err(ValidationError::NotSupported("saturation")),
        }
    }

    /// Validate hue value.
    ///
    /// Returns `NotSupported` error if `HUE_RANGE` is `None`.
    fn validate_hue(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::HUE_RANGE {
            Some(range) if range.contains(value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "hue",
                value: value as f64,
                min: range.min() as f64,
                max: range.max() as f64,
            }),
            None => Err(ValidationError::NotSupported("hue")),
        }
    }

    /// Validate luminance value.
    ///
    /// Returns `NotSupported` error if `LUMINANCE_RANGE` is `None`.
    fn validate_luminance(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::LUMINANCE_RANGE {
            Some(range) if range.contains(value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "luminance",
                value: value as f64,
                min: range.min() as f64,
                max: range.max() as f64,
            }),
            None => Err(ValidationError::NotSupported("luminance")),
        }
    }

    /// Validate gamma value.
    ///
    /// Note: This method should only be called on profiles that support gamma.
    fn validate_gamma(&self, value: u8) -> Result<u8, ValidationError> {
        match Self::GAMMA_RANGE {
            Some(range) if range.contains(value) => Ok(value),
            Some(ref range) => Err(ValidationError::OutOfRange {
                parameter: "gamma",
                value: value as f64,
                min: range.min() as f64,
                max: range.max() as f64,
            }),
            None => Err(ValidationError::NotSupported("gamma")),
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

    /// Check if camera uses combined flip command (0xA4).
    fn uses_combined_flip_command(&self) -> bool {
        Self::USES_COMBINED_FLIP_COMMAND
    }

    /// Check if camera requires settings save after flip changes.
    fn requires_settings_save_for_flip(&self) -> bool {
        Self::REQUIRES_SETTINGS_SAVE_FOR_FLIP
    }
}

// Automatic implementation for all types that support image processing
impl<T: ImageProcessing> ImageProcessingExt for T {}

// Note: ImageFlipMode, SharpnessMode, and NoiseReduction2DMode enums are defined
// in the command module and re-exported from the crate root. This avoids duplication.

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCamera;
    struct MetadataOnlyCamera;

    impl ImageProcessing for TestCamera {
        const CONTRAST_RANGE: Option<CapabilityRange<u8>> = Some(CapabilityRange::<u8>::new(0, 15));
        const SHARPNESS_RANGE: Option<CapabilityRange<u8>> =
            Some(CapabilityRange::<u8>::new(0, 15));
        const SATURATION_RANGE: Option<CapabilityRange<u8>> =
            Some(CapabilityRange::<u8>::new(0, 15));
        const SUPPORTS_FLIP: bool = true;
        const SUPPORTS_MIRROR: bool = true;
        const SUPPORTS_HUE: bool = true;
        const HUE_RANGE: Option<CapabilityRange<u8>> = Some(CapabilityRange::<u8>::new(0, 14));
        const SUPPORTS_IMAGE_PROCESSING: bool = true;
    }

    impl ImageProcessing for MetadataOnlyCamera {
        const CONTRAST_RANGE: Option<CapabilityRange<u8>> = None;
        const SHARPNESS_RANGE: Option<CapabilityRange<u8>> = None;
        const SATURATION_RANGE: Option<CapabilityRange<u8>> = None;
        const SUPPORTS_FLIP: bool = true;
        const SUPPORTS_MIRROR: bool = false;
    }

    #[test]
    fn test_contrast_validation() {
        let camera = TestCamera;

        assert!(camera.validate_contrast(0).is_ok());
        assert!(camera.validate_contrast(15).is_ok());
        assert!(camera.validate_contrast(16).is_err());
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

    #[test]
    fn explicit_base_permission_reports_the_image_domain() {
        const { assert!(<TestCamera as ImageProcessing>::SUPPORTS_IMAGE_PROCESSING) };
    }

    #[test]
    fn image_metadata_does_not_default_to_base_permission() {
        const { assert!(!<MetadataOnlyCamera as ImageProcessing>::SUPPORTS_IMAGE_PROCESSING) };
    }
}
