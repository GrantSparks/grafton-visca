//! Inquiry control implementation for PTZ cameras.
//!
//! This module provides comprehensive camera status inquiry functionality including:
//! - Current camera settings and modes (focus, exposure, white balance, etc.)
//! - Position information (pan, tilt, zoom, focus positions)
//! - Image processing settings (sharpness, saturation, noise reduction)
//! - System status (power, version, menu state)
//! - Color settings (gain, tuning, temperature)
//! - Special features (tally lights, digital PTZ, defog)
//!
//! Inquiry operations allow you to read the current state of various camera
//! parameters without changing them. This is essential for building user
//! interfaces, monitoring camera status, and synchronizing settings.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::{
        focus::AutoFocusSensitivity, ExposureMode, FocusMode, FocusZone, SharpnessMode,
        WhiteBalanceMode,
    },
    mode::Mode,
    Error,
};

/// Inquiry operations for PTZ cameras.
///
/// This trait provides comprehensive inquiry methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Inquiry Categories
///
/// - **Position**: Current pan/tilt/zoom/focus positions
/// - **Exposure**: Iris, shutter, gain, exposure mode, compensation
/// - **Color**: White balance, color temperature, red/blue gain and tuning
/// - **Image**: Sharpness, saturation, hue, noise reduction, picture effects
/// - **System**: Power state, version, menu status, tally lights
/// - **Special**: digital PTZ, defog
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// let zoom_pos = camera.zoom_position()?;
/// let exp_mode = camera.exposure_mode()?;
/// let wb_mode = camera.white_balance_mode()?;
/// let version = camera.version()?;
/// ```
///
/// ## Async mode
/// ```ignore
/// let zoom_pos = camera.zoom_position().await?;
/// let exp_mode = camera.exposure_mode().await?;
/// let wb_mode = camera.white_balance_mode().await?;
/// let version = camera.version().await?;
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait InquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current power state of the camera.
    ///
    /// Returns the camera's power status.
    ///
    /// # Returns
    /// - `true` if the camera is powered on and operational
    /// - `false` if the camera is in standby mode
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn power_state(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current zoom position.
    ///
    /// Returns the current zoom level as a position value.
    /// The range depends on the camera model and zoom capabilities.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn zoom_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ZoomPosition, Error>>;

    /// Get the current focus position.
    ///
    /// Returns the current focus position as a typed value.
    /// Higher values typically indicate focus on more distant objects.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::FocusPosition, Error>>;

    /// Get the current exposure mode.
    ///
    /// Returns the active exposure mode (auto, manual, shutter priority, etc.).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn exposure_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<ExposureMode, Error>>;

    /// Get the current shutter speed.
    ///
    /// Returns the current shutter speed setting as a typed value.
    /// The exact interpretation depends on the camera model.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn shutter(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ShutterSpeed, Error>>;

    /// Get the current gain value.
    ///
    /// Returns the current sensor gain level. Higher values increase
    /// image brightness but also increase noise.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::GainLevel, Error>>;

    /// Get the gain limit value.
    ///
    /// Returns the maximum gain level that auto exposure is allowed to use.
    /// This helps control noise in low light conditions.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn gain_limit(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::GainLimit, Error>>;

    /// Get the white balance mode.
    ///
    /// Returns the current white balance mode (auto, indoor, outdoor, etc.).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn white_balance_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<WhiteBalanceMode, Error>>;

    /// Get the brightness level.
    ///
    /// Returns the current brightness adjustment level.
    /// This is separate from exposure brightness.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn brightness(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::BrightnessLevel, Error>>;

    /// Get the sharpness mode.
    ///
    /// Returns the current sharpness mode setting (auto or manual).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn sharpness_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<SharpnessMode, Error>>;

    /// Get the sharpness level.
    ///
    /// Returns the current sharpness level setting.
    /// Higher values increase edge enhancement/sharpening.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn sharpness_level(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::SharpnessLevel, Error>>;

    /// Get the contrast level.
    ///
    /// Returns the current contrast level setting.
    /// Higher values increase the difference between light and dark areas.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn contrast(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ContrastLevel, Error>>;

    /// Get the current video resolution mode.
    ///
    /// Returns the current video resolution setting.
    /// Common modes include 1080p60, 1080p30, 720p60, etc.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn resolution(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::ResolutionMode, Error>>;

    /// Get the camera version information.
    ///
    /// Returns detailed version information including model name,
    /// firmware version, and other identifying information.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn version(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::VersionInfo, Error>>;

    /// Get the current focus mode.
    ///
    /// Returns whether the camera is in auto focus or manual focus mode.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<FocusMode, Error>>;

    /// Get the menu open/close status.
    ///
    /// Returns whether the camera's on-screen menu is currently displayed.
    ///
    /// # Returns
    /// - `true` if the menu is open/visible
    /// - `false` if the menu is closed/hidden
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn menu_status(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the tally light status.
    ///
    /// Returns the current state of the camera's tally lights
    /// including both red and green indicators.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn tally_light_status(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::TallyStatusState, Error>>;

    /// Get the night/day mode status.
    ///
    /// Returns whether the camera is in night mode (for low light conditions)
    /// or day mode (for normal lighting conditions).
    ///
    /// # Returns
    /// - `true` if night mode is active
    /// - `false` if day mode is active
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn night_day_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the standby mode status.
    ///
    /// Returns whether the camera is in standby mode.
    ///
    /// # Returns
    /// - `true` if standby mode is enabled
    /// - `false` if standby mode is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn standby_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the defog level.
    ///
    /// Returns the current defog processing level which helps
    /// improve visibility in foggy or hazy conditions.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn defog_level(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::DefogLevel, Error>>;

    /// Get the digital PTZ mode status.
    ///
    /// Returns whether digital pan/tilt/zoom is enabled.
    /// Digital PTZ uses image cropping instead of mechanical movement.
    ///
    /// # Returns
    /// - `true` if digital PTZ is enabled
    /// - `false` if digital PTZ is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn digital_ptz_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the auto trace mode status.
    ///
    /// Returns whether automatic subject tracking is enabled.
    /// Auto trace attempts to follow moving subjects automatically.
    ///
    /// # Returns
    /// - `true` if auto trace is enabled
    /// - `false` if auto trace is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn auto_trace_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the focus unlock state.
    ///
    /// Returns whether focus unlock is enabled, which may affect
    /// how focus controls respond.
    ///
    /// # Returns
    /// - `true` if focus unlock is enabled
    /// - `false` if focus unlock is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_unlock(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the broadcast domain setting.
    ///
    /// Returns the current broadcast domain configuration
    /// which may affect video output standards.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn broadcast_domain(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::BroadcastDomain, Error>>;

    /// Get the USB audio state.
    ///
    /// Returns whether USB audio is enabled for cameras
    /// that support audio over USB connections.
    ///
    /// # Returns
    /// - `true` if USB audio is enabled
    /// - `false` if USB audio is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn usb_audio_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the two tone mode state.
    ///
    /// Returns whether two tone mode is enabled, which may
    /// affect color reproduction or contrast.
    ///
    /// # Returns
    /// - `true` if two tone mode is enabled
    /// - `false` if two tone mode is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn two_tone_mode_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the digital mode state.
    ///
    /// Returns whether digital mode is enabled, which may affect
    /// image processing or output characteristics.
    ///
    /// # Returns
    /// - `true` if digital mode is enabled
    /// - `false` if digital mode is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn digital_mode_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the tally auto adjust state.
    ///
    /// Returns whether automatic tally light adjustment is enabled,
    /// which may adjust tally brightness based on conditions.
    ///
    /// # Returns
    /// - `true` if tally auto adjust is enabled
    /// - `false` if tally auto adjust is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn tally_auto_adjust_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the anti-flicker mode setting.
    ///
    /// Returns the current flicker reduction mode (Off, 50Hz, or 60Hz).
    ///
    /// **Vendor-Specific**: PTZOptics cameras only.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn flicker_mode(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::exposure::AntiFlickerMode, Error>>;
}

/// ND filter-specific inquiry operations for cameras with typed ND filter support.
#[grafton_visca_macros::delegate_to_session]
pub trait NdFilterInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current ND filter position.
    fn nd_filter_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::NdFilterPosition, Error>>;

    /// Get the ND filter preset setting.
    fn nd_filter_preset(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::NdFilterPreset, Error>>;
}

/// Focus near-limit inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait FocusNearLimitInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the focus near limit position.
    fn focus_near_limit(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::FocusPosition, Error>>;
}

/// Focus zone inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait FocusZoneInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current focus zone.
    fn focus_zone(&self) -> <Self::Mode as Mode>::Fut<'_, Result<FocusZone, Error>>;
}

/// Auto-focus sensitivity inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait AutoFocusSensitivityInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the auto focus sensitivity.
    fn auto_focus_sensitivity(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<AutoFocusSensitivity, Error>>;
}

/// Iris value inquiry for profiles with documented iris support.
#[grafton_visca_macros::delegate_to_session]
pub trait IrisInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the iris control mode.
    fn iris_control(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current iris value.
    fn iris(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::IrisLevel, Error>>;
}

/// Exposure-compensation inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ExposureCompensationInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get exposure compensation value.
    fn exposure_compensation(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ExposureCompensationLevel, Error>>;

    /// Check whether exposure compensation is enabled.
    fn exposure_compensation_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get exposure compensation position.
    fn exposure_compensation_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ExposureCompensationPosition, Error>>;
}

/// Backlight compensation inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait BacklightCompensationInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Check whether backlight compensation is enabled.
    fn backlight_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;
}

/// Wide dynamic range inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait WideDynamicRangeInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get dynamic range level.
    fn dynamic_range(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::DynamicRangeLevel, Error>>;
}

/// Color-temperature inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ColorTemperatureInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get color temperature.
    fn color_temperature(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ColorTemp, Error>>;
}

/// RGB gain inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait RgbGainInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get red gain.
    fn red_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::RedChannel, Error>>;

    /// Get blue gain.
    fn blue_gain(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::BlueChannel, Error>>;
}

