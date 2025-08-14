//! Inquiry methods for querying camera state using the new GAT architecture.

use crate::{
    command::{
        inquiry::*, response::Response, AutoFocusSensitivity, ExposureMode, FocusMode, FocusZone,
        InquiryResponse, SharpnessMode, WhiteBalanceMode,
    },
    Error,
};

/// Inquiry operations (async).
#[cfg(feature = "async")]
pub trait InquiryOps: Sized {
    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    async fn get_power_state(&self) -> Result<bool, Error>;

    /// Get the current zoom position.
    async fn get_zoom_position(&self) -> Result<u16, Error>;

    /// Get the current focus position.
    async fn get_focus_position(&self) -> Result<u16, Error>;

    /// Get the focus near limit position.
    async fn get_focus_near_limit(&self) -> Result<u16, Error>;

    /// Get the current focus zone.
    async fn get_focus_zone(&self) -> Result<FocusZone, Error>;

    /// Get the auto-focus sensitivity setting.
    async fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error>;

    /// Get the current exposure mode.
    async fn get_exposure_mode(&self) -> Result<ExposureMode, Error>;

    /// Get the exposure compensation value.
    async fn get_exposure_compensation(&self) -> Result<i8, Error>;

    /// Check if exposure compensation is enabled.
    async fn get_exposure_compensation_enabled(&self) -> Result<bool, Error>;

    /// Get the current iris value.
    async fn get_iris(&self) -> Result<u8, Error>;

    /// Get the current shutter speed.
    async fn get_shutter(&self) -> Result<u16, Error>;

    /// Get the current gain value.
    async fn get_gain(&self) -> Result<u8, Error>;

    /// Get the gain limit value.
    async fn get_gain_limit(&self) -> Result<u8, Error>;

    /// Get the white balance mode.
    async fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error>;

    /// Get the current red gain.
    async fn get_red_gain(&self) -> Result<u8, Error>;

    /// Get the current blue gain.
    async fn get_blue_gain(&self) -> Result<u8, Error>;

    /// Get the red tuning value.
    async fn get_red_tuning(&self) -> Result<u8, Error>;

    /// Get the blue tuning value.
    async fn get_blue_tuning(&self) -> Result<u8, Error>;

    /// Get the current color temperature in Kelvin.
    async fn get_color_temperature(&self) -> Result<u16, Error>;

    /// Get the gamma level.
    async fn get_gamma(&self) -> Result<u8, Error>;

    /// Get the brightness level.
    async fn get_brightness(&self) -> Result<u8, Error>;

    /// Get the sharpness mode.
    async fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error>;

    /// Get the saturation level.
    async fn get_saturation(&self) -> Result<u8, Error>;

    /// Get the hue setting.
    async fn get_hue(&self) -> Result<u8, Error>;

    /// Get the 2D noise reduction level.
    async fn get_noise_reduction_2d(&self) -> Result<u8, Error>;

    /// Get the 3D noise reduction level.
    async fn get_noise_reduction_3d(&self) -> Result<u8, Error>;

    /// Check if black and white mode is enabled.
    async fn get_black_white(&self) -> Result<bool, Error>;

    /// Get the current video resolution mode.
    async fn get_resolution(&self) -> Result<crate::command::resolution::ResolutionMode, Error>;

    /// Get the current picture effect mode.
    async fn get_picture_effect(
        &self,
    ) -> Result<crate::command::resolution::PictureEffectMode, Error>;

    /// Get the current ND filter position (Sony FR7 only).
    async fn get_nd_filter_position(
        &self,
    ) -> Result<crate::command::resolution::NDFilterPosition, Error>;

    /// Get the camera version information.
    async fn get_version(&self) -> Result<crate::command::Version, Error>;

    /// Check if backlight compensation is enabled.
    async fn get_backlight_enabled(&self) -> Result<bool, Error>;

    /// Get the image flip settings (mirror/reverse).
    async fn get_image_flip(&self) -> Result<crate::command::ImageFlipStatus, Error>;

    /// Get the dynamic range mode/level.
    async fn get_dynamic_range(&self) -> Result<u8, Error>;

