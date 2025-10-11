//! Inquiry control implementation for PTZ cameras.
//!
//! This module provides comprehensive camera status inquiry functionality including:
//! - Current camera settings and modes (focus, exposure, white balance, etc.)
//! - Position information (pan, tilt, zoom, focus positions)
//! - Image processing settings (sharpness, saturation, noise reduction)
//! - System status (power, version, menu state)
//! - Color settings (gain, tuning, temperature)
//! - Special features (tally lights, ND filters, motion sync)
//!
//! Inquiry operations allow you to read the current state of various camera
//! parameters without changing them. This is essential for building user
//! interfaces, monitoring camera status, and synchronizing settings.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::camera::ViscaClient;
use crate::command::system::MotionSyncMode;
use crate::command::{ExposureMode, FocusMode, FocusZone, SharpnessMode, WhiteBalanceMode};
use crate::mode::Mode;
use crate::Error;

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
/// - **Special**: ND filters, motion sync, digital PTZ, defog
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
    fn power_state(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current zoom position.
    ///
    /// Returns the current zoom level as a position value.
    /// The range depends on the camera model and zoom capabilities.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn zoom_position(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::types::ZoomPosition, Error>>;

    /// Get the current focus position.
    ///
    /// Returns the current focus position as a numeric value.
    /// Higher values typically indicate focus on more distant objects.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_position(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the focus near limit position.
    ///
    /// Returns the minimum focus distance setting that prevents
    /// the camera from focusing on objects too close to the lens.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_near_limit(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the current focus zone.
    ///
    /// Returns which area of the image the camera uses for auto focus detection.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_zone(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<FocusZone, Error>>;

    /// Get the current exposure mode.
    ///
    /// Returns the active exposure mode (auto, manual, shutter priority, etc.).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn exposure_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<ExposureMode, Error>>;

    /// Get the exposure compensation value.
    ///
    /// Returns the current exposure compensation level (-7 to +7).
    /// Positive values make the image brighter, negative values make it darker.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn exposure_compensation(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<i8, Error>>;

    /// Check if exposure compensation is enabled.
    ///
    /// Returns whether exposure compensation is currently active.
    ///
    /// # Returns
    /// - `true` if exposure compensation is enabled
    /// - `false` if exposure compensation is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn exposure_compensation_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current iris value.
    ///
    /// Returns the current iris (aperture) setting. Lower values indicate
    /// a more closed aperture, higher values indicate a more open aperture.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn iris(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the current shutter speed.
    ///
    /// Returns the current shutter speed setting as a numeric value.
    /// The exact interpretation depends on the camera model.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn shutter(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the current gain value.
    ///
    /// Returns the current sensor gain level. Higher values increase
    /// image brightness but also increase noise.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn gain(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the gain limit value.
    ///
    /// Returns the maximum gain level that auto exposure is allowed to use.
    /// This helps control noise in low light conditions.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn gain_limit(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the white balance mode.
    ///
    /// Returns the current white balance mode (auto, indoor, outdoor, etc.).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn white_balance_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<WhiteBalanceMode, Error>>;

    /// Get the current red gain.
    ///
    /// Returns the red color channel gain adjustment value.
    /// Used for fine-tuning color balance.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn red_gain(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<i8, Error>>;

    /// Get the current blue gain.
    ///
    /// Returns the blue color channel gain adjustment value.
    /// Used for fine-tuning color balance.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn blue_gain(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<i8, Error>>;

    /// Get the red tuning value.
    ///
    /// Returns the red color channel tuning adjustment.
    /// Provides finer control than gain adjustment.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn red_tuning(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the blue tuning value.
    ///
    /// Returns the blue color channel tuning adjustment.
    /// Provides finer control than gain adjustment.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn blue_tuning(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the current color temperature in Kelvin.
    ///
    /// Returns the color temperature setting which controls
    /// the overall warmth or coolness of the image.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn color_temperature(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the gamma level.
    ///
    /// Returns the current gamma correction setting which affects
    /// the overall brightness curve and contrast of the image.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn gamma(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the brightness level.
    ///
    /// Returns the current brightness adjustment level.
    /// This is separate from exposure brightness.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn brightness(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

    /// Get the sharpness mode.
    ///
    /// Returns the current sharpness mode setting (auto or manual).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn sharpness_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<SharpnessMode, Error>>;

    /// Get the saturation level.
    ///
    /// Returns the current color saturation level.
    /// Higher values make colors more vivid.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn saturation(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the hue setting.
    ///
    /// Returns the current hue adjustment which shifts
    /// the overall color tone of the image.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn hue(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Check if black and white mode is enabled.
    ///
    /// Returns whether the camera is currently outputting
    /// in monochrome (black and white) mode.
    ///
    /// # Returns
    /// - `true` if black and white mode is active
    /// - `false` if color mode is active
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn black_white(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current video resolution mode.
    ///
    /// Returns the current video resolution setting as a numeric value.
    /// The interpretation depends on the camera model.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn resolution(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the current picture effect mode.
    ///
    /// Returns the active picture effect (normal, negative, sepia, etc.).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn picture_effect(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the current ND filter position.
    ///
    /// Returns the position of the neutral density filter.
    /// Only available on cameras with built-in ND filters (e.g., Sony FR7).
    ///
    /// # Errors
    /// Returns an error if the inquiry fails, times out, or is not supported.
    fn nd_filter_position(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the camera version information.
    ///
    /// Returns detailed version information including model name,
    /// firmware version, and other identifying information.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn version(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::typed::VersionInfo, Error>>;

    /// Check if backlight compensation is enabled.
    ///
    /// Returns whether backlight compensation is currently active
    /// to improve visibility when subjects are backlit.
    ///
    /// # Returns
    /// - `true` if backlight compensation is enabled
    /// - `false` if backlight compensation is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn backlight_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the image flip settings.
    ///
    /// Returns the current image orientation settings including
    /// horizontal and vertical flip states.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn image_flip(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::FlipState, Error>>;

    /// Get the current focus mode.
    ///
    /// Returns whether the camera is in auto focus or manual focus mode.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn focus_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<FocusMode, Error>>;

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
    fn menu_status(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the tally light status.
    ///
    /// Returns the current state of the camera's tally lights
    /// including both red and green indicators.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn tally_light_status(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::typed::TallyStatusState, Error>>;

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
    fn night_day_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the current flip mode (combined horizontal/vertical).
    ///
    /// Returns the combined image flip state for both horizontal
    /// and vertical orientations.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn flip_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::FlipState, Error>>;

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
    fn standby_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the iris control mode.
    ///
    /// Returns the current iris control setting.
    ///
    /// # Returns
    /// - `true` if iris control is enabled
    /// - `false` if iris control is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn iris_control(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the defog level.
    ///
    /// Returns the current defog processing level which helps
    /// improve visibility in foggy or hazy conditions.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn defog_level(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

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
    fn digital_ptz_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the exposure compensation position.
    ///
    /// Returns the detailed position value for exposure compensation
    /// rather than just the level.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn exposure_compensation_position(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u16, Error>>;

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
    fn auto_trace_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

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
    fn focus_unlock(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the noise reduction level.
    ///
    /// Returns the current general noise reduction level setting.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn noise_reduction_level(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the noise reduction 2D level.
    ///
    /// Returns the current spatial (2D) noise reduction level
    /// which processes individual frames.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn noise_reduction_2d(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the noise reduction 3D level.
    ///
    /// Returns the current temporal (3D) noise reduction level
    /// which processes across multiple frames.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn noise_reduction_3d(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the broadcast domain setting.
    ///
    /// Returns the current broadcast domain configuration
    /// which may affect video output standards.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn broadcast_domain(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

    /// Get the noise reduction mode setting.
    ///
    /// Returns the current noise reduction mode configuration
    /// which determines how noise reduction is applied.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn noise_reduction_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::NoiseReductionMode, Error>>;

    /// Get the black and white mode setting.
    ///
    /// Returns the detailed black and white mode configuration
    /// rather than just the enabled/disabled state.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn black_white_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::BlackWhiteMode, Error>>;

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
    fn usb_audio_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

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
    fn two_tone_mode_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the ND filter preset setting.
    ///
    /// Returns the current ND filter preset number for cameras
    /// with multiple ND filter configurations.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn nd_filter_preset(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<u8, Error>>;

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
    fn digital_mode_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

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
    fn tally_auto_adjust_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Get the motion sync mode setting.
    ///
    /// Returns the current motion sync mode which affects how
    /// the camera synchronizes movement with other devices.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn motion_sync_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<MotionSyncMode, Error>>;
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
        opts: crate::CommandOptions<'_>,
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

    fn power_state(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::PowerInquiry;
        self.query_with_opts(PowerInquiry, opts)
    }

    fn zoom_position(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<crate::types::ZoomPosition, Error>> {
        use crate::command::inquiry_structs::ZoomPositionInquiry;
        self.query_with_opts(ZoomPositionInquiry, opts)
    }

    fn focus_position(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::FocusPositionInquiry;
        self.query_with_opts(FocusPositionInquiry, opts)
    }

    fn focus_near_limit(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::FocusNearLimitInquiry;
        self.query_with_opts(FocusNearLimitInquiry, opts)
    }

    fn focus_zone(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<FocusZone, Error>> {
        use crate::command::inquiry_structs::FocusZoneInquiry;
        self.query_with_opts(FocusZoneInquiry, opts)
    }

    fn exposure_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<ExposureMode, Error>> {
        use crate::command::inquiry_structs::ExposureModeInquiry;
        self.query_with_opts(ExposureModeInquiry, opts)
    }

    fn exposure_compensation(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationInquiry;
        self.query_with_opts(ExposureCompensationInquiry, opts)
    }

    fn exposure_compensation_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationModeInquiry;
        self.query_with_opts(ExposureCompensationModeInquiry, opts)
    }

    fn iris(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::IrisInquiry;
        self.query_with_opts(IrisInquiry, opts)
    }

    fn shutter(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ShutterInquiry;
        self.query_with_opts(ShutterInquiry, opts)
    }

    fn gain(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GainInquiry;
        self.query_with_opts(GainInquiry, opts)
    }

    fn gain_limit(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GainLimitInquiry;
        self.query_with_opts(GainLimitInquiry, opts)
    }

    fn white_balance_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<WhiteBalanceMode, Error>> {
        use crate::command::inquiry_structs::WhiteBalanceModeInquiry;
        self.query_with_opts(WhiteBalanceModeInquiry, opts)
    }

    fn red_gain(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::RedGainInquiry;
        self.query_with_opts(RedGainInquiry, opts)
    }

    fn blue_gain(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<i8, Error>> {
        use crate::command::inquiry_structs::BlueGainInquiry;
        self.query_with_opts(BlueGainInquiry, opts)
    }

    fn red_tuning(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::RedTuningInquiry;
        self.query_with_opts(RedTuningInquiry, opts)
    }

    fn blue_tuning(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::BlueTuningInquiry;
        self.query_with_opts(BlueTuningInquiry, opts)
    }

    fn color_temperature(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ColorTemperatureInquiry;
        self.query_with_opts(ColorTemperatureInquiry, opts)
    }

    fn gamma(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::GammaInquiry;
        self.query_with_opts(GammaInquiry, opts)
    }

    fn brightness(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::BrightnessInquiry;
        self.query_with_opts(BrightnessInquiry, opts)
    }

    fn sharpness_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<SharpnessMode, Error>> {
        use crate::command::inquiry_structs::SharpnessModeInquiry;
        self.query_with_opts(SharpnessModeInquiry, opts)
    }

    fn saturation(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::SaturationInquiry;
        self.query_with_opts(SaturationInquiry, opts)
    }

    fn hue(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::HueInquiry;
        self.query_with_opts(HueInquiry, opts)
    }

    fn black_white(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::BlackWhiteInquiry;
        self.query_with_opts(BlackWhiteInquiry, opts)
    }

    fn resolution(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::ResolutionInquiry;
        self.query_with_opts(ResolutionInquiry, opts)
    }

    fn picture_effect(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::PictureEffectInquiry;
        self.query_with_opts(PictureEffectInquiry, opts)
    }

    fn nd_filter_position(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NdFilterInquiry;
        self.query_with_opts(NdFilterInquiry, opts)
    }

    fn version(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<crate::command::typed::VersionInfo, Error>> {
        use crate::command::inquiry_structs::VersionInquiry;
        self.query_with_opts(VersionInquiry, opts)
    }

    fn backlight_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::BacklightInquiry;
        self.query_with_opts(BacklightInquiry, opts)
    }

    fn image_flip(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<crate::command::FlipState, Error>> {
        use crate::command::inquiry_structs::ImageFlipInquiry;
        self.query_with_opts(ImageFlipInquiry, opts)
    }

    fn focus_mode(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<FocusMode, Error>> {
        use crate::command::inquiry_structs::FocusModeInquiry;
        self.query_with_opts(FocusModeInquiry, opts)
    }

    fn menu_status(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::MenuOpenCloseInquiry;
        self.query_with_opts(MenuOpenCloseInquiry, opts)
    }

    fn tally_light_status(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<crate::command::typed::TallyStatusState, Error>> {
        use crate::command::inquiry_structs::TallyStatusInquiry;
        self.query_with_opts(TallyStatusInquiry, opts)
    }

    fn night_day_mode(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::NightDayModeInquiry;
        self.query_with_opts(NightDayModeInquiry, opts)
    }

    fn flip_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<crate::command::FlipState, Error>> {
        use crate::command::inquiry_structs::FlipStateInquiry;
        self.query_with_opts(FlipStateInquiry, opts)
    }

    fn standby_enabled(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::StandbyInquiry;
        self.query_with_opts(StandbyInquiry, opts)
    }

    fn iris_control(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::IrisControlInquiry;
        self.query_with_opts(IrisControlInquiry, opts)
    }

    fn defog_level(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::DefogLevelInquiry;
        self.query_with_opts(DefogLevelInquiry, opts)
    }

    fn digital_ptz_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::DigitalPtzInquiry;
        self.query_with_opts(DigitalPtzInquiry, opts)
    }

    fn exposure_compensation_position(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<u16, Error>> {
        use crate::command::inquiry_structs::ExposureCompensationPositionInquiry;
        self.query_with_opts(ExposureCompensationPositionInquiry, opts)
    }

    fn auto_trace_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::AutoTraceInquiry;
        self.query_with_opts(AutoTraceInquiry, opts)
    }

    fn focus_unlock(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::FocusUnlockInquiry;
        self.query_with_opts(FocusUnlockInquiry, opts)
    }

    fn noise_reduction_level(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NrLevelInquiry;
        self.query_with_opts(NrLevelInquiry, opts)
    }

    fn noise_reduction_2d(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NoiseReduction2DInquiry;
        self.query_with_opts(NoiseReduction2DInquiry, opts)
    }

    fn noise_reduction_3d(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NoiseReduction3DInquiry;
        self.query_with_opts(NoiseReduction3DInquiry, opts)
    }

    fn broadcast_domain(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::BroadcastDomainInquiry;
        self.query_with_opts(BroadcastDomainInquiry, opts)
    }

    fn noise_reduction_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<crate::command::NoiseReductionMode, Error>> {
        use crate::command::inquiry_structs::NrModeInquiry;
        self.query_with_opts(NrModeInquiry, opts)
    }

    fn black_white_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<crate::command::BlackWhiteMode, Error>> {
        use crate::command::inquiry_structs::BlackWhiteModeInquiry;
        self.query_with_opts(BlackWhiteModeInquiry, opts)
    }

    fn usb_audio_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::UsbAudioInquiry;
        self.query_with_opts(UsbAudioInquiry, opts)
    }

    fn two_tone_mode_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::TwoToneModeInquiry;
        self.query_with_opts(TwoToneModeInquiry, opts)
    }

    fn nd_filter_preset(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<u8, Error>> {
        use crate::command::inquiry_structs::NdFilterPresetInquiry;
        self.query_with_opts(NdFilterPresetInquiry, opts)
    }

    fn digital_mode_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::DigitalInquiry;
        self.query_with_opts(DigitalInquiry, opts)
    }

    fn tally_auto_adjust_enabled(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<bool, Error>> {
        use crate::command::inquiry_structs::TallyAutoAdjustInquiry;
        self.query_with_opts(TallyAutoAdjustInquiry, opts)
    }

    fn motion_sync_mode(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<MotionSyncMode, Error>> {
        use crate::command::inquiry_structs::MotionSyncModeInquiry;
        self.query_with_opts(MotionSyncModeInquiry, opts)
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

    fn pan_tilt_position(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<crate::camera::PanTiltPosition, Error>> {
        use crate::command::inquiry_structs::PanTiltPositionInquiry;
        self.query_with_opts(PanTiltPositionInquiry, opts)
    }
}
