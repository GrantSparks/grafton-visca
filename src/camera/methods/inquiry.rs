//! Inquiry methods for querying camera state using the unified API architecture.

use crate::{
    command::{
        inquiry::*, response::ViscaResponse, AutoFocusSensitivity, ExposureMode, FocusMode,
        FocusZone, InquiryResponse, SharpnessMode, WhiteBalanceMode,
    },
    Error,
};

/// Inquiry operations for cameras.
///
/// This trait provides inquiry methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait InquiryControl {
    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    #[cfg(feature = "async")]
    fn get_power_state(&self)
        -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    #[cfg(not(feature = "async"))]
    fn get_power_state(&mut self) -> Result<bool, Error>;

    /// Get the current zoom position.
    #[cfg(feature = "async")]
    fn get_zoom_position(
        &self,
    ) -> impl std::future::Future<Output = Result<u16, Error>> + Send + '_;

    /// Get the current zoom position.
    #[cfg(not(feature = "async"))]
    fn get_zoom_position(&mut self) -> Result<u16, Error>;

    /// Get the current focus position.
    #[cfg(feature = "async")]
    fn get_focus_position(
        &self,
    ) -> impl std::future::Future<Output = Result<u16, Error>> + Send + '_;

    /// Get the current focus position.
    #[cfg(not(feature = "async"))]
    fn get_focus_position(&mut self) -> Result<u16, Error>;

    /// Get the focus near limit position.
    #[cfg(feature = "async")]
    fn get_focus_near_limit(
        &self,
    ) -> impl std::future::Future<Output = Result<u16, Error>> + Send + '_;

    /// Get the focus near limit position.
    #[cfg(not(feature = "async"))]
    fn get_focus_near_limit(&mut self) -> Result<u16, Error>;

    /// Get the current focus zone.
    #[cfg(feature = "async")]
    fn get_focus_zone(
        &self,
    ) -> impl std::future::Future<Output = Result<FocusZone, Error>> + Send + '_;

    /// Get the current focus zone.
    #[cfg(not(feature = "async"))]
    fn get_focus_zone(&mut self) -> Result<FocusZone, Error>;

    /// Get the auto-focus sensitivity setting.
    #[cfg(feature = "async")]
    fn get_auto_focus_sensitivity(
        &self,
    ) -> impl std::future::Future<Output = Result<AutoFocusSensitivity, Error>> + Send + '_;

    /// Get the auto-focus sensitivity setting.
    #[cfg(not(feature = "async"))]
    fn get_auto_focus_sensitivity(&mut self) -> Result<AutoFocusSensitivity, Error>;

    /// Get the current exposure mode.
    #[cfg(feature = "async")]
    fn get_exposure_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<ExposureMode, Error>> + Send + '_;

    /// Get the current exposure mode.
    #[cfg(not(feature = "async"))]
    fn get_exposure_mode(&mut self) -> Result<ExposureMode, Error>;

    /// Get the exposure compensation value.
    #[cfg(feature = "async")]
    fn get_exposure_compensation(
        &self,
    ) -> impl std::future::Future<Output = Result<i8, Error>> + Send + '_;

    /// Get the exposure compensation value.
    #[cfg(not(feature = "async"))]
    fn get_exposure_compensation(&mut self) -> Result<i8, Error>;

    /// Check if exposure compensation is enabled.
    #[cfg(feature = "async")]
    fn get_exposure_compensation_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Check if exposure compensation is enabled.
    #[cfg(not(feature = "async"))]
    fn get_exposure_compensation_enabled(&mut self) -> Result<bool, Error>;

    /// Get the current iris value.
    #[cfg(feature = "async")]
    fn get_iris(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the current iris value.
    #[cfg(not(feature = "async"))]
    fn get_iris(&mut self) -> Result<u8, Error>;

    /// Get the current shutter speed.
    #[cfg(feature = "async")]
    fn get_shutter(&self) -> impl std::future::Future<Output = Result<u16, Error>> + Send + '_;

    /// Get the current shutter speed.
    #[cfg(not(feature = "async"))]
    fn get_shutter(&mut self) -> Result<u16, Error>;

    /// Get the current gain value.
    #[cfg(feature = "async")]
    fn get_gain(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the current gain value.
    #[cfg(not(feature = "async"))]
    fn get_gain(&mut self) -> Result<u8, Error>;

    /// Get the gain limit value.
    #[cfg(feature = "async")]
    fn get_gain_limit(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the gain limit value.
    #[cfg(not(feature = "async"))]
    fn get_gain_limit(&mut self) -> Result<u8, Error>;

    /// Get the white balance mode.
    #[cfg(feature = "async")]
    fn get_white_balance_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<WhiteBalanceMode, Error>> + Send + '_;

    /// Get the white balance mode.
    #[cfg(not(feature = "async"))]
    fn get_white_balance_mode(&mut self) -> Result<WhiteBalanceMode, Error>;

    /// Get the current red gain.
    #[cfg(feature = "async")]
    fn get_red_gain(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the current red gain.
    #[cfg(not(feature = "async"))]
    fn get_red_gain(&mut self) -> Result<u8, Error>;

    /// Get the current blue gain.
    #[cfg(feature = "async")]
    fn get_blue_gain(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the current blue gain.
    #[cfg(not(feature = "async"))]
    fn get_blue_gain(&mut self) -> Result<u8, Error>;

    /// Get the red tuning value.
    #[cfg(feature = "async")]
    fn get_red_tuning(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the red tuning value.
    #[cfg(not(feature = "async"))]
    fn get_red_tuning(&mut self) -> Result<u8, Error>;

    /// Get the blue tuning value.
    #[cfg(feature = "async")]
    fn get_blue_tuning(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the blue tuning value.
    #[cfg(not(feature = "async"))]
    fn get_blue_tuning(&mut self) -> Result<u8, Error>;

    /// Get the current color temperature in Kelvin.
    #[cfg(feature = "async")]
    fn get_color_temperature(
        &self,
    ) -> impl std::future::Future<Output = Result<u16, Error>> + Send + '_;

    /// Get the current color temperature in Kelvin.
    #[cfg(not(feature = "async"))]
    fn get_color_temperature(&mut self) -> Result<u16, Error>;

    /// Get the gamma level.
    #[cfg(feature = "async")]
    fn get_gamma(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the gamma level.
    #[cfg(not(feature = "async"))]
    fn get_gamma(&mut self) -> Result<u8, Error>;

    /// Get the brightness level.
    #[cfg(feature = "async")]
    fn get_brightness(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the brightness level.
    #[cfg(not(feature = "async"))]
    fn get_brightness(&mut self) -> Result<u8, Error>;

    /// Get the sharpness mode.
    #[cfg(feature = "async")]
    fn get_sharpness_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<SharpnessMode, Error>> + Send + '_;

    /// Get the sharpness mode.
    #[cfg(not(feature = "async"))]
    fn get_sharpness_mode(&mut self) -> Result<SharpnessMode, Error>;

    /// Get the saturation level.
    #[cfg(feature = "async")]
    fn get_saturation(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the saturation level.
    #[cfg(not(feature = "async"))]
    fn get_saturation(&mut self) -> Result<u8, Error>;

    /// Get the hue setting.
    #[cfg(feature = "async")]
    fn get_hue(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the hue setting.
    #[cfg(not(feature = "async"))]
    fn get_hue(&mut self) -> Result<u8, Error>;

    /// Get the 2D noise reduction level.
    #[cfg(feature = "async")]
    fn get_noise_reduction_2d(
        &self,
    ) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the 2D noise reduction level.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_2d(&mut self) -> Result<u8, Error>;

    /// Get the 3D noise reduction level.
    #[cfg(feature = "async")]
    fn get_noise_reduction_3d(
        &self,
    ) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the 3D noise reduction level.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_3d(&mut self) -> Result<u8, Error>;

    /// Check if black and white mode is enabled.
    #[cfg(feature = "async")]
    fn get_black_white(&self)
        -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Check if black and white mode is enabled.
    #[cfg(not(feature = "async"))]
    fn get_black_white(&mut self) -> Result<bool, Error>;

    /// Get the current video resolution mode.
    #[cfg(feature = "async")]
    fn get_resolution(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::resolution::ResolutionMode, Error>>
           + Send
           + '_;

    /// Get the current video resolution mode.
    #[cfg(not(feature = "async"))]
    fn get_resolution(&mut self) -> Result<crate::command::resolution::ResolutionMode, Error>;

    /// Get the current picture effect mode.
    #[cfg(feature = "async")]
    fn get_picture_effect(
        &self,
    ) -> impl std::future::Future<
        Output = Result<crate::command::resolution::PictureEffectMode, Error>,
    > + Send
           + '_;

    /// Get the current picture effect mode.
    #[cfg(not(feature = "async"))]
    fn get_picture_effect(
        &mut self,
    ) -> Result<crate::command::resolution::PictureEffectMode, Error>;

    /// Get the current ND filter position (Sony FR7 only).
    #[cfg(feature = "async")]
    fn get_nd_filter_position(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::resolution::NDFilterPosition, Error>>
           + Send
           + '_;

    /// Get the current ND filter position (Sony FR7 only).
    #[cfg(not(feature = "async"))]
    fn get_nd_filter_position(
        &mut self,
    ) -> Result<crate::command::resolution::NDFilterPosition, Error>;

    /// Get the camera version information.
    #[cfg(feature = "async")]
    fn get_version(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::Version, Error>> + Send + '_;

    /// Get the camera version information.
    #[cfg(not(feature = "async"))]
    fn get_version(&mut self) -> Result<crate::command::Version, Error>;

    /// Check if backlight compensation is enabled.
    #[cfg(feature = "async")]
    fn get_backlight_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Check if backlight compensation is enabled.
    #[cfg(not(feature = "async"))]
    fn get_backlight_enabled(&mut self) -> Result<bool, Error>;

    /// Get the image flip settings (mirror/reverse).
    #[cfg(feature = "async")]
    fn get_image_flip(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::ImageFlipStatus, Error>> + Send + '_;

    /// Get the image flip settings (mirror/reverse).
    #[cfg(not(feature = "async"))]
    fn get_image_flip(&mut self) -> Result<crate::command::ImageFlipStatus, Error>;

    /// Get the dynamic range mode/level.
    #[cfg(feature = "async")]
    fn get_dynamic_range(&self)
        -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the dynamic range mode/level.
    #[cfg(not(feature = "async"))]
    fn get_dynamic_range(&mut self) -> Result<u8, Error>;

    /// Get the current focus mode (Auto/Manual).
    #[cfg(feature = "async")]
    fn get_focus_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<FocusMode, Error>> + Send + '_;

    /// Get the current focus mode (Auto/Manual).
    #[cfg(not(feature = "async"))]
    fn get_focus_mode(&mut self) -> Result<FocusMode, Error>;

    /// Get the menu open/close status.
    #[cfg(feature = "async")]
    fn get_menu_status(&self)
        -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the menu open/close status.
    #[cfg(not(feature = "async"))]
    fn get_menu_status(&mut self) -> Result<bool, Error>;

    /// Get the auto focus on/off status.
    #[cfg(feature = "async")]
    fn get_auto_focus_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the auto focus on/off status.
    #[cfg(not(feature = "async"))]
    fn get_auto_focus_enabled(&mut self) -> Result<bool, Error>;

    /// Get the tally light status (red and green).
    #[cfg(feature = "async")]
    fn get_tally_light_status(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::TallyStatus, Error>> + Send + '_;

    /// Get the tally light status (red and green).
    #[cfg(not(feature = "async"))]
    fn get_tally_light_status(&mut self) -> Result<crate::command::TallyStatus, Error>;

    /// Get the night/day mode status.
    #[cfg(feature = "async")]
    fn get_night_day_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::NightDayMode, Error>> + Send + '_;

    /// Get the night/day mode status.
    #[cfg(not(feature = "async"))]
    fn get_night_day_mode(&mut self) -> Result<crate::command::NightDayMode, Error>;

    /// Get the current flip mode (combined horizontal/vertical).
    #[cfg(feature = "async")]
    fn get_flip_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::FlipMode, Error>> + Send + '_;

    /// Get the current flip mode (combined horizontal/vertical).
    #[cfg(not(feature = "async"))]
    fn get_flip_mode(&mut self) -> Result<crate::command::FlipMode, Error>;

    /// Get the standby mode status.
    #[cfg(feature = "async")]
    fn get_standby_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the standby mode status.
    #[cfg(not(feature = "async"))]
    fn get_standby_enabled(&mut self) -> Result<bool, Error>;

    /// Get the focus range setting.
    #[cfg(feature = "async")]
    fn get_focus_range(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::FocusRange, Error>> + Send + '_;

    /// Get the focus range setting.
    #[cfg(not(feature = "async"))]
    fn get_focus_range(&mut self) -> Result<crate::command::FocusRange, Error>;

    /// Get the iris control mode.
    #[cfg(feature = "async")]
    fn get_iris_control(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::IrisControl, Error>> + Send + '_;

    /// Get the iris control mode.
    #[cfg(not(feature = "async"))]
    fn get_iris_control(&mut self) -> Result<crate::command::IrisControl, Error>;

    /// Get the defog mode status.
    #[cfg(feature = "async")]
    fn get_defog_mode(&self) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the defog mode status.
    #[cfg(not(feature = "async"))]
    fn get_defog_mode(&mut self) -> Result<bool, Error>;

    /// Get the defog level.
    #[cfg(feature = "async")]
    fn get_defog_level(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the defog level.
    #[cfg(not(feature = "async"))]
    fn get_defog_level(&mut self) -> Result<u8, Error>;

    /// Get the digital Ptz mode status.
    #[cfg(feature = "async")]
    fn get_digital_ptz_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the digital Ptz mode status.
    #[cfg(not(feature = "async"))]
    fn get_digital_ptz_enabled(&mut self) -> Result<bool, Error>;

    /// Get the auto white balance sensitivity setting.
    #[cfg(feature = "async")]
    fn get_auto_white_balance_sensitivity(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::AutoWhiteBalanceSensitivity, Error>>
           + Send
           + '_;

    /// Get the auto white balance sensitivity setting.
    #[cfg(not(feature = "async"))]
    fn get_auto_white_balance_sensitivity(
        &mut self,
    ) -> Result<crate::command::AutoWhiteBalanceSensitivity, Error>;

    /// Get the exposure compensation position.
    #[cfg(feature = "async")]
    fn get_exposure_compensation_position(
        &self,
    ) -> impl std::future::Future<Output = Result<u16, Error>> + Send + '_;

    /// Get the exposure compensation position.
    #[cfg(not(feature = "async"))]
    fn get_exposure_compensation_position(&mut self) -> Result<u16, Error>;

    /// Get the auto trace mode status.
    #[cfg(feature = "async")]
    fn get_auto_trace_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the auto trace mode status.
    #[cfg(not(feature = "async"))]
    fn get_auto_trace_enabled(&mut self) -> Result<bool, Error>;

    /// Get the focus unlock state.
    #[cfg(feature = "async")]
    fn get_focus_unlock(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the focus unlock state.
    #[cfg(not(feature = "async"))]
    fn get_focus_unlock(&mut self) -> Result<bool, Error>;

    /// Get the current sharpness position.
    #[cfg(feature = "async")]
    fn get_sharpness_position(
        &self,
    ) -> impl std::future::Future<Output = Result<u16, Error>> + Send + '_;

    /// Get the current sharpness position.
    #[cfg(not(feature = "async"))]
    fn get_sharpness_position(&mut self) -> Result<u16, Error>;

    /// Get the noise reduction level.
    #[cfg(feature = "async")]
    fn get_noise_reduction_level(
        &self,
    ) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the noise reduction level.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_level(&mut self) -> Result<u8, Error>;

    /// Get the broadcast domain setting.
    #[cfg(feature = "async")]
    fn get_broadcast_domain(
        &self,
    ) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the broadcast domain setting.
    #[cfg(not(feature = "async"))]
    fn get_broadcast_domain(&mut self) -> Result<u8, Error>;

    /// Get the noise reduction mode setting.
    #[cfg(feature = "async")]
    fn get_noise_reduction_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::NrMode, Error>> + Send + '_;

    /// Get the noise reduction mode setting.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_mode(&mut self) -> Result<crate::command::NrMode, Error>;

    /// Get the noise reduction speed setting.
    #[cfg(feature = "async")]
    fn get_noise_reduction_speed(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::NrSpeed, Error>> + Send + '_;

    /// Get the noise reduction speed setting.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_speed(&mut self) -> Result<crate::command::NrSpeed, Error>;

    /// Get the black and white mode setting.
    #[cfg(feature = "async")]
    fn get_black_white_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<crate::command::BlackWhiteMode, Error>> + Send + '_;

    /// Get the black and white mode setting.
    #[cfg(not(feature = "async"))]
    fn get_black_white_mode(&mut self) -> Result<crate::command::BlackWhiteMode, Error>;

    /// Get the USB audio state.
    #[cfg(feature = "async")]
    fn get_usb_audio_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the USB audio state.
    #[cfg(not(feature = "async"))]
    fn get_usb_audio_enabled(&mut self) -> Result<bool, Error>;

    /// Get the two tone mode state.
    #[cfg(feature = "async")]
    fn get_two_tone_mode_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the two tone mode state.
    #[cfg(not(feature = "async"))]
    fn get_two_tone_mode_enabled(&mut self) -> Result<bool, Error>;

    /// Get the ND filter preset setting.
    #[cfg(feature = "async")]
    fn get_nd_filter_preset(
        &self,
    ) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get the ND filter preset setting.
    #[cfg(not(feature = "async"))]
    fn get_nd_filter_preset(&mut self) -> Result<u8, Error>;

    /// Get the digital mode state.
    #[cfg(feature = "async")]
    fn get_digital_mode_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the digital mode state.
    #[cfg(not(feature = "async"))]
    fn get_digital_mode_enabled(&mut self) -> Result<bool, Error>;

    /// Get the tally auto adjust state.
    #[cfg(feature = "async")]
    fn get_tally_auto_adjust_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the tally auto adjust state.
    #[cfg(not(feature = "async"))]
    fn get_tally_auto_adjust_enabled(&mut self) -> Result<bool, Error>;

    /// Get the green tally light status (FR7 only).
    #[cfg(feature = "async")]
    fn get_tally_green_enabled(
        &self,
    ) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Get the green tally light status (FR7 only).
    #[cfg(not(feature = "async"))]
    fn get_tally_green_enabled(&mut self) -> Result<bool, Error>;
}

/// Pan/Tilt-specific inquiry operations for cameras.
///
/// This trait provides pan/tilt inquiry methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait PanTiltInquiryControl {
    /// Get the current pan and tilt position.
    #[cfg(feature = "async")]
    fn get_pan_tilt_position(
        &self,
    ) -> impl std::future::Future<Output = Result<(i16, i16), Error>> + Send + '_;

    /// Get the current pan and tilt position.
    #[cfg(not(feature = "async"))]
    fn get_pan_tilt_position(&mut self) -> Result<(i16, i16), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, Tr, Exec> InquiryControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn get_power_state(&self) -> Result<bool, Error> {
        let cmd = PowerInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Power { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_zoom_position(&self) -> Result<u16, Error> {
        let cmd = ZoomPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ZoomPosition { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_position(&self) -> Result<u16, Error> {
        let cmd = FocusPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusPosition { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_near_limit(&self) -> Result<u16, Error> {
        let cmd = FocusNearLimitInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_zone(&self) -> Result<FocusZone, Error> {
        let cmd = FocusZoneInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusZone { zone }) => Ok(zone),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error> {
        let cmd = AutoFocusSensitivityInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_mode(&self) -> Result<ExposureMode, Error> {
        let cmd = ExposureModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_compensation(&self) -> Result<i8, Error> {
        let cmd = ExposureCompensationInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        let cmd = ExposureCompensationModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ExposureCompensationMode { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_iris(&self) -> Result<u8, Error> {
        let cmd = IrisInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Iris { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_shutter(&self) -> Result<u16, Error> {
        let cmd = ShutterInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Shutter { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gain(&self) -> Result<u8, Error> {
        let cmd = GainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::GainLevel { gain }) => Ok(gain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gain_limit(&self) -> Result<u8, Error> {
        let cmd = GainLimitInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::GainLimit { limit }) => Ok(limit),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error> {
        let cmd = WhiteBalanceModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::WhiteBalanceMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_red_gain(&self) -> Result<u8, Error> {
        let cmd = RedGainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::RedChannel { gain }) => Ok(gain as u8),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_blue_gain(&self) -> Result<u8, Error> {
        let cmd = BlueGainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BlueChannel { gain }) => Ok(gain as u8),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_red_tuning(&self) -> Result<u8, Error> {
        let cmd = RedTuningInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::RedTuning { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_blue_tuning(&self) -> Result<u8, Error> {
        let cmd = BlueTuningInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BlueTuning { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_color_temperature(&self) -> Result<u16, Error> {
        let cmd = ColorTemperatureInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ColorTemperature { temperature }) => {
                Ok(temperature)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gamma(&self) -> Result<u8, Error> {
        let cmd = GammaInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Gamma { value }) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_brightness(&self) -> Result<u8, Error> {
        let cmd = BrightInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Bright { position }) => Ok(position as u8),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error> {
        let cmd = SharpnessModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_saturation(&self) -> Result<u8, Error> {
        let cmd = SaturationInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Saturation { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_hue(&self) -> Result<u8, Error> {
        let cmd = HueInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Hue { hue }) => Ok(hue),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_2d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction2DInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_3d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction3DInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_black_white(&self) -> Result<bool, Error> {
        let cmd = BlackWhiteInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BlackWhite { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_resolution(&self) -> Result<crate::command::resolution::ResolutionMode, Error> {
        let cmd = ResolutionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Resolution(mode_byte)) => Ok(
                crate::command::resolution::ResolutionMode::from_byte(mode_byte),
            ),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_picture_effect(
        &self,
    ) -> Result<crate::command::resolution::PictureEffectMode, Error> {
        let cmd = PictureEffectInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::PictureEffect { effect }) => Ok(
                crate::command::resolution::PictureEffectMode::from_byte(effect),
            ),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_nd_filter_position(
        &self,
    ) -> Result<crate::command::resolution::NDFilterPosition, Error> {
        let cmd = NdFilterInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NdFilter { position }) => Ok(
                crate::command::resolution::NDFilterPosition::from_byte(position),
            ),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_version(&self) -> Result<crate::command::Version, Error> {
        let cmd = VersionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            }) => Ok(crate::command::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_backlight_enabled(&self) -> Result<bool, Error> {
        let cmd = BacklightInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Backlight { status }) => Ok(status),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_image_flip(&self) -> Result<crate::command::ImageFlipStatus, Error> {
        let cmd = ImageFlipInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok(crate::command::ImageFlipStatus {
                vertical,
                horizontal,
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_dynamic_range(&self) -> Result<u8, Error> {
        let cmd = DynamicRangeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::DynamicRange { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_mode(&self) -> Result<FocusMode, Error> {
        let cmd = FocusModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_menu_status(&self) -> Result<bool, Error> {
        let cmd = MenuOpenCloseInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::MenuOpenClose { is_open }) => Ok(is_open),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_auto_focus_enabled(&self) -> Result<bool, Error> {
        // TODO: AutoFocusInquiry struct is not available in inquiry_structs.rs
        // We can derive this from FocusModeInquiry instead
        let cmd = FocusModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusMode { mode }) => {
                Ok(mode == FocusMode::Auto)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_tally_light_status(&self) -> Result<crate::command::TallyStatus, Error> {
        let cmd = TallyStatusInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyStatus { red_on, green_on }) => {
                Ok(crate::command::TallyStatus { red_on, green_on })
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_night_day_mode(&self) -> Result<crate::command::NightDayMode, Error> {
        let cmd = NightDayModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NightDayMode { is_night }) => Ok(if is_night {
                crate::command::NightDayMode::Night
            } else {
                crate::command::NightDayMode::Day
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_flip_mode(&self) -> Result<crate::command::FlipMode, Error> {
        let cmd = FlipModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FlipMode {
                horizontal,
                vertical,
            }) => Ok(crate::command::FlipMode {
                horizontal,
                vertical,
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_standby_enabled(&self) -> Result<bool, Error> {
        let cmd = StandbyInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Standby { in_standby }) => Ok(in_standby),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_range(&self) -> Result<crate::command::FocusRange, Error> {
        let cmd = FocusRangeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusRange { range }) => Ok(range),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_iris_control(&self) -> Result<crate::command::IrisControl, Error> {
        let cmd = IrisControlInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::IrisControl { auto }) => Ok(if auto {
                crate::command::IrisControl::Auto
            } else {
                crate::command::IrisControl::Manual
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_defog_mode(&self) -> Result<bool, Error> {
        let cmd = DefogModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::DefogMode { enabled }) => Ok(enabled),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_defog_level(&self) -> Result<u8, Error> {
        let cmd = DefogLevelInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::DefogLevel { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_digital_ptz_enabled(&self) -> Result<bool, Error> {
        let cmd = DigitalPtzInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::DigitalPtz { enabled }) => Ok(enabled),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_auto_white_balance_sensitivity(
        &self,
    ) -> Result<crate::command::AutoWhiteBalanceSensitivity, Error> {
        let cmd = AutoWhiteBalanceSensitivityInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::AutoWhiteBalanceSensitivity {
                sensitivity,
            }) => Ok(sensitivity),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_compensation_position(&self) -> Result<u16, Error> {
        let cmd = ExposureCompensationPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ExposureCompensationPosition { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_auto_trace_enabled(&self) -> Result<bool, Error> {
        let cmd = AutoTraceInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::AutoTrace { enabled }) => Ok(enabled),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_unlock(&self) -> Result<bool, Error> {
        let cmd = FocusUnlockInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusUnlock { unlocked }) => Ok(unlocked),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_sharpness_position(&self) -> Result<u16, Error> {
        let cmd = SharpnessPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::SharpnessPosition { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_level(&self) -> Result<u8, Error> {
        let cmd = NrLevelInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NrLevel(level)) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_broadcast_domain(&self) -> Result<u8, Error> {
        let cmd = BroadcastDomainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BroadcastDomain(domain)) => Ok(domain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_mode(&self) -> Result<crate::command::NrMode, Error> {
        let cmd = NrModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NrMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_speed(&self) -> Result<crate::command::NrSpeed, Error> {
        let cmd = NrSpeedInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NrSpeed { speed }) => Ok(speed),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_black_white_mode(&self) -> Result<crate::command::BlackWhiteMode, Error> {
        let cmd = BlackWhiteModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BlackWhiteMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_usb_audio_enabled(&self) -> Result<bool, Error> {
        let cmd = UsbAudioInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::UsbAudio { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_two_tone_mode_enabled(&self) -> Result<bool, Error> {
        let cmd = TwoToneModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TwoToneMode { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_nd_filter_preset(&self) -> Result<u8, Error> {
        let cmd = NdFilterPresetInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NdFilterPreset { preset }) => Ok(preset),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_digital_mode_enabled(&self) -> Result<bool, Error> {
        let cmd = DigitalInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Digital { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_tally_auto_adjust_enabled(&self) -> Result<bool, Error> {
        let cmd = TallyAutoAdjustInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyAutoAdjust { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_tally_green_enabled(&self) -> Result<bool, Error> {
        let cmd = TallyGreenInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyGreen { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P, T> InquiryControl for crate::camera::BlockingCamera<P, T>
where
    P: crate::capabilities::Profile + Default,
    T: crate::transport::BlockingTransport,
{
    fn get_power_state(&mut self) -> Result<bool, Error> {
        let cmd = PowerInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Power { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_zoom_position(&mut self) -> Result<u16, Error> {
        let cmd = ZoomPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ZoomPosition { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_position(&mut self) -> Result<u16, Error> {
        let cmd = FocusPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusPosition { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_near_limit(&mut self) -> Result<u16, Error> {
        let cmd = FocusNearLimitInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_zone(&mut self) -> Result<FocusZone, Error> {
        let cmd = FocusZoneInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusZone { zone }) => Ok(zone),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_auto_focus_sensitivity(&mut self) -> Result<AutoFocusSensitivity, Error> {
        let cmd = AutoFocusSensitivityInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_mode(&mut self) -> Result<ExposureMode, Error> {
        let cmd = ExposureModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_compensation(&mut self) -> Result<i8, Error> {
        let cmd = ExposureCompensationInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_compensation_enabled(&mut self) -> Result<bool, Error> {
        let cmd = ExposureCompensationModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ExposureCompensationMode { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_iris(&mut self) -> Result<u8, Error> {
        let cmd = IrisInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Iris { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_shutter(&mut self) -> Result<u16, Error> {
        let cmd = ShutterInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Shutter { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gain(&mut self) -> Result<u8, Error> {
        let cmd = GainInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::GainLevel { gain }) => Ok(gain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gain_limit(&mut self) -> Result<u8, Error> {
        let cmd = GainLimitInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::GainLimit { limit }) => Ok(limit),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_white_balance_mode(&mut self) -> Result<WhiteBalanceMode, Error> {
        let cmd = WhiteBalanceModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::WhiteBalanceMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_red_gain(&mut self) -> Result<u8, Error> {
        let cmd = RedGainInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::RedChannel { gain }) => Ok(gain as u8),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_blue_gain(&mut self) -> Result<u8, Error> {
        let cmd = BlueGainInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BlueChannel { gain }) => Ok(gain as u8),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_red_tuning(&mut self) -> Result<u8, Error> {
        let cmd = RedTuningInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::RedTuning { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_blue_tuning(&mut self) -> Result<u8, Error> {
        let cmd = BlueTuningInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BlueTuning { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_color_temperature(&mut self) -> Result<u16, Error> {
        let cmd = ColorTemperatureInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ColorTemperature { temperature }) => {
                Ok(temperature)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gamma(&mut self) -> Result<u8, Error> {
        let cmd = GammaInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Gamma { value }) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_brightness(&mut self) -> Result<u8, Error> {
        let cmd = BrightInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Bright { position }) => Ok(position as u8),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_sharpness_mode(&mut self) -> Result<SharpnessMode, Error> {
        let cmd = SharpnessModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_saturation(&mut self) -> Result<u8, Error> {
        let cmd = SaturationInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Saturation { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_hue(&mut self) -> Result<u8, Error> {
        let cmd = HueInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Hue { hue }) => Ok(hue),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_2d(&mut self) -> Result<u8, Error> {
        let cmd = NoiseReduction2DInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_3d(&mut self) -> Result<u8, Error> {
        let cmd = NoiseReduction3DInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_black_white(&mut self) -> Result<bool, Error> {
        let cmd = BlackWhiteInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BlackWhite { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_resolution(&mut self) -> Result<crate::command::resolution::ResolutionMode, Error> {
        let cmd = ResolutionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Resolution(mode_byte)) => Ok(
                crate::command::resolution::ResolutionMode::from_byte(mode_byte),
            ),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_picture_effect(
        &mut self,
    ) -> Result<crate::command::resolution::PictureEffectMode, Error> {
        let cmd = PictureEffectInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::PictureEffect { effect }) => Ok(
                crate::command::resolution::PictureEffectMode::from_byte(effect),
            ),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_nd_filter_position(
        &mut self,
    ) -> Result<crate::command::resolution::NDFilterPosition, Error> {
        let cmd = NdFilterInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NdFilter { position }) => Ok(
                crate::command::resolution::NDFilterPosition::from_byte(position),
            ),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_version(&mut self) -> Result<crate::command::Version, Error> {
        let cmd = VersionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            }) => Ok(crate::command::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_backlight_enabled(&mut self) -> Result<bool, Error> {
        let cmd = BacklightInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Backlight { status }) => Ok(status),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_image_flip(&mut self) -> Result<crate::command::ImageFlipStatus, Error> {
        let cmd = ImageFlipInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok(crate::command::ImageFlipStatus {
                vertical,
                horizontal,
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_dynamic_range(&mut self) -> Result<u8, Error> {
        let cmd = DynamicRangeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::DynamicRange { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_mode(&mut self) -> Result<FocusMode, Error> {
        let cmd = FocusModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_menu_status(&mut self) -> Result<bool, Error> {
        let cmd = MenuOpenCloseInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::MenuOpenClose { is_open }) => Ok(is_open),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_auto_focus_enabled(&mut self) -> Result<bool, Error> {
        // TODO: AutoFocusInquiry struct is not available in inquiry_structs.rs
        // We can derive this from FocusModeInquiry instead
        let cmd = FocusModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusMode { mode }) => {
                Ok(mode == FocusMode::Auto)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_tally_light_status(&mut self) -> Result<crate::command::TallyStatus, Error> {
        let cmd = TallyStatusInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyStatus { red_on, green_on }) => {
                Ok(crate::command::TallyStatus { red_on, green_on })
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_night_day_mode(&mut self) -> Result<crate::command::NightDayMode, Error> {
        let cmd = NightDayModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NightDayMode { is_night }) => Ok(if is_night {
                crate::command::NightDayMode::Night
            } else {
                crate::command::NightDayMode::Day
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_flip_mode(&mut self) -> Result<crate::command::FlipMode, Error> {
        let cmd = FlipModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FlipMode {
                horizontal,
                vertical,
            }) => Ok(crate::command::FlipMode {
                horizontal,
                vertical,
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_standby_enabled(&mut self) -> Result<bool, Error> {
        let cmd = StandbyInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Standby { in_standby }) => Ok(in_standby),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_range(&mut self) -> Result<crate::command::FocusRange, Error> {
        let cmd = FocusRangeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusRange { range }) => Ok(range),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_iris_control(&mut self) -> Result<crate::command::IrisControl, Error> {
        let cmd = IrisControlInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::IrisControl { auto }) => Ok(if auto {
                crate::command::IrisControl::Auto
            } else {
                crate::command::IrisControl::Manual
            }),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_defog_mode(&mut self) -> Result<bool, Error> {
        let cmd = DefogModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::DefogMode { enabled }) => Ok(enabled),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_defog_level(&mut self) -> Result<u8, Error> {
        let cmd = DefogLevelInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::DefogLevel { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_digital_ptz_enabled(&mut self) -> Result<bool, Error> {
        let cmd = DigitalPtzInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::DigitalPtz { enabled }) => Ok(enabled),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_auto_white_balance_sensitivity(
        &mut self,
    ) -> Result<crate::command::AutoWhiteBalanceSensitivity, Error> {
        let cmd = AutoWhiteBalanceSensitivityInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::AutoWhiteBalanceSensitivity {
                sensitivity,
            }) => Ok(sensitivity),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_compensation_position(&mut self) -> Result<u16, Error> {
        let cmd = ExposureCompensationPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::ExposureCompensationPosition { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_auto_trace_enabled(&mut self) -> Result<bool, Error> {
        let cmd = AutoTraceInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::AutoTrace { enabled }) => Ok(enabled),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_unlock(&mut self) -> Result<bool, Error> {
        let cmd = FocusUnlockInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::FocusUnlock { unlocked }) => Ok(unlocked),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_sharpness_position(&mut self) -> Result<u16, Error> {
        let cmd = SharpnessPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::SharpnessPosition { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_level(&mut self) -> Result<u8, Error> {
        let cmd = NrLevelInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NrLevel(level)) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_broadcast_domain(&mut self) -> Result<u8, Error> {
        let cmd = BroadcastDomainInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BroadcastDomain(domain)) => Ok(domain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_mode(&mut self) -> Result<crate::command::NrMode, Error> {
        let cmd = NrModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NrMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_speed(&mut self) -> Result<crate::command::NrSpeed, Error> {
        let cmd = NrSpeedInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NrSpeed { speed }) => Ok(speed),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_black_white_mode(&mut self) -> Result<crate::command::BlackWhiteMode, Error> {
        let cmd = BlackWhiteModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::BlackWhiteMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_usb_audio_enabled(&mut self) -> Result<bool, Error> {
        let cmd = UsbAudioInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::UsbAudio { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_two_tone_mode_enabled(&mut self) -> Result<bool, Error> {
        let cmd = TwoToneModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TwoToneMode { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_nd_filter_preset(&mut self) -> Result<u8, Error> {
        let cmd = NdFilterPresetInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NdFilterPreset { preset }) => Ok(preset),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_digital_mode_enabled(&mut self) -> Result<bool, Error> {
        let cmd = DigitalInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Digital { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_tally_auto_adjust_enabled(&mut self) -> Result<bool, Error> {
        let cmd = TallyAutoAdjustInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyAutoAdjust { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_tally_green_enabled(&mut self) -> Result<bool, Error> {
        let cmd = TallyGreenInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::TallyGreen { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Async implementation for PanTiltInquiryControl
#[cfg(feature = "async")]
impl<P, Tr, Exec> PanTiltInquiryControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error> {
        let cmd = PanTiltPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation for PanTiltInquiryControl
#[cfg(not(feature = "async"))]
impl<P, T> PanTiltInquiryControl for crate::camera::BlockingCamera<P, T>
where
    P: crate::capabilities::Profile + Default,
    T: crate::transport::BlockingTransport,
{
    fn get_pan_tilt_position(&mut self) -> Result<(i16, i16), Error> {
        let cmd = PanTiltPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
