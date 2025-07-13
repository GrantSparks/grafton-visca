//! Inquiry methods for querying camera state using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    command::{
        inquiry::*, AntiFlickerMode, AutoFocusSensitivity, ExposureMode, FocusZone,
        InquiryResponse, Response, SharpnessMode, WhiteBalanceMode,
    },
    units::Degrees,
    Error,
};

/// Inquiry operations (async).
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

    /// Get the anti-flicker mode.
    async fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error>;

    /// Get the gamma level.
    async fn get_gamma(&self) -> Result<u8, Error>;

    /// Get the contrast level.
    async fn get_contrast(&self) -> Result<u8, Error>;

    /// Get the brightness level.
    async fn get_brightness(&self) -> Result<u8, Error>;

    /// Get the sharpness level.
    async fn get_sharpness(&self) -> Result<u8, Error>;

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
}

/// Inquiry operations (blocking).
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

    /// Get the anti-flicker mode.
    fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error>;

    /// Get the gamma level.
    fn get_gamma(&self) -> Result<u8, Error>;

    /// Get the contrast level.
    fn get_contrast(&self) -> Result<u8, Error>;

    /// Get the brightness level.
    fn get_brightness(&self) -> Result<u8, Error>;

    /// Get the sharpness level.
    fn get_sharpness(&self) -> Result<u8, Error>;

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
}