    /// Get the current focus mode (Auto/Manual).
    async fn get_focus_mode(&self) -> Result<FocusMode, Error>;

    /// Get the menu open/close status.
    async fn get_menu_status(&self) -> Result<bool, Error>;

    /// Get the auto focus on/off status.
    async fn get_auto_focus_enabled(&self) -> Result<bool, Error>;

    /// Get the tally light status (red and green).
    async fn get_tally_light_status(&self) -> Result<crate::command::TallyStatus, Error>;

    /// Get the night/day mode status.
    async fn get_night_day_mode(&self) -> Result<crate::command::NightDayMode, Error>;

    /// Get the current flip mode (combined horizontal/vertical).
    async fn get_flip_mode(&self) -> Result<crate::command::FlipMode, Error>;

    /// Get the standby mode status.
    async fn get_standby_enabled(&self) -> Result<bool, Error>;

    /// Get the focus range setting.
    async fn get_focus_range(&self) -> Result<crate::command::FocusRange, Error>;

    /// Get the iris control mode.
    async fn get_iris_control(&self) -> Result<crate::command::IrisControl, Error>;

    /// Get the defog mode status.
    async fn get_defog_mode(&self) -> Result<bool, Error>;

    /// Get the defog level.
    async fn get_defog_level(&self) -> Result<u8, Error>;

    /// Get the digital PTZ mode status.
    async fn get_digital_ptz_enabled(&self) -> Result<bool, Error>;

    /// Get the auto white balance sensitivity setting.
    async fn get_auto_white_balance_sensitivity(
        &self,
    ) -> Result<crate::command::AutoWhiteBalanceSensitivity, Error>;

    /// Get the exposure compensation position.
    async fn get_exposure_compensation_position(&self) -> Result<u16, Error>;

    /// Get the auto trace mode status.
    async fn get_auto_trace_enabled(&self) -> Result<bool, Error>;

    /// Get the focus unlock state.
    async fn get_focus_unlock(&self) -> Result<bool, Error>;

    /// Get the current sharpness position.
    async fn get_sharpness_position(&self) -> Result<u16, Error>;

    /// Get the noise reduction level.
    async fn get_noise_reduction_level(&self) -> Result<u8, Error>;

    /// Get the broadcast domain setting.
    async fn get_broadcast_domain(&self) -> Result<u8, Error>;

    /// Get the noise reduction mode setting.
    async fn get_noise_reduction_mode(&self) -> Result<crate::command::NrMode, Error>;

    /// Get the noise reduction speed setting.
    async fn get_noise_reduction_speed(&self) -> Result<crate::command::NrSpeed, Error>;

    /// Get the black and white mode setting.
    async fn get_black_white_mode(&self) -> Result<crate::command::BlackWhiteMode, Error>;

    /// Get the USB audio state.
    async fn get_usb_audio_enabled(&self) -> Result<bool, Error>;

    /// Get the two tone mode state.
    async fn get_two_tone_mode_enabled(&self) -> Result<bool, Error>;

    /// Get the ND filter preset setting.
    async fn get_nd_filter_preset(&self) -> Result<u8, Error>;

    /// Get the digital mode state.
    async fn get_digital_mode_enabled(&self) -> Result<bool, Error>;

    /// Get the tally auto adjust state.
    async fn get_tally_auto_adjust_enabled(&self) -> Result<bool, Error>;

    /// Get the green tally light status (FR7 only).
    async fn get_tally_green_enabled(&self) -> Result<bool, Error>;
}

/// Inquiry operations (blocking).
#[cfg(not(feature = "async"))]
pub trait InquiryOpsBlocking: Sized {
    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    fn get_power_state(&self) -> Result<bool, Error>;

    /// Get the current zoom position.
    fn get_zoom_position(&self) -> Result<u16, Error>;

    /// Get the current focus position.
    fn get_focus_position(&self) -> Result<u16, Error>;

    /// Get the focus near limit position.
    fn get_focus_near_limit(&self) -> Result<u16, Error>;

    /// Get the current focus zone.
    fn get_focus_zone(&self) -> Result<FocusZone, Error>;

    /// Get the auto-focus sensitivity setting.
    fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error>;

    /// Get the current exposure mode.
    fn get_exposure_mode(&self) -> Result<ExposureMode, Error>;

    /// Get the exposure compensation value.
    fn get_exposure_compensation(&self) -> Result<i8, Error>;

    /// Check if exposure compensation is enabled.
    fn get_exposure_compensation_enabled(&self) -> Result<bool, Error>;

    /// Get the current iris value.
    fn get_iris(&self) -> Result<u8, Error>;

    /// Get the current shutter speed.
    fn get_shutter(&self) -> Result<u16, Error>;

    /// Get the current gain value.
    fn get_gain(&self) -> Result<u8, Error>;

    /// Get the gain limit value.
    fn get_gain_limit(&self) -> Result<u8, Error>;

    /// Get the white balance mode.
    fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error>;

    /// Get the current red gain.
    fn get_red_gain(&self) -> Result<u8, Error>;

    /// Get the current blue gain.
    fn get_blue_gain(&self) -> Result<u8, Error>;

    /// Get the red tuning value.
    fn get_red_tuning(&self) -> Result<u8, Error>;

    /// Get the blue tuning value.
    fn get_blue_tuning(&self) -> Result<u8, Error>;

    /// Get the current color temperature in Kelvin.
    fn get_color_temperature(&self) -> Result<u16, Error>;

    /// Get the gamma level.
    fn get_gamma(&self) -> Result<u8, Error>;

    /// Get the brightness level.
    fn get_brightness(&self) -> Result<u8, Error>;

    /// Get the sharpness mode.
    fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error>;

    /// Get the saturation level.
    fn get_saturation(&self) -> Result<u8, Error>;

    /// Get the hue setting.
    fn get_hue(&self) -> Result<u8, Error>;

    /// Get the 2D noise reduction level.
    fn get_noise_reduction_2d(&self) -> Result<u8, Error>;

    /// Get the 3D noise reduction level.
    fn get_noise_reduction_3d(&self) -> Result<u8, Error>;

    /// Check if black and white mode is enabled.
    fn get_black_white(&self) -> Result<bool, Error>;

    /// Get the current video resolution mode.
    fn get_resolution(&self) -> Result<crate::command::resolution::ResolutionMode, Error>;

    /// Get the current picture effect mode.
    fn get_picture_effect(&self) -> Result<crate::command::resolution::PictureEffectMode, Error>;

    /// Get the current ND filter position (Sony FR7 only).
    fn get_nd_filter_position(&self)
        -> Result<crate::command::resolution::NDFilterPosition, Error>;

    /// Get the camera version information.
    fn get_version(&self) -> Result<crate::command::Version, Error>;

    /// Check if backlight compensation is enabled.
    fn get_backlight_enabled(&self) -> Result<bool, Error>;

    /// Get the image flip settings (mirror/reverse).
    fn get_image_flip(&self) -> Result<crate::command::ImageFlipStatus, Error>;

    /// Get the dynamic range mode/level.
    fn get_dynamic_range(&self) -> Result<u8, Error>;

    /// Get the current focus mode (Auto/Manual).
    fn get_focus_mode(&self) -> Result<FocusMode, Error>;

    /// Get the menu open/close status.
    fn get_menu_status(&self) -> Result<bool, Error>;

    /// Get the auto focus on/off status.
    fn get_auto_focus_enabled(&self) -> Result<bool, Error>;

    /// Get the tally light status (red and green).
    fn get_tally_light_status(&self) -> Result<crate::command::TallyStatus, Error>;

    /// Get the night/day mode status.
    fn get_night_day_mode(&self) -> Result<crate::command::NightDayMode, Error>;

    /// Get the current flip mode (combined horizontal/vertical).
    fn get_flip_mode(&self) -> Result<crate::command::FlipMode, Error>;

    /// Get the standby mode status.
    fn get_standby_enabled(&self) -> Result<bool, Error>;

    /// Get the focus range setting.
    fn get_focus_range(&self) -> Result<crate::command::FocusRange, Error>;

    /// Get the iris control mode.
    fn get_iris_control(&self) -> Result<crate::command::IrisControl, Error>;

    /// Get the defog mode status.
    fn get_defog_mode(&self) -> Result<bool, Error>;

    /// Get the defog level.
    fn get_defog_level(&self) -> Result<u8, Error>;

    /// Get the digital PTZ mode status.
    fn get_digital_ptz_enabled(&self) -> Result<bool, Error>;

    /// Get the auto white balance sensitivity setting.
    fn get_auto_white_balance_sensitivity(
        &self,
    ) -> Result<crate::command::AutoWhiteBalanceSensitivity, Error>;

    /// Get the exposure compensation position.
    fn get_exposure_compensation_position(&self) -> Result<u16, Error>;

    /// Get the auto trace mode status.
    fn get_auto_trace_enabled(&self) -> Result<bool, Error>;

    /// Get the focus unlock state.
    fn get_focus_unlock(&self) -> Result<bool, Error>;

    /// Get the current sharpness position.
    fn get_sharpness_position(&self) -> Result<u16, Error>;

    /// Get the noise reduction level.
    fn get_noise_reduction_level(&self) -> Result<u8, Error>;

    /// Get the broadcast domain setting.
    fn get_broadcast_domain(&self) -> Result<u8, Error>;

    /// Get the noise reduction mode setting.
    fn get_noise_reduction_mode(&self) -> Result<crate::command::NrMode, Error>;

    /// Get the noise reduction speed setting.
    fn get_noise_reduction_speed(&self) -> Result<crate::command::NrSpeed, Error>;

    /// Get the black and white mode setting.
    fn get_black_white_mode(&self) -> Result<crate::command::BlackWhiteMode, Error>;

    /// Get the USB audio state.
    fn get_usb_audio_enabled(&self) -> Result<bool, Error>;

    /// Get the two tone mode state.
    fn get_two_tone_mode_enabled(&self) -> Result<bool, Error>;

    /// Get the ND filter preset setting.
    fn get_nd_filter_preset(&self) -> Result<u8, Error>;

    /// Get the digital mode state.
    fn get_digital_mode_enabled(&self) -> Result<bool, Error>;

    /// Get the tally auto adjust state.
    fn get_tally_auto_adjust_enabled(&self) -> Result<bool, Error>;

    /// Get the green tally light status (FR7 only).
    fn get_tally_green_enabled(&self) -> Result<bool, Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> InquiryOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn get_power_state(&self) -> Result<bool, Error> {
        let cmd = PowerInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Power { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_zoom_position(&self) -> Result<u16, Error> {
        let cmd = ZoomPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::ZoomPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_position(&self) -> Result<u16, Error> {
        let cmd = FocusPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::FocusPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_near_limit(&self) -> Result<u16, Error> {
        let cmd = FocusNearLimitInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_zone(&self) -> Result<FocusZone, Error> {
        let cmd = FocusZoneInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::FocusZone { zone }) => Ok(zone),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error> {
        let cmd = AutoFocusSensitivityInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_mode(&self) -> Result<ExposureMode, Error> {
        let cmd = ExposureModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_compensation(&self) -> Result<i8, Error> {
        let cmd = ExposureCompensationInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        let cmd = ExposureCompensationModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::ExposureCompensationMode { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_iris(&self) -> Result<u8, Error> {
        let cmd = IrisInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Iris { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_shutter(&self) -> Result<u16, Error> {
        let cmd = ShutterInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Shutter { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gain(&self) -> Result<u8, Error> {
        let cmd = GainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::GainLevel { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gain_limit(&self) -> Result<u8, Error> {
        let cmd = GainLimitInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::GainLimit { limit }) => Ok(limit),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error> {
        let cmd = WhiteBalanceModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::WhiteBalanceMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_red_gain(&self) -> Result<u8, Error> {
        let cmd = RedGainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::RedChannel { gain }) => Ok(gain as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_blue_gain(&self) -> Result<u8, Error> {
        let cmd = BlueGainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::BlueChannel { gain }) => Ok(gain as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_red_tuning(&self) -> Result<u8, Error> {
        let cmd = RedTuningInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::RedTuning { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_blue_tuning(&self) -> Result<u8, Error> {
        let cmd = BlueTuningInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::BlueTuning { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_color_temperature(&self) -> Result<u16, Error> {
        let cmd = ColorTemperatureInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::ColorTemperature { temperature }) => Ok(temperature),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gamma(&self) -> Result<u8, Error> {
        let cmd = GammaInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Gamma { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_brightness(&self) -> Result<u8, Error> {
        let cmd = BrightInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Bright { position }) => Ok(position as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error> {
        let cmd = SharpnessModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_saturation(&self) -> Result<u8, Error> {
        let cmd = SaturationInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Saturation { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_hue(&self) -> Result<u8, Error> {
        let cmd = HueInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Hue { hue }) => Ok(hue),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_2d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction2DInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_3d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction3DInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_black_white(&self) -> Result<bool, Error> {
        let cmd = BlackWhiteInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::BlackWhite { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_resolution(&self) -> Result<crate::command::resolution::ResolutionMode, Error> {
        let cmd = ResolutionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Resolution(mode_byte)) => Ok(
                crate::command::resolution::ResolutionMode::from_byte(mode_byte),
            ),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_picture_effect(
        &self,
    ) -> Result<crate::command::resolution::PictureEffectMode, Error> {
        let cmd = PictureEffectInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::PictureEffect { effect }) => Ok(
                crate::command::resolution::PictureEffectMode::from_byte(effect),
            ),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_nd_filter_position(
        &self,
    ) -> Result<crate::command::resolution::NDFilterPosition, Error> {
        let cmd = NdFilterInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::NdFilter { position }) => Ok(
                crate::command::resolution::NDFilterPosition::from_byte(position),
            ),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_version(&self) -> Result<crate::command::Version, Error> {
        let cmd = VersionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Version {
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
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_backlight_enabled(&self) -> Result<bool, Error> {
        let cmd = BacklightInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Backlight { status }) => Ok(status),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_image_flip(&self) -> Result<crate::command::ImageFlipStatus, Error> {
        let cmd = ImageFlipInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok(crate::command::ImageFlipStatus {
                vertical,
                horizontal,
            }),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_dynamic_range(&self) -> Result<u8, Error> {
        let cmd = DynamicRangeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::DynamicRange { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_mode(&self) -> Result<FocusMode, Error> {
        let cmd = FocusModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::FocusMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_menu_status(&self) -> Result<bool, Error> {
        let cmd = MenuOpenCloseInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::MenuOpenClose { is_open }) => Ok(is_open),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_auto_focus_enabled(&self) -> Result<bool, Error> {
        // TODO: AutoFocusInquiry struct is not available in inquiry_structs.rs
        // We can derive this from FocusModeInquiry instead
        let cmd = FocusModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::FocusMode { mode }) => Ok(mode == FocusMode::Auto),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_tally_light_status(&self) -> Result<crate::command::TallyStatus, Error> {
        let cmd = TallyStatusInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::TallyStatus { red_on, green_on }) => {
                Ok(crate::command::TallyStatus { red_on, green_on })
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_night_day_mode(&self) -> Result<crate::command::NightDayMode, Error> {
        let cmd = NightDayModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::NightDayMode { is_night }) => Ok(if is_night {
                crate::command::NightDayMode::Night
            } else {
                crate::command::NightDayMode::Day
            }),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_flip_mode(&self) -> Result<crate::command::FlipMode, Error> {
        let cmd = FlipModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::FlipMode {
                horizontal,
                vertical,
            }) => Ok(crate::command::FlipMode {
                horizontal,
                vertical,
            }),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_standby_enabled(&self) -> Result<bool, Error> {
        let cmd = StandbyInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Standby { in_standby }) => Ok(in_standby),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_range(&self) -> Result<crate::command::FocusRange, Error> {
        let cmd = FocusRangeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::FocusRange { range }) => Ok(range),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_iris_control(&self) -> Result<crate::command::IrisControl, Error> {
        let cmd = IrisControlInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::IrisControl { auto }) => Ok(if auto {
                crate::command::IrisControl::Auto
            } else {
                crate::command::IrisControl::Manual
            }),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_defog_mode(&self) -> Result<bool, Error> {
        let cmd = DefogModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::DefogMode { enabled }) => Ok(enabled),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_defog_level(&self) -> Result<u8, Error> {
        let cmd = DefogLevelInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::DefogLevel { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_digital_ptz_enabled(&self) -> Result<bool, Error> {
        let cmd = DigitalPtzInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::DigitalPtz { enabled }) => Ok(enabled),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_auto_white_balance_sensitivity(
        &self,
    ) -> Result<crate::command::AutoWhiteBalanceSensitivity, Error> {
        let cmd = AutoWhiteBalanceSensitivityInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::AutoWhiteBalanceSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_compensation_position(&self) -> Result<u16, Error> {
        let cmd = ExposureCompensationPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::ExposureCompensationPosition { position }) => {
                Ok(position)
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_auto_trace_enabled(&self) -> Result<bool, Error> {
        let cmd = AutoTraceInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::AutoTrace { enabled }) => Ok(enabled),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_unlock(&self) -> Result<bool, Error> {
        let cmd = FocusUnlockInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::FocusUnlock { unlocked }) => Ok(unlocked),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_sharpness_position(&self) -> Result<u16, Error> {
        let cmd = SharpnessPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::SharpnessPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_level(&self) -> Result<u8, Error> {
        let cmd = NrLevelInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::NrLevel(level)) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_broadcast_domain(&self) -> Result<u8, Error> {
        let cmd = BroadcastDomainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::BroadcastDomain(domain)) => Ok(domain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_mode(&self) -> Result<crate::command::NrMode, Error> {
        let cmd = NrModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::NrMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_speed(&self) -> Result<crate::command::NrSpeed, Error> {
        let cmd = NrSpeedInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::NrSpeed { speed }) => Ok(speed),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_black_white_mode(&self) -> Result<crate::command::BlackWhiteMode, Error> {
        let cmd = BlackWhiteModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::BlackWhiteMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_usb_audio_enabled(&self) -> Result<bool, Error> {
        let cmd = UsbAudioInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::UsbAudio { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_two_tone_mode_enabled(&self) -> Result<bool, Error> {
        let cmd = TwoToneModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::TwoToneMode { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_nd_filter_preset(&self) -> Result<u8, Error> {
        let cmd = NdFilterPresetInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::NdFilterPreset { preset }) => Ok(preset),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_digital_mode_enabled(&self) -> Result<bool, Error> {
        let cmd = DigitalInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::Digital { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_tally_auto_adjust_enabled(&self) -> Result<bool, Error> {
        let cmd = TallyAutoAdjustInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::TallyAutoAdjust { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_tally_green_enabled(&self) -> Result<bool, Error> {
        let cmd = TallyGreenInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::TallyGreen { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P, T> InquiryOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport,
{
    fn get_power_state(&self) -> Result<bool, Error> {
        let cmd = PowerInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Power { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_zoom_position(&self) -> Result<u16, Error> {
        let cmd = ZoomPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::ZoomPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_position(&self) -> Result<u16, Error> {
        let cmd = FocusPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::FocusPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_near_limit(&self) -> Result<u16, Error> {
        let cmd = FocusNearLimitInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_zone(&self) -> Result<FocusZone, Error> {
        let cmd = FocusZoneInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::FocusZone { zone }) => Ok(zone),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error> {
        let cmd = AutoFocusSensitivityInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_mode(&self) -> Result<ExposureMode, Error> {
        let cmd = ExposureModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_compensation(&self) -> Result<i8, Error> {
        let cmd = ExposureCompensationInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        let cmd = ExposureCompensationModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::ExposureCompensationMode { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_iris(&self) -> Result<u8, Error> {
        let cmd = IrisInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Iris { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_shutter(&self) -> Result<u16, Error> {
        let cmd = ShutterInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Shutter { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gain(&self) -> Result<u8, Error> {
        let cmd = GainInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::GainLevel { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gain_limit(&self) -> Result<u8, Error> {
        let cmd = GainLimitInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::GainLimit { limit }) => Ok(limit),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error> {
        let cmd = WhiteBalanceModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::WhiteBalanceMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_red_gain(&self) -> Result<u8, Error> {
        let cmd = RedGainInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::RedChannel { gain }) => Ok(gain as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_blue_gain(&self) -> Result<u8, Error> {
        let cmd = BlueGainInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::BlueChannel { gain }) => Ok(gain as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_red_tuning(&self) -> Result<u8, Error> {
        let cmd = RedTuningInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::RedTuning { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_blue_tuning(&self) -> Result<u8, Error> {
        let cmd = BlueTuningInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::BlueTuning { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_color_temperature(&self) -> Result<u16, Error> {
        let cmd = ColorTemperatureInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::ColorTemperature { temperature }) => Ok(temperature),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gamma(&self) -> Result<u8, Error> {
        let cmd = GammaInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Gamma { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_brightness(&self) -> Result<u8, Error> {
        let cmd = BrightInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Bright { position }) => Ok(position as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error> {
        let cmd = SharpnessModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_saturation(&self) -> Result<u8, Error> {
        let cmd = SaturationInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Saturation { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_hue(&self) -> Result<u8, Error> {
        let cmd = HueInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Hue { hue }) => Ok(hue),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_2d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction2DInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_3d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction3DInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_black_white(&self) -> Result<bool, Error> {
        let cmd = BlackWhiteInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::BlackWhite { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_resolution(&self) -> Result<crate::command::resolution::ResolutionMode, Error> {
        let cmd = ResolutionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Resolution(mode_byte)) => Ok(
                crate::command::resolution::ResolutionMode::from_byte(mode_byte),
            ),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_picture_effect(&self) -> Result<crate::command::resolution::PictureEffectMode, Error> {
        let cmd = PictureEffectInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::PictureEffect { effect }) => Ok(
                crate::command::resolution::PictureEffectMode::from_byte(effect),
            ),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_nd_filter_position(
        &self,
    ) -> Result<crate::command::resolution::NDFilterPosition, Error> {
        let cmd = NdFilterInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::NdFilter { position }) => Ok(
                crate::command::resolution::NDFilterPosition::from_byte(position),
            ),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_version(&self) -> Result<crate::command::Version, Error> {
        let cmd = VersionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Version {
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
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_backlight_enabled(&self) -> Result<bool, Error> {
        let cmd = BacklightInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Backlight { status }) => Ok(status),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_image_flip(&self) -> Result<crate::command::ImageFlipStatus, Error> {
        let cmd = ImageFlipInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok(crate::command::ImageFlipStatus {
                vertical,
                horizontal,
            }),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_dynamic_range(&self) -> Result<u8, Error> {
        let cmd = DynamicRangeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::DynamicRange { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_mode(&self) -> Result<FocusMode, Error> {
        let cmd = FocusModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::FocusMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_menu_status(&self) -> Result<bool, Error> {
        let cmd = MenuOpenCloseInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::MenuOpenClose { is_open }) => Ok(is_open),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_auto_focus_enabled(&self) -> Result<bool, Error> {
        // TODO: AutoFocusInquiry struct is not available in inquiry_structs.rs
        // We can derive this from FocusModeInquiry instead
        let cmd = FocusModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::FocusMode { mode }) => Ok(mode == FocusMode::Auto),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_tally_light_status(&self) -> Result<crate::command::TallyStatus, Error> {
        let cmd = TallyStatusInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::TallyStatus { red_on, green_on }) => {
                Ok(crate::command::TallyStatus { red_on, green_on })
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_night_day_mode(&self) -> Result<crate::command::NightDayMode, Error> {
        let cmd = NightDayModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::NightDayMode { is_night }) => Ok(if is_night {
                crate::command::NightDayMode::Night
            } else {
                crate::command::NightDayMode::Day
            }),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_flip_mode(&self) -> Result<crate::command::FlipMode, Error> {
        let cmd = FlipModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::FlipMode {
                horizontal,
                vertical,
            }) => Ok(crate::command::FlipMode {
                horizontal,
                vertical,
            }),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_standby_enabled(&self) -> Result<bool, Error> {
        let cmd = StandbyInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Standby { in_standby }) => Ok(in_standby),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_range(&self) -> Result<crate::command::FocusRange, Error> {
        let cmd = FocusRangeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::FocusRange { range }) => Ok(range),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_iris_control(&self) -> Result<crate::command::IrisControl, Error> {
        let cmd = IrisControlInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::IrisControl { auto }) => Ok(if auto {
                crate::command::IrisControl::Auto
            } else {
                crate::command::IrisControl::Manual
            }),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_defog_mode(&self) -> Result<bool, Error> {
        let cmd = DefogModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::DefogMode { enabled }) => Ok(enabled),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_defog_level(&self) -> Result<u8, Error> {
        let cmd = DefogLevelInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::DefogLevel { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_digital_ptz_enabled(&self) -> Result<bool, Error> {
        let cmd = DigitalPtzInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::DigitalPtz { enabled }) => Ok(enabled),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_auto_white_balance_sensitivity(
        &self,
    ) -> Result<crate::command::AutoWhiteBalanceSensitivity, Error> {
        let cmd = AutoWhiteBalanceSensitivityInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::AutoWhiteBalanceSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_compensation_position(&self) -> Result<u16, Error> {
        let cmd = ExposureCompensationPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::ExposureCompensationPosition { position }) => {
                Ok(position)
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_auto_trace_enabled(&self) -> Result<bool, Error> {
        let cmd = AutoTraceInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::AutoTrace { enabled }) => Ok(enabled),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_unlock(&self) -> Result<bool, Error> {
        let cmd = FocusUnlockInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::FocusUnlock { unlocked }) => Ok(unlocked),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_sharpness_position(&self) -> Result<u16, Error> {
        let cmd = SharpnessPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::SharpnessPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_level(&self) -> Result<u8, Error> {
        let cmd = NrLevelInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::NrLevel(level)) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_broadcast_domain(&self) -> Result<u8, Error> {
        let cmd = BroadcastDomainInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::BroadcastDomain(domain)) => Ok(domain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_mode(&self) -> Result<crate::command::NrMode, Error> {
        let cmd = NrModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::NrMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_speed(&self) -> Result<crate::command::NrSpeed, Error> {
        let cmd = NrSpeedInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::NrSpeed { speed }) => Ok(speed),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_black_white_mode(&self) -> Result<crate::command::BlackWhiteMode, Error> {
        let cmd = BlackWhiteModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::BlackWhiteMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_usb_audio_enabled(&self) -> Result<bool, Error> {
        let cmd = UsbAudioInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::UsbAudio { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_two_tone_mode_enabled(&self) -> Result<bool, Error> {
        let cmd = TwoToneModeInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::TwoToneMode { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_nd_filter_preset(&self) -> Result<u8, Error> {
        let cmd = NdFilterPresetInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::NdFilterPreset { preset }) => Ok(preset),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_digital_mode_enabled(&self) -> Result<bool, Error> {
        let cmd = DigitalInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::Digital { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_tally_auto_adjust_enabled(&self) -> Result<bool, Error> {
        let cmd = TallyAutoAdjustInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::TallyAutoAdjust { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_tally_green_enabled(&self) -> Result<bool, Error> {
        let cmd = TallyGreenInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::TallyGreen { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

/// Pan/Tilt-specific inquiry operations (async).
#[cfg(feature = "async")]
pub trait PanTiltInquiryOps: Sized {
    /// Get the current pan and tilt position.
    async fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error>;
}

/// Pan/Tilt-specific inquiry operations (blocking).
#[cfg(not(feature = "async"))]
pub trait PanTiltInquiryOpsBlocking: Sized {
    /// Get the current pan and tilt position.
    fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T, E> PanTiltInquiryOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error> {
        let cmd = PanTiltPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => Ok((pan, tilt)),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P, T> PanTiltInquiryOpsBlocking
    for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport,
{
    fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error> {
        let cmd = PanTiltPositionInquiry;
        let response = self.send_command(&cmd)?;
        match response {
            Response::Inquiry(InquiryResponse::PanTiltPosition { pan, tilt }) => Ok((pan, tilt)),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