/// RGB tuning inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait RgbTuningInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get red tuning.
    fn red_tuning(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::RedTuning, Error>>;

    /// Get blue tuning.
    fn blue_tuning(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::BlueTuning, Error>>;
}

/// Saturation inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait SaturationInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get saturation level.
    fn saturation(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::SaturationLevel, Error>>;
}

/// Hue inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait HueInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get hue level.
    fn hue(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::HueLevel, Error>>;
}

/// Luminance inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait LuminanceInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get luminance level.
    fn luminance(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::LuminanceLevel, Error>>;
}

/// Gamma inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait GammaInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get gamma level.
    fn gamma(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::GammaLevel, Error>>;
}

/// Image flip inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait ImageFlipInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get image flip settings.
    fn image_flip(&self)
        -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::FlipState, Error>>;

    /// Get combined flip mode.
    fn flip_mode(&self) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::FlipState, Error>>;
}

/// Aggregate noise-reduction inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait NoiseReductionInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get aggregate noise reduction level.
    fn noise_reduction_level(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::NoiseReductionLevel, Error>>;

    /// Get noise reduction mode.
    fn noise_reduction_mode(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::NoiseReductionMode, Error>>;
}

/// 2D noise-reduction inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait NoiseReduction2DInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get 2D noise-reduction level.
    fn noise_reduction_2d(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::NoiseReduction2DLevel, Error>>;
}

/// 3D noise-reduction inquiry for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait NoiseReduction3DInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get 3D noise-reduction level.
    fn noise_reduction_3d(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::NoiseReduction3DLevel, Error>>;
}

/// Picture-effect inquiries for profiles with documented support.
#[grafton_visca_macros::delegate_to_session]
pub trait PictureEffectInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Check whether black-and-white mode is enabled.
    fn black_white(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get black-and-white mode.
    fn black_white_mode(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::BlackWhiteMode, Error>>;

    /// Get picture effect mode.
    fn picture_effect(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::PictureEffectMode, Error>>;
}

/// Pan/tilt-specific inquiry operations for cameras.
///
/// This trait provides pan/tilt position inquiry methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// let position = camera.pan_tilt_position()?;
/// println!("Pan: {}, Tilt: {}", position.pan, position.tilt);
/// ```
///
/// ## Async mode
/// ```ignore
/// let position = camera.pan_tilt_position().await?;
/// println!("Pan: {}, Tilt: {}", position.pan, position.tilt);
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait PanTiltInquiryControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Get the current pan and tilt position.
    ///
    /// Returns the current pan and tilt position coordinates.
    /// Values are typically in degrees or camera-specific units.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn pan_tilt_position(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::camera::PanTiltPosition, Error>>;
}

// Single unified implementation for InquiryControl
impl<M, P, Tr, Exec> InquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn power_state(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::PowerInquiry;
        self.query(PowerInquiry)
    }

    fn zoom_position(&self) -> M::Fut<'_, Result<crate::types::ZoomPosition, Error>> {
        use crate::command::inquiry_structs::ZoomPositionInquiry;
        self.query(ZoomPositionInquiry)
    }

    fn focus_position(&self) -> M::Fut<'_, Result<crate::types::FocusPosition, Error>> {
        use crate::command::inquiry_structs::FocusPositionInquiry;
        self.query(FocusPositionInquiry)
    }

    fn exposure_mode(&self) -> M::Fut<'_, Result<ExposureMode, Error>> {
        use crate::command::inquiry_structs::ExposureModeInquiry;
        self.query(ExposureModeInquiry)
    }

    fn shutter(&self) -> M::Fut<'_, Result<crate::types::ShutterSpeed, Error>> {
        use crate::command::inquiry_structs::ShutterInquiry;
        self.query(ShutterInquiry)
    }

    fn gain(&self) -> M::Fut<'_, Result<crate::types::GainLevel, Error>> {
        use crate::command::inquiry_structs::GainInquiry;
        self.query(GainInquiry)
    }

    fn gain_limit(&self) -> M::Fut<'_, Result<crate::types::GainLimit, Error>> {
        use crate::command::inquiry_structs::GainLimitInquiry;
        self.query(GainLimitInquiry)
    }

    fn white_balance_mode(&self) -> M::Fut<'_, Result<WhiteBalanceMode, Error>> {
        use crate::command::inquiry_structs::WhiteBalanceModeInquiry;
        self.query(WhiteBalanceModeInquiry)
    }

    fn brightness(&self) -> M::Fut<'_, Result<crate::types::BrightnessLevel, Error>> {
        use crate::command::inquiry_structs::BrightnessInquiry;
        self.query(BrightnessInquiry)
    }

    fn sharpness_mode(&self) -> M::Fut<'_, Result<SharpnessMode, Error>> {
        use crate::command::inquiry_structs::SharpnessModeInquiry;
        self.query(SharpnessModeInquiry)
    }

    fn sharpness_level(&self) -> M::Fut<'_, Result<crate::types::SharpnessLevel, Error>> {
        use crate::command::inquiry_structs::SharpnessPositionInquiry;
        self.query(SharpnessPositionInquiry)
    }

    fn contrast(&self) -> M::Fut<'_, Result<crate::types::ContrastLevel, Error>> {
        use crate::command::inquiry_structs::ContrastInquiry;
        self.query(ContrastInquiry)
    }

    fn resolution(&self) -> M::Fut<'_, Result<crate::command::ResolutionMode, Error>> {
        use crate::command::inquiry_structs::ResolutionInquiry;
        self.query(ResolutionInquiry)
    }

    fn version(&self) -> M::Fut<'_, Result<crate::command::VersionInfo, Error>> {
        use crate::command::inquiry_structs::VersionInquiry;
        self.query(VersionInquiry)
    }

    fn focus_mode(&self) -> M::Fut<'_, Result<FocusMode, Error>> {
        use crate::command::inquiry_structs::FocusModeInquiry;
        self.query(FocusModeInquiry)
    }

    fn menu_status(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::MenuOpenCloseInquiry;
        self.query(MenuOpenCloseInquiry)
    }

    fn tally_light_status(&self) -> M::Fut<'_, Result<crate::command::TallyStatusState, Error>> {
        use crate::command::inquiry_structs::TallyStatusInquiry;
        self.query(TallyStatusInquiry)
    }

    fn night_day_mode(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::NightDayModeInquiry;
        self.query(NightDayModeInquiry)
    }

    fn standby_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::StandbyInquiry;
        self.query(StandbyInquiry)
    }

    fn defog_level(&self) -> M::Fut<'_, Result<crate::types::DefogLevel, Error>> {
        use crate::command::inquiry_structs::DefogLevelInquiry;
        self.query(DefogLevelInquiry)
    }

    fn digital_ptz_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::DigitalPtzInquiry;
        self.query(DigitalPtzInquiry)
    }

    fn auto_trace_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::AutoTraceInquiry;
        self.query(AutoTraceInquiry)
    }

    fn focus_unlock(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::FocusUnlockInquiry;
        self.query(FocusUnlockInquiry)
    }

    fn broadcast_domain(&self) -> M::Fut<'_, Result<crate::types::BroadcastDomain, Error>> {
        use crate::command::inquiry_structs::BroadcastDomainInquiry;
        self.query(BroadcastDomainInquiry)
    }

    fn usb_audio_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::UsbAudioInquiry;
        self.query(UsbAudioInquiry)
    }

    fn two_tone_mode_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::TwoToneModeInquiry;
        self.query(TwoToneModeInquiry)
    }

    fn digital_mode_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::DigitalInquiry;
        self.query(DigitalInquiry)
    }

    fn tally_auto_adjust_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::TallyAutoAdjustInquiry;
        self.query(TallyAutoAdjustInquiry)
    }

    fn flicker_mode(&self) -> M::Fut<'_, Result<crate::command::exposure::AntiFlickerMode, Error>> {
        use crate::command::inquiry_structs::FlickerModeInquiry;
        self.query(FlickerModeInquiry)
    }
}