// Async implementation
impl InquiryOps for Camera {
    async fn get_power_state(&self) -> Result<bool, Error> {
        let cmd = PowerInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_zoom_position(&self) -> Result<u16, Error> {
        let cmd = ZoomPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_position(&self) -> Result<u16, Error> {
        let cmd = FocusPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_near_limit(&self) -> Result<u16, Error> {
        let cmd = FocusNearLimitInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_focus_zone(&self) -> Result<FocusZone, Error> {
        // TODO: AFZoneInquiry struct needs to be defined
        // let cmd = AFZoneInquiry;
        // let response = self.send_command(&cmd).await?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::AFZone { zone }) => Ok(zone),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    async fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error> {
        // TODO: AFSensitivityInquiry struct needs to be defined
        // let cmd = AFSensitivityInquiry;
        // let response = self.send_command(&cmd).await?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::AFSensitivity { sensitivity }) => {
        //         Ok(sensitivity)
        //     }
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    async fn get_exposure_mode(&self) -> Result<ExposureMode, Error> {
        let cmd = ExposureModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_compensation(&self) -> Result<i8, Error> {
        let cmd = ExposureCompensationInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        // TODO: ExposureCompensationEnabledInquiry struct needs to be defined
        // let cmd = ExposureCompensationEnabledInquiry;
        // let response = self.send_command(&cmd).await?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::ExposureCompensationEnabled { enabled }) => {
        //         Ok(enabled)
        //     }
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    async fn get_iris(&self) -> Result<u8, Error> {
        let cmd = IrisInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_shutter(&self) -> Result<u16, Error> {
        let cmd = ShutterInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gain(&self) -> Result<u8, Error> {
        let cmd = GainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::GainLevel { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gain_limit(&self) -> Result<u8, Error> {
        let cmd = GainLimitInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error> {
        let cmd = WhiteBalanceModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_red_gain(&self) -> Result<u8, Error> {
        let cmd = RedGainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::RedChannel { gain }) => Ok(gain as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_blue_gain(&self) -> Result<u8, Error> {
        let cmd = BlueGainInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::BlueChannel { gain }) => Ok(gain as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_red_tuning(&self) -> Result<u8, Error> {
        // TODO: RedTuningInquiry struct needs to be defined
        // let cmd = RedTuningInquiry;
        // let response = self.send_command(&cmd).await?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::RedTuning { value }) => Ok(value),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    async fn get_blue_tuning(&self) -> Result<u8, Error> {
        // TODO: BlueTuningInquiry struct needs to be defined
        // let cmd = BlueTuningInquiry;
        // let response = self.send_command(&cmd).await?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::BlueTuning { value }) => Ok(value),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    async fn get_color_temperature(&self) -> Result<u16, Error> {
        let cmd = ColorTemperatureInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => Ok(temperature),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error> {
        let cmd = AntiFlickerInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_gamma(&self) -> Result<u8, Error> {
        // TODO: GammaInquiry struct needs to be defined
        // let cmd = GammaInquiry;
        // let response = self.send_command(&cmd).await?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::Gamma { value }) => Ok(value),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    async fn get_contrast(&self) -> Result<u8, Error> {
        let cmd = ContrastInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::Contrast(value)) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_brightness(&self) -> Result<u8, Error> {
        // TODO: BrightnessInquiry struct needs to be defined
        // let cmd = BrightnessInquiry;
        // let response = self.send_command(&cmd).await?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::Brightness { value }) => Ok(value),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    async fn get_sharpness(&self) -> Result<u8, Error> {
        let cmd = SharpnessInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error> {
        let cmd = SharpnessModeInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_saturation(&self) -> Result<u8, Error> {
        let cmd = SaturationInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_hue(&self) -> Result<u8, Error> {
        let cmd = HueInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_2d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction2DInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_noise_reduction_3d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction3DInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_black_white(&self) -> Result<bool, Error> {
        let cmd = BlackWhiteInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
impl InquiryOpsBlocking for Camera {
    fn get_power_state(&self) -> Result<bool, Error> {
        let cmd = PowerInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_zoom_position(&self) -> Result<u16, Error> {
        let cmd = ZoomPositionInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_position(&self) -> Result<u16, Error> {
        let cmd = FocusPositionInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_near_limit(&self) -> Result<u16, Error> {
        let cmd = FocusNearLimitInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_focus_zone(&self) -> Result<FocusZone, Error> {
        // TODO: AFZoneInquiry struct needs to be defined
        // let cmd = AFZoneInquiry;
        // let response = self.send_command_blocking(&cmd)?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::AFZone { zone }) => Ok(zone),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error> {
        // TODO: AFSensitivityInquiry struct needs to be defined
        // let cmd = AFSensitivityInquiry;
        // let response = self.send_command_blocking(&cmd)?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::AFSensitivity { sensitivity }) => {
        //         Ok(sensitivity)
        //     }
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    fn get_exposure_mode(&self) -> Result<ExposureMode, Error> {
        let cmd = ExposureModeInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_compensation(&self) -> Result<i8, Error> {
        let cmd = ExposureCompensationInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        // TODO: ExposureCompensationEnabledInquiry struct needs to be defined
        // let cmd = ExposureCompensationEnabledInquiry;
        // let response = self.send_command_blocking(&cmd)?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::ExposureCompensationEnabled { enabled }) => {
        //         Ok(enabled)
        //     }
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    fn get_iris(&self) -> Result<u8, Error> {
        let cmd = IrisInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_shutter(&self) -> Result<u16, Error> {
        let cmd = ShutterInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gain(&self) -> Result<u8, Error> {
        let cmd = GainInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::GainLevel { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gain_limit(&self) -> Result<u8, Error> {
        let cmd = GainLimitInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error> {
        let cmd = WhiteBalanceModeInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_red_gain(&self) -> Result<u8, Error> {
        let cmd = RedGainInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::RedChannel { gain }) => Ok(gain as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_blue_gain(&self) -> Result<u8, Error> {
        let cmd = BlueGainInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::BlueChannel { gain }) => Ok(gain as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_red_tuning(&self) -> Result<u8, Error> {
        // TODO: RedTuningInquiry struct needs to be defined
        // let cmd = RedTuningInquiry;
        // let response = self.send_command_blocking(&cmd)?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::RedTuning { value }) => Ok(value),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    fn get_blue_tuning(&self) -> Result<u8, Error> {
        // TODO: BlueTuningInquiry struct needs to be defined
        // let cmd = BlueTuningInquiry;
        // let response = self.send_command_blocking(&cmd)?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::BlueTuning { value }) => Ok(value),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    fn get_color_temperature(&self) -> Result<u16, Error> {
        let cmd = ColorTemperatureInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => Ok(temperature),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error> {
        let cmd = AntiFlickerInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_gamma(&self) -> Result<u8, Error> {
        // TODO: GammaInquiry struct needs to be defined
        // let cmd = GammaInquiry;
        // let response = self.send_command_blocking(&cmd)?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::Gamma { value }) => Ok(value),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    fn get_contrast(&self) -> Result<u8, Error> {
        let cmd = ContrastInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::Contrast(value)) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_brightness(&self) -> Result<u8, Error> {
        // TODO: BrightnessInquiry struct needs to be defined
        // let cmd = BrightnessInquiry;
        // let response = self.send_command_blocking(&cmd)?;
        // match response {
        //     Response::InquiryResponse(InquiryResponse::Brightness { value }) => Ok(value),
        //     Response::Error(e) => Err(e),
        //     _ => Err(Error::UnexpectedResponseType),
        // }
        Err(Error::UnexpectedResponseType)
    }

    fn get_sharpness(&self) -> Result<u8, Error> {
        let cmd = SharpnessInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error> {
        let cmd = SharpnessModeInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_saturation(&self) -> Result<u8, Error> {
        let cmd = SaturationInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_hue(&self) -> Result<u8, Error> {
        let cmd = HueInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_2d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction2DInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_noise_reduction_3d(&self) -> Result<u8, Error> {
        let cmd = NoiseReduction3DInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_black_white(&self) -> Result<bool, Error> {
        let cmd = BlackWhiteInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

/// Pan/Tilt-specific inquiry operations (async).
pub trait PanTiltInquiryOps: Sized {
    /// Get the current pan and tilt position.
    async fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error>;

    /// Get the current pan and tilt position in degrees.
    async fn get_pan_tilt_degrees(&self) -> Result<(Degrees, Degrees), Error>;
}

/// Pan/Tilt-specific inquiry operations (blocking).
pub trait PanTiltInquiryOpsBlocking: Sized {
    /// Get the current pan and tilt position.
    fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error>;

    /// Get the current pan and tilt position in degrees.
    fn get_pan_tilt_degrees(&self) -> Result<(Degrees, Degrees), Error>;
}

// Async implementation
impl PanTiltInquiryOps for Camera {
    async fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error> {
        let cmd = PanTiltPositionInquiry;
        let response = self.send_command(&cmd).await?;
        match response {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_pan_tilt_degrees(&self) -> Result<(Degrees, Degrees), Error> {
        let (pan_units, tilt_units) = PanTiltInquiryOps::get_pan_tilt_position(self).await?;
        let (pan_deg, tilt_deg) = self.units_to_degrees(pan_units, tilt_units);
        Ok((pan_deg, tilt_deg))
    }
}

// Blocking implementation
impl PanTiltInquiryOpsBlocking for Camera {
    fn get_pan_tilt_position(&self) -> Result<(i16, i16), Error> {
        let cmd = PanTiltPositionInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_pan_tilt_degrees(&self) -> Result<(Degrees, Degrees), Error> {
        let (pan_units, tilt_units) = PanTiltInquiryOpsBlocking::get_pan_tilt_position(self)?;
        let (pan_deg, tilt_deg) = self.units_to_degrees(pan_units, tilt_units);
        Ok((pan_deg, tilt_deg))
    }
}