//! Image processing methods for cameras using the new GAT architecture.

use crate::{
    command::{resolution::PictureEffectMode, ImageFlipMode},
    types::{
        ContrastLevel, HueLevel, LuminanceLevel, NoiseReduction2DLevel, NoiseReduction3DLevel,
        SaturationLevel, SharpnessLevel,
    },
    Error,
};

/// Image processing operations (async).
#[cfg(feature = "async")]
pub trait ImageProcessingOps: Sized {
    /// Enable image flip.
    async fn enable_flip(&self) -> Result<(), Error>;

    /// Disable image flip.
    async fn disable_flip(&self) -> Result<(), Error>;

    /// Enable horizontal flip (mirror).
    async fn enable_horizontal_flip(&self) -> Result<(), Error>;

    /// Disable horizontal flip (mirror).
    async fn disable_horizontal_flip(&self) -> Result<(), Error>;

    /// Set contrast level.
    async fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    async fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error>;

    /// Set sharpness mode (auto or manual).
    async fn set_sharpness_mode(&self, mode: crate::command::SharpnessMode) -> Result<(), Error>;

    /// Reset sharpness to default.
    async fn reset_sharpness(&self) -> Result<(), Error>;

    /// Increase sharpness by one step.
    async fn increase_sharpness(&self) -> Result<(), Error>;

    /// Decrease sharpness by one step.
    async fn decrease_sharpness(&self) -> Result<(), Error>;

    /// Set saturation level.
    async fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    async fn set_hue(&self, level: HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    async fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error>;

    /// Disable noise reduction 2D.
    async fn disable_noise_reduction_2d(&self) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    async fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error>;

    /// Disable noise reduction 3D.
    async fn disable_noise_reduction_3d(&self) -> Result<(), Error>;

    /// Set image flip mode (combined horizontal and vertical).
    async fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error>;

    /// Set luminance (brightness) level.
    async fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error>;

    /// Enable image freeze.
    /// Freezes the camera's video output on the last frame.
    async fn enable_freeze(&self) -> Result<(), Error>;

    /// Disable image freeze.
    /// Resumes normal video output.
    async fn disable_freeze(&self) -> Result<(), Error>;

    /// Enable black and white mode.
    /// Switches the camera output to monochrome (black and white).
    async fn enable_black_white(&self) -> Result<(), Error>;

    /// Disable black and white mode.
    /// Switches the camera output to color mode.
    async fn disable_black_white(&self) -> Result<(), Error>;

    /// Set picture effect mode.
    /// Controls various artistic effects like negative, sepia, sketch, etc.
    /// Note that not all effects are supported on all camera models.
    async fn set_picture_effect(&self, mode: PictureEffectMode) -> Result<(), Error>;
}

/// Image processing operations (blocking).
#[cfg(not(feature = "async"))]
pub trait ImageProcessingOpsBlocking: Sized {
    /// Enable image flip.
    fn enable_flip(&self) -> Result<(), Error>;

    /// Disable image flip.
    fn disable_flip(&self) -> Result<(), Error>;

    /// Enable horizontal flip (mirror).
    fn enable_horizontal_flip(&self) -> Result<(), Error>;

    /// Disable horizontal flip (mirror).
    fn disable_horizontal_flip(&self) -> Result<(), Error>;

    /// Set contrast level.
    fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error>;

    /// Set sharpness level.
    fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error>;

    /// Set sharpness mode (auto or manual).
    fn set_sharpness_mode(&self, mode: crate::command::SharpnessMode) -> Result<(), Error>;

    /// Reset sharpness to default.
    fn reset_sharpness(&self) -> Result<(), Error>;

    /// Increase sharpness by one step.
    fn increase_sharpness(&self) -> Result<(), Error>;

    /// Decrease sharpness by one step.
    fn decrease_sharpness(&self) -> Result<(), Error>;

    /// Set saturation level.
    fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error>;

    /// Set hue level.
    fn set_hue(&self, level: HueLevel) -> Result<(), Error>;

    /// Set noise reduction 2D level.
    fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error>;

    /// Disable noise reduction 2D.
    fn disable_noise_reduction_2d(&self) -> Result<(), Error>;

    /// Set noise reduction 3D level.
    fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error>;

    /// Disable noise reduction 3D.
    fn disable_noise_reduction_3d(&self) -> Result<(), Error>;

    /// Set image flip mode (combined horizontal and vertical).
    fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error>;

    /// Set luminance (brightness) level.
    fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error>;

    /// Enable image freeze.
    /// Freezes the camera's video output on the last frame.
    fn enable_freeze(&self) -> Result<(), Error>;

    /// Disable image freeze.
    /// Resumes normal video output.
    fn disable_freeze(&self) -> Result<(), Error>;

    /// Enable black and white mode.
    /// Switches the camera output to monochrome (black and white).
    fn enable_black_white(&self) -> Result<(), Error>;

    /// Disable black and white mode.
    /// Switches the camera output to color mode.
    fn disable_black_white(&self) -> Result<(), Error>;

    /// Set picture effect mode.
    /// Controls various artistic effects like negative, sepia, sketch, etc.
    /// Note that not all effects are supported on all camera models.
    fn set_picture_effect(&self, mode: PictureEffectMode) -> Result<(), Error>;
}

// Async implementation
