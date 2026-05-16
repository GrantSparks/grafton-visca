//! Image processing control implementation for PTZ cameras.
//!
//! This module provides comprehensive image processing and enhancement functionality including:
//! - Image orientation control (flip, mirror, rotation)
//! - Visual quality adjustments (contrast, sharpness, saturation, hue)
//! - Noise reduction for improved image quality
//! - Special effects and picture modes
//! - Color mode switching (color/black & white)
//! - Luminance (brightness) control
//!
//! These controls allow fine-tuning of the camera's image output to achieve
//! the desired visual quality for different environments and use cases.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    capabilities::ImageProcessing as ImageProcessingCap,
    command::{ImageFlipMode, PictureEffectMode},
    mode::Mode,
    types::{
        ContrastLevel, GammaLevel, HueLevel, LuminanceLevel, NoiseReduction2DLevel,
        NoiseReduction3DLevel, SaturationLevel, SharpnessLevel,
    },
    Error,
};

/// Contrast operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ContrastControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set contrast level.
    ///
    /// Adjusts the difference between light and dark areas in the image.
    /// Higher values increase contrast (more dramatic differences),
    /// lower values decrease contrast (flatter appearance).
    ///
    /// Use [`ContrastInquiryControl::contrast`] to query the current value.
    ///
    /// # Parameters
    /// - `level`: The contrast level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    ///
    /// [`ContrastInquiryControl::contrast`]: crate::camera::controls::inquiry::ContrastInquiryControl::contrast
    fn set_contrast(
        &self,
        level: ContrastLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Sharpness operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait SharpnessControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set sharpness level.
    ///
    /// Controls edge enhancement to make the image appear sharper or softer.
    /// Higher values increase sharpness (more edge enhancement),
    /// lower values decrease sharpness (softer appearance).
    ///
    /// Use [`SharpnessInquiryControl::sharpness_level`] to query the current
    /// value, and [`SharpnessInquiryControl::sharpness_mode`] to query whether
    /// sharpness is in auto or manual mode.
    ///
    /// # Parameters
    /// - `level`: The sharpness level to set
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    ///
    /// [`SharpnessInquiryControl::sharpness_level`]: crate::camera::controls::inquiry::SharpnessInquiryControl::sharpness_level
    /// [`SharpnessInquiryControl::sharpness_mode`]: crate::camera::controls::inquiry::SharpnessInquiryControl::sharpness_mode
    fn set_sharpness(
        &self,
        level: SharpnessLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset sharpness to default.
    ///
    /// Resets the sharpness level to the camera's default setting.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn reset_sharpness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Increase sharpness by one step.
    ///
    /// Increases edge enhancement by one increment, making the image sharper.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn increase_sharpness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Decrease sharpness by one step.
    ///
    /// Decreases edge enhancement by one increment, making the image softer.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn decrease_sharpness(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Vertical image flip operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ImageFlipControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable vertical image flip.
    fn enable_flip(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable vertical image flip.
    fn disable_flip(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Horizontal image mirror operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ImageMirrorControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable horizontal image mirror.
    fn enable_horizontal_flip(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable horizontal image mirror.
    fn disable_horizontal_flip(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Combined image flip-mode operations for profiles using the combined opcode.
#[grafton_visca_macros::delegate_to_session]
pub trait ImageFlipModeControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set combined horizontal/vertical image flip mode.
    fn set_image_flip(
        &self,
        mode: ImageFlipMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Saturation operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait SaturationControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set saturation level.
    fn set_saturation(
        &self,
        level: SaturationLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Hue operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait HueControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set hue level.
    fn set_hue(&self, level: HueLevel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Luminance operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait LuminanceControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set luminance level.
    fn set_luminance(
        &self,
        level: LuminanceLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Gamma operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait GammaControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set gamma curve.
    fn set_gamma(&self, level: GammaLevel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// 2D noise-reduction operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait NoiseReduction2DControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set 2D noise-reduction level.
    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable 2D noise reduction.
    fn disable_noise_reduction_2d(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// 3D noise-reduction operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait NoiseReduction3DControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set 3D noise-reduction level.
    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable 3D noise reduction.
    fn disable_noise_reduction_3d(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

/// Picture-effect operations for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait PictureEffectControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable black-and-white picture effect.
    fn enable_black_white(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable picture effects.
    fn disable_black_white(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set picture effect mode.
    fn set_picture_effect(
        &self,
        mode: PictureEffectMode,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> ImageFlipControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasImageFlip,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_flip(&self) -> M::Fut<'_, Result<(), Error>> {
        if P::USES_COMBINED_FLIP_COMMAND {
            // PTZOptics: Use combined flip command
            // Get current horizontal state from cache
            let cached = self.cache().flip_state();
            let h = cached.map(|s| s.horizontal).unwrap_or(false);
            let mode = if h {
                ImageFlipMode::Both
            } else {
                ImageFlipMode::Vertical
            };
            let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
            self.execute_updating_cache(cmd, move |cache| {
                cache.set_flip_state(h, true);
            })
        } else {
            // Legacy: Use separate 0x66 command
            let cmd = crate::command::flip::ImageFlip {
                flip: crate::command::flip::Flip::On,
            };
            self.execute(cmd)
        }
    }

    fn disable_flip(&self) -> M::Fut<'_, Result<(), Error>> {
        if P::USES_COMBINED_FLIP_COMMAND {
            // PTZOptics: Use combined flip command
            let cached = self.cache().flip_state();
            let h = cached.map(|s| s.horizontal).unwrap_or(false);
            let mode = if h {
                ImageFlipMode::Horizontal
            } else {
                ImageFlipMode::Off
            };
            let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
            self.execute_updating_cache(cmd, move |cache| {
                cache.set_flip_state(h, false);
            })
        } else {
            // Legacy: Use separate 0x66 command
            let cmd = crate::command::flip::ImageFlip {
                flip: crate::command::flip::Flip::Off,
            };
            self.execute(cmd)
        }
    }
}

impl<M, P, Tr, Exec> ImageMirrorControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasImageMirror,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_horizontal_flip(&self) -> M::Fut<'_, Result<(), Error>> {
        if P::USES_COMBINED_FLIP_COMMAND {
            // PTZOptics: Use combined flip command
            let cached = self.cache().flip_state();
            let v = cached.map(|s| s.vertical).unwrap_or(false);
            let mode = if v {
                ImageFlipMode::Both
            } else {
                ImageFlipMode::Horizontal
            };
            let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
            self.execute_updating_cache(cmd, move |cache| {
                cache.set_flip_state(true, v);
            })
        } else {
            // Legacy: Use separate 0x61 command
            let cmd = crate::command::flip::HorizontalFlip { on: true };
            self.execute(cmd)
        }
    }

    fn disable_horizontal_flip(&self) -> M::Fut<'_, Result<(), Error>> {
        if P::USES_COMBINED_FLIP_COMMAND {
            // PTZOptics: Use combined flip command
            let cached = self.cache().flip_state();
            let v = cached.map(|s| s.vertical).unwrap_or(false);
            let mode = if v {
                ImageFlipMode::Vertical
            } else {
                ImageFlipMode::Off
            };
            let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
            self.execute_updating_cache(cmd, move |cache| {
                cache.set_flip_state(false, v);
            })
        } else {
            // Legacy: Use separate 0x61 command
            let cmd = crate::command::flip::HorizontalFlip { on: false };
            self.execute(cmd)
        }
    }
}

impl<M, P, Tr, Exec> ContrastControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasContrastControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_contrast(&self, level: ContrastLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Contrast::new(level);
        self.execute(cmd)
    }
}

impl<M, P, Tr, Exec> SharpnessControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasSharpnessControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_sharpness(&self, level: SharpnessLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::SetLevel {
            value: level.value(),
        };
        self.execute(cmd)
    }

    fn reset_sharpness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Reset;
        self.execute(cmd)
    }

    fn increase_sharpness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Up;
        self.execute(cmd)
    }

    fn decrease_sharpness(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Sharpness::Down;
        self.execute(cmd)
    }
}

impl<M, P, Tr, Exec> ImageFlipModeControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasCombinedImageFlip,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_image_flip(&self, mode: ImageFlipMode) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::ImageFlipCombinedCommand::new(mode);
        let (h, v) = match mode {
            ImageFlipMode::Off => (false, false),
            ImageFlipMode::Horizontal => (true, false),
            ImageFlipMode::Vertical => (false, true),
            ImageFlipMode::Both => (true, true),
        };
        self.execute_updating_cache(cmd, move |cache| {
            cache.set_flip_state(h, v);
        })
    }
}

impl<M, P, Tr, Exec> SaturationControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasSaturationControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_saturation(&self, level: SaturationLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::color::SaturationCommand::new(level);
        self.execute(cmd)
    }
}

impl<M, P, Tr, Exec> HueControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasHueControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_hue(&self, level: HueLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::color::HueCommand::new(level);
        self.execute(cmd)
    }
}

impl<M, P, Tr, Exec> LuminanceControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasLuminanceControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_luminance(&self, level: LuminanceLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::Luminance::new(level);
        self.execute(cmd)
    }
}

impl<M, P, Tr, Exec> GammaControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasGammaControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_gamma(&self, level: GammaLevel) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::GammaCommand::new(level);
        self.execute(cmd)
    }
}