impl<M, P, Tr, Exec> ExposureCompensationInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasExposureCompensation,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn exposure_compensation(
        &self,
    ) -> M::Fut<'_, Result<crate::types::ExposureCompensationLevel, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationInquiry;
        self.query(ExposureCompensationInquiry)
    }

    fn exposure_compensation_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationModeInquiry;
        self.query(ExposureCompensationModeInquiry)
    }

    fn exposure_compensation_position(
        &self,
    ) -> M::Fut<'_, Result<crate::types::ExposureCompensationPosition, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationPositionInquiry;
        self.query(ExposureCompensationPositionInquiry)
    }
}

impl<M, P, Tr, Exec> BacklightCompensationInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasBacklightCompensation,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn backlight_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::BacklightInquiry;
        self.query(BacklightInquiry)
    }
}

impl<M, P, Tr, Exec> WideDynamicRangeInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasWideDynamicRange,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn dynamic_range(&self) -> M::Fut<'_, Result<crate::types::DynamicRangeLevel, Error>> {
        use crate::command::inquiry_structs::DynamicRangeInquiry;
        self.query(DynamicRangeInquiry)
    }
}

impl<M, P, Tr, Exec> ColorTemperatureInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasColorTemperature,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn color_temperature(&self) -> M::Fut<'_, Result<crate::types::ColorTemp, Error>> {
        use crate::command::inquiry_structs::ColorTemperatureInquiry;
        self.query(ColorTemperatureInquiry)
    }
}

