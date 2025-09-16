//! image processing control implementation using Mode trait.

use crate::{
    camera::CommandClient,
    command::{resolution::PictureEffectMode, ImageFlipMode},
    mode::Mode,
    types::{
        ContrastLevel, HueLevel, LuminanceLevel, NoiseReduction2DLevel, NoiseReduction3DLevel,
        SaturationLevel, SharpnessLevel,
    },
    Error,
};

/// image processing operations for cameras.
///
/// This trait provides image processing control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait ImageProcessingControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable image flip.
    fn enable_flip(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Disable image flip.
    fn disable_flip(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Enable horizontal flip (mirror).
    fn enable_horizontal_flip(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Disable horizontal flip (mirror).
    fn disable_horizontal_flip(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set contrast level.
    fn set_contrast(
        &self,
        level: ContrastLevel,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set sharpness level.
    fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set sharpness mode (auto or manual).
    fn set_sharpness_mode(
        &self,
        mode: crate::command::SharpnessMode,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Reset sharpness to default.
    fn reset_sharpness(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Increase sharpness by one step.
    fn increase_sharpness(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Decrease sharpness by one step.
    fn decrease_sharpness(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set saturation level.
    fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set hue level.
    fn set_hue(&self, level: HueLevel) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set noise reduction 2D level.
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Disable noise reduction 2D.
    fn disable_noise_reduction_2d(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set noise reduction 3D level.
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Disable noise reduction 3D.
    fn disable_noise_reduction_3d(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set image flip mode (combined horizontal and vertical).
    fn set_image_flip(
        &self,
        mode: ImageFlipMode,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set luminance (brightness) level.
    fn set_luminance(
        &self,
        level: LuminanceLevel,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Enable image freeze.
    /// Freezes the camera's video output on the last frame.
    fn enable_freeze(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Disable image freeze.
    /// Resumes normal video output.
    fn disable_freeze(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Enable black and white mode.
    /// Switches the camera output to monochrome (black and white).
    fn enable_black_white(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Disable black and white mode.
    /// Switches the camera output to color mode.
    fn disable_black_white(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set picture effect mode.
    /// Controls various artistic effects like negative, sepia, sketch, etc.
    /// Note that not all effects are supported on all camera models.
    fn set_picture_effect(
        &self,
        mode: PictureEffectMode,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> ImageProcessingControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CommandClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_flip(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::flip::ImageFlip::new(crate::command::flip::Flip::On);
        self.send_and_complete(cmd)
    }

    fn disable_flip(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::flip::ImageFlip::new(crate::command::flip::Flip::Off);
        self.send_and_complete(cmd)
    }

    fn enable_horizontal_flip(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::flip::HorizontalFlipCommand::new(
            crate::command::flip::HorizontalFlip::On,
        );
        self.send_and_complete(cmd)
    }

    fn disable_horizontal_flip(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::flip::HorizontalFlipCommand::new(
            crate::command::flip::HorizontalFlip::Off,
        );
        self.send_and_complete(cmd)
    }

    fn set_contrast(&self, level: ContrastLevel) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::Contrast::new(level);
        self.send_and_complete(cmd)
    }

    fn set_sharpness(&self, level: SharpnessLevel) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::SetLevel {
            value: level.value(),
        };
        self.send_and_complete(cmd)
    }

    fn set_sharpness_mode(
        &self,
        _mode: crate::command::SharpnessMode,
    ) -> M::Ret<'_, Result<(), Error>> {
        // SharpnessMode command not documented in VISCA protocol spec
        // This may be a proprietary extension - returning unsupported for now
        self.error(Error::Unsupported)
    }

    fn reset_sharpness(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Reset;
        self.send_and_complete(cmd)
    }

    fn increase_sharpness(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Up;
        self.send_and_complete(cmd)
    }

    fn decrease_sharpness(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Down;
        self.send_and_complete(cmd)
    }

    fn set_saturation(&self, level: SaturationLevel) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::color::SaturationCommand::new(level);
        self.send_and_complete(cmd)
    }

    fn set_hue(&self, level: HueLevel) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::color::HueCommand::new(level);
        self.send_and_complete(cmd)
    }

    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction2D::Level(level);
        self.send_and_complete(cmd)
    }

    fn disable_noise_reduction_2d(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction2D::Off;
        self.send_and_complete(cmd)
    }

    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction3D::Level(level);
        self.send_and_complete(cmd)
    }

    fn disable_noise_reduction_3d(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction3D::Off;
        self.send_and_complete(cmd)
    }

    fn set_image_flip(&self, mode: ImageFlipMode) -> M::Ret<'_, Result<(), Error>> {
        // Use the combined flip command (PtzOptics A4 opcode)
        // This is more efficient than sending separate vertical and horizontal commands
        let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
        self.send_and_complete(cmd)
    }

    fn set_luminance(&self, level: LuminanceLevel) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::Luminance::new(level);
        self.send_and_complete(cmd)
    }

    fn enable_freeze(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::flip::ImageFreezeCommand::new(crate::command::flip::Freeze::On);
        self.send_and_complete(cmd)
    }

    fn disable_freeze(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::flip::ImageFreezeCommand::new(crate::command::flip::Freeze::Off);
        self.send_and_complete(cmd)
    }

    fn enable_black_white(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite,
        };
        self.send_and_complete(cmd)
    }

    fn disable_black_white(&self) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::Off,
        };
        self.send_and_complete(cmd)
    }

    fn set_picture_effect(&self, mode: PictureEffectMode) -> M::Ret<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand { mode };
        self.send_and_complete(cmd)
    }
}