impl<M, P, Tr, Exec> NoiseReduction2DControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasNoiseReduction2D,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_noise_reduction_2d(
        &self,
        level: NoiseReduction2DLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction2D::with_level(level);
        self.execute(cmd)
    }

    fn disable_noise_reduction_2d(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction2D::off();
        self.execute(cmd)
    }
}

impl<M, P, Tr, Exec> NoiseReduction3DControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasNoiseReduction3D,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_noise_reduction_3d(
        &self,
        level: NoiseReduction3DLevel,
    ) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction3D::with_level(level);
        self.execute(cmd)
    }

    fn disable_noise_reduction_3d(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::NoiseReduction3D::off();
        self.execute(cmd)
    }
}

impl<M, P, Tr, Exec> PictureEffectControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile
        + ImageProcessingCap
        + Default
        + crate::capabilities::HasPictureEffect,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_black_white(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::BlackAndWhite,
        };
        self.execute(cmd)
    }

    fn disable_black_white(&self) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand {
            mode: PictureEffectMode::Off,
        };
        self.execute(cmd)
    }

    fn set_picture_effect(&self, mode: PictureEffectMode) -> M::Fut<'_, Result<(), Error>> {
        let cmd = crate::command::image::PictureEffectCommand { mode };
        self.execute(cmd)
    }
}