impl<M, P, Tr, Exec> RgbGainInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasRgbGain,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn red_gain(&self) -> M::Fut<'_, Result<crate::types::RedChannel, Error>> {
        use crate::command::inquiry_structs::RedGainInquiry;
        self.query(RedGainInquiry)
    }

    fn blue_gain(&self) -> M::Fut<'_, Result<crate::types::BlueChannel, Error>> {
        use crate::command::inquiry_structs::BlueGainInquiry;
        self.query(BlueGainInquiry)
    }
}

impl<M, P, Tr, Exec> RgbTuningInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasRgbTuning,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn red_tuning(&self) -> M::Fut<'_, Result<crate::types::RedTuning, Error>> {
        use crate::command::inquiry_structs::RedTuningInquiry;
        self.query(RedTuningInquiry)
    }

    fn blue_tuning(&self) -> M::Fut<'_, Result<crate::types::BlueTuning, Error>> {
        use crate::command::inquiry_structs::BlueTuningInquiry;
        self.query(BlueTuningInquiry)
    }
}

impl<M, P, Tr, Exec> SaturationInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasSaturationControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn saturation(&self) -> M::Fut<'_, Result<crate::types::SaturationLevel, Error>> {
        use crate::command::inquiry_structs::SaturationInquiry;
        self.query(SaturationInquiry)
    }
}

impl<M, P, Tr, Exec> HueInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasHueControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn hue(&self) -> M::Fut<'_, Result<crate::types::HueLevel, Error>> {
        use crate::command::inquiry_structs::HueInquiry;
        self.query(HueInquiry)
    }
}

impl<M, P, Tr, Exec> LuminanceInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasLuminanceControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn luminance(&self) -> M::Fut<'_, Result<crate::types::LuminanceLevel, Error>> {
        use crate::command::inquiry_structs::LuminanceInquiry;
        self.query(LuminanceInquiry)
    }
}

impl<M, P, Tr, Exec> GammaInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasGammaControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn gamma(&self) -> M::Fut<'_, Result<crate::types::GammaLevel, Error>> {
        use crate::command::inquiry_structs::GammaInquiry;
        self.query(GammaInquiry)
    }
}

impl<M, P, Tr, Exec> ImageFlipInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasImageFlip,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn image_flip(&self) -> M::Fut<'_, Result<crate::command::FlipState, Error>> {
        use crate::command::inquiry_structs::ImageFlipInquiry;
        self.query(ImageFlipInquiry)
    }

    fn flip_mode(&self) -> M::Fut<'_, Result<crate::command::FlipState, Error>> {
        use crate::command::inquiry_structs::FlipStateInquiry;
        self.query(FlipStateInquiry)
    }
}

impl<M, P, Tr, Exec> NoiseReductionInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNoiseReduction,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn noise_reduction_level(
        &self,
    ) -> M::Fut<'_, Result<crate::types::NoiseReductionLevel, Error>> {
        use crate::command::inquiry_structs::NrLevelInquiry;
        self.query(NrLevelInquiry)
    }

    fn noise_reduction_mode(
        &self,
    ) -> M::Fut<'_, Result<crate::command::NoiseReductionMode, Error>> {
        use crate::command::inquiry_structs::NrModeInquiry;
        self.query(NrModeInquiry)
    }
}

impl<M, P, Tr, Exec> NoiseReduction2DInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNoiseReduction2D,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn noise_reduction_2d(&self) -> M::Fut<'_, Result<crate::types::NoiseReduction2DLevel, Error>> {
        use crate::command::inquiry_structs::NoiseReduction2DInquiry;
        self.query(NoiseReduction2DInquiry)
    }
}

impl<M, P, Tr, Exec> NoiseReduction3DInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNoiseReduction3D,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn noise_reduction_3d(&self) -> M::Fut<'_, Result<crate::types::NoiseReduction3DLevel, Error>> {
        use crate::command::inquiry_structs::NoiseReduction3DInquiry;
        self.query(NoiseReduction3DInquiry)
    }
}

impl<M, P, Tr, Exec> PictureEffectInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasPictureEffect,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn black_white(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::BlackWhiteInquiry;
        self.query(BlackWhiteInquiry)
    }

    fn black_white_mode(&self) -> M::Fut<'_, Result<crate::command::BlackWhiteMode, Error>> {
        use crate::command::inquiry_structs::BlackWhiteModeInquiry;
        self.query(BlackWhiteModeInquiry)
    }

    fn picture_effect(&self) -> M::Fut<'_, Result<crate::command::PictureEffectMode, Error>> {
        use crate::command::inquiry_structs::PictureEffectInquiry;
        self.query(PictureEffectInquiry)
    }
}

impl<M, P, Tr, Exec> NdFilterInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasNdFilter,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn nd_filter_position(&self) -> M::Fut<'_, Result<crate::command::NdFilterPosition, Error>> {
        use crate::command::inquiry_structs::NdFilterInquiry;
        self.query(NdFilterInquiry)
    }

    fn nd_filter_preset(&self) -> M::Fut<'_, Result<crate::types::NdFilterPreset, Error>> {
        use crate::command::inquiry_structs::NdFilterPresetInquiry;
        self.query(NdFilterPresetInquiry)
    }
}

impl<M, P, Tr, Exec> FocusNearLimitInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasFocusNearLimitInquiry,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn focus_near_limit(&self) -> M::Fut<'_, Result<crate::types::FocusPosition, Error>> {
        use crate::command::inquiry_structs::FocusNearLimitInquiry;
        self.query(FocusNearLimitInquiry)
    }
}

impl<M, P, Tr, Exec> FocusZoneInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasFocusZone,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn focus_zone(&self) -> M::Fut<'_, Result<FocusZone, Error>> {
        use crate::command::inquiry_structs::FocusZoneInquiry;
        self.query(FocusZoneInquiry)
    }
}

impl<M, P, Tr, Exec> AutoFocusSensitivityInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasAutoFocusSensitivity,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn auto_focus_sensitivity(&self) -> M::Fut<'_, Result<AutoFocusSensitivity, Error>> {
        use crate::command::inquiry_structs::AutoFocusSensitivityInquiry;
        self.query(AutoFocusSensitivityInquiry)
    }
}

impl<M, P, Tr, Exec> IrisInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::HasIrisControl,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn iris_control(&self) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::IrisControlInquiry;
        self.query(IrisControlInquiry)
    }

    fn iris(&self) -> M::Fut<'_, Result<crate::types::IrisLevel, Error>> {
        use crate::command::inquiry_structs::IrisInquiry;
        self.query(IrisInquiry)
    }
}

// Single unified implementation for PanTiltInquiryControl
impl<M, P, Tr, Exec> PanTiltInquiryControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn pan_tilt_position(&self) -> M::Fut<'_, Result<crate::camera::PanTiltPosition, Error>> {
        use crate::command::inquiry_structs::PanTiltPositionInquiry;
        self.query(PanTiltPositionInquiry)
    }
}

macro_rules! validate_builtin_camera_inquiry_accessors {
    (
        queryable { $($query_entries:tt)* }
        decode_only { $($decode_entries:tt)* }
        accessors { $($accessor_groups:tt)* }
    ) => {
        validate_builtin_camera_inquiry_accessors!(@groups $($accessor_groups)*);
    };
    (@groups) => {};
    (@groups
        $trait_name:ident {
            gate: none;
            $(
                $command:ident => $method:ident : $response_ty:ty;
            )*
        }
        $($rest:tt)*
    ) => {
        $(
            validate_builtin_camera_inquiry_accessors!(@one
                $trait_name,
                none,
                $command,
                $method,
                $response_ty
            );
        )*
        validate_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@groups
        $trait_name:ident {
            gate: $profile_gate:path;
            $(
                $command:ident => $method:ident : $response_ty:ty;
            )*
        }
        $($rest:tt)*
    ) => {
        $(
            validate_builtin_camera_inquiry_accessors!(@one
                $trait_name,
                $profile_gate,
                $command,
                $method,
                $response_ty
            );
        )*
        validate_builtin_camera_inquiry_accessors!(@groups $($rest)*);
    };
    (@one $trait_name:ident, none, $command:ident, $method:ident, $response_ty:ty) => {
        const _: () = {
            #[allow(dead_code)]
            fn command_response_matches_accessor()
            where
                crate::command::inquiry_structs::$command:
                    crate::command::ViscaCommand + crate::command::ResponseParser<Response = $response_ty>,
            {
            }

            #[allow(dead_code)]
            fn camera_exposes_accessor<M, P, Tr, Exec>()
            where
                M: crate::mode::Mode,
                P: crate::capabilities::Profile + Default,
                Exec: crate::executor::Executor,
                crate::camera::Camera<M, P, Tr, Exec>: $trait_name<Mode = M>,
            {
                let _method: for<'a> fn(
                    &'a crate::camera::Camera<M, P, Tr, Exec>,
                ) -> M::Fut<'a, Result<$response_ty, crate::Error>> =
                    <crate::camera::Camera<M, P, Tr, Exec> as $trait_name>::$method;
            }
        };
    };
    (@one $trait_name:ident, $profile_gate:path, $command:ident, $method:ident, $response_ty:ty) => {
        const _: () = {
            #[allow(dead_code)]
            fn command_response_matches_accessor()
            where
                crate::command::inquiry_structs::$command:
                    crate::command::ViscaCommand + crate::command::ResponseParser<Response = $response_ty>,
            {
            }

            #[allow(dead_code)]
            fn camera_exposes_accessor<M, P, Tr, Exec>()
            where
                M: crate::mode::Mode,
                P: crate::capabilities::Profile + Default + $profile_gate,
                Exec: crate::executor::Executor,
                crate::camera::Camera<M, P, Tr, Exec>: $trait_name<Mode = M>,
            {
                let _method: for<'a> fn(
                    &'a crate::camera::Camera<M, P, Tr, Exec>,
                ) -> M::Fut<'a, Result<$response_ty, crate::Error>> =
                    <crate::camera::Camera<M, P, Tr, Exec> as $trait_name>::$method;
            }
        };
    };
}

crate::command::inquiry_structs::builtin_inquiry_table!(validate_builtin_camera_inquiry_accessors);
