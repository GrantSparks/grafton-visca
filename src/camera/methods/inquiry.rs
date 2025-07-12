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

#[cfg(feature = "tokio")]
use core::future::Future;





/// Inquiry operations.
pub trait InquiryOps: Sized {

    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby.
    #[cfg(feature = "tokio")]
    fn get_power_state(&self) -> impl Future<Output = Result<bool, Error>> + Send;


    /// Get the current power state of the camera.
    /// Returns `true` if powered on, `false` if in standby. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_power_state_blocking(&mut self) -> Result<bool, Error>;

    /// Get the current zoom position.
    #[cfg(feature = "tokio")]
    fn get_zoom_position(&self) -> impl Future<Output = Result<u16, Error>> + Send;


    /// Get the current zoom position. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_zoom_position_blocking(&mut self) -> Result<u16, Error>;

    /// Get the current focus position.
    #[cfg(feature = "tokio")]
    fn get_focus_position(&self) -> impl Future<Output = Result<u16, Error>> + Send;


    /// Get the current focus position. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_focus_position_blocking(&mut self) -> Result<u16, Error>;

    /// Get the focus near limit position.
    #[cfg(feature = "tokio")]
    fn get_focus_near_limit(&self) -> impl Future<Output = Result<u16, Error>> + Send;


    /// Get the focus near limit position. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_focus_near_limit_blocking(&mut self) -> Result<u16, Error>;

    /// Get the current focus zone.
    #[cfg(feature = "tokio")]
    fn get_focus_zone(&self) -> impl Future<Output = Result<FocusZone, Error>> + Send;


    /// Get the current focus zone. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_focus_zone_blocking(&mut self) -> Result<FocusZone, Error>;

    /// Get the auto-focus sensitivity setting.
    #[cfg(feature = "tokio")]
    fn get_auto_focus_sensitivity(&self) -> impl Future<Output = Result<AutoFocusSensitivity, Error>> + Send;


    /// Get the auto-focus sensitivity setting. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_auto_focus_sensitivity_blocking(&mut self) -> Result<AutoFocusSensitivity, Error>;

    /// Get the current exposure mode.
    #[cfg(feature = "tokio")]
    fn get_exposure_mode(&self) -> impl Future<Output = Result<ExposureMode, Error>> + Send;


    /// Get the current exposure mode. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_exposure_mode_blocking(&mut self) -> Result<ExposureMode, Error>;

    /// Get the exposure compensation value.
    #[cfg(feature = "tokio")]
    fn get_exposure_compensation(&self) -> impl Future<Output = Result<i8, Error>> + Send;


    /// Get the exposure compensation value. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_exposure_compensation_blocking(&mut self) -> Result<i8, Error>;

    /// Check if exposure compensation is enabled.
    #[cfg(feature = "tokio")]
    fn get_exposure_compensation_enabled(&self) -> impl Future<Output = Result<bool, Error>> + Send;


    /// Check if exposure compensation is enabled. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_exposure_compensation_enabled_blocking(&mut self) -> Result<bool, Error>;

    /// Get the current iris value.
    #[cfg(feature = "tokio")]
    fn get_iris(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the current iris value. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_iris_blocking(&mut self) -> Result<u8, Error>;

    /// Get the current shutter speed.
    #[cfg(feature = "tokio")]
    fn get_shutter(&self) -> impl Future<Output = Result<u16, Error>> + Send;


    /// Get the current shutter speed. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_shutter_blocking(&mut self) -> Result<u16, Error>;

    /// Get the brightness setting.
    #[cfg(feature = "tokio")]
    fn get_brightness(&self) -> impl Future<Output = Result<u16, Error>> + Send;


    /// Get the brightness setting. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_brightness_blocking(&mut self) -> Result<u16, Error>;

    /// Get the current gain value.
    #[cfg(feature = "tokio")]
    fn get_gain(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the current gain value. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_gain_blocking(&mut self) -> Result<u8, Error>;

    /// Get the gain limit setting.
    #[cfg(feature = "tokio")]
    fn get_gain_limit(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the gain limit setting. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_gain_limit_blocking(&mut self) -> Result<u8, Error>;

    /// Get the anti-flicker mode.
    #[cfg(feature = "tokio")]
    fn get_anti_flicker(&self) -> impl Future<Output = Result<AntiFlickerMode, Error>> + Send;


    /// Get the anti-flicker mode. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_anti_flicker_blocking(&mut self) -> Result<AntiFlickerMode, Error>;

    /// Check if backlight compensation is enabled.
    #[cfg(feature = "tokio")]
    fn get_backlight(&self) -> impl Future<Output = Result<bool, Error>> + Send;


    /// Check if backlight compensation is enabled. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_backlight_blocking(&mut self) -> Result<bool, Error>;

    /// Get the dynamic range setting.
    #[cfg(feature = "tokio")]
    fn get_dynamic_range(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the dynamic range setting. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_dynamic_range_blocking(&mut self) -> Result<u8, Error>;

    /// Get the current white balance mode.
    #[cfg(feature = "tokio")]
    fn get_white_balance_mode(&self) -> impl Future<Output = Result<WhiteBalanceMode, Error>> + Send;


    /// Get the current white balance mode. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_white_balance_mode_blocking(&mut self) -> Result<WhiteBalanceMode, Error>;

    /// Get the color temperature value.
    #[cfg(feature = "tokio")]
    fn get_color_temperature(&self) -> impl Future<Output = Result<u16, Error>> + Send;


    /// Get the color temperature value. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_color_temperature_blocking(&mut self) -> Result<u16, Error>;

    /// Get the red gain value.
    #[cfg(feature = "tokio")]
    fn get_red_gain(&self) -> impl Future<Output = Result<i8, Error>> + Send;


    /// Get the red gain value. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_red_gain_blocking(&mut self) -> Result<i8, Error>;

    /// Get the blue gain value.
    #[cfg(feature = "tokio")]
    fn get_blue_gain(&self) -> impl Future<Output = Result<i8, Error>> + Send;


    /// Get the blue gain value. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_blue_gain_blocking(&mut self) -> Result<i8, Error>;

    /// Get the luminance level.
    #[cfg(feature = "tokio")]
    fn get_luminance(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the luminance level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_luminance_blocking(&mut self) -> Result<u8, Error>;

    /// Get the contrast level.
    #[cfg(feature = "tokio")]
    fn get_contrast(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the contrast level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_contrast_blocking(&mut self) -> Result<u8, Error>;

    /// Get the sharpness level.
    #[cfg(feature = "tokio")]
    fn get_sharpness(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the sharpness level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_sharpness_blocking(&mut self) -> Result<u8, Error>;

    /// Get the sharpness mode.
    #[cfg(feature = "tokio")]
    fn get_sharpness_mode(&self) -> impl Future<Output = Result<SharpnessMode, Error>> + Send;


    /// Get the sharpness mode. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_sharpness_mode_blocking(&mut self) -> Result<SharpnessMode, Error>;

    /// Get the saturation level.
    #[cfg(feature = "tokio")]
    fn get_saturation(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the saturation level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_saturation_blocking(&mut self) -> Result<u8, Error>;

    /// Get the hue setting.
    #[cfg(feature = "tokio")]
    fn get_hue(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the hue setting. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_hue_blocking(&mut self) -> Result<u8, Error>;

    /// Get the 2D noise reduction level.
    #[cfg(feature = "tokio")]
    fn get_noise_reduction_2d(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the 2D noise reduction level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_noise_reduction_2d_blocking(&mut self) -> Result<u8, Error>;

    /// Get the 3D noise reduction level.
    #[cfg(feature = "tokio")]
    fn get_noise_reduction_3d(&self) -> impl Future<Output = Result<u8, Error>> + Send;


    /// Get the 3D noise reduction level. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_noise_reduction_3d_blocking(&mut self) -> Result<u8, Error>;

    /// Check if black and white mode is enabled.
    #[cfg(feature = "tokio")]
    fn get_black_white(&self) -> impl Future<Output = Result<bool, Error>> + Send;


    /// Check if black and white mode is enabled. (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_black_white_blocking(&mut self) -> Result<bool, Error>;
}

impl InquiryOps for Camera {
    #[cfg(feature = "tokio")]
    fn get_power_state(&self) -> impl Future<Output = Result<bool, Error>> + Send {
        async move {
            let cmd = PowerInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_power_state_blocking(&mut self) -> Result<bool, Error> {
        
            let cmd = PowerInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_zoom_position(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move {
            let cmd = ZoomPositionInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_zoom_position_blocking(&mut self) -> Result<u16, Error> {
        
            let cmd = ZoomPositionInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_focus_position(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move {
            let cmd = FocusPositionInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_focus_position_blocking(&mut self) -> Result<u16, Error> {
        
            let cmd = FocusPositionInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_focus_near_limit(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move {
            let cmd = FocusNearLimitInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_focus_near_limit_blocking(&mut self) -> Result<u16, Error> {
        
            let cmd = FocusNearLimitInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_focus_zone(&self) -> impl Future<Output = Result<FocusZone, Error>> + Send {
        async move {
            let cmd = FocusZoneInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => Ok(zone),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_focus_zone_blocking(&mut self) -> Result<FocusZone, Error> {
        
            let cmd = FocusZoneInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => Ok(zone),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_auto_focus_sensitivity(&self) -> impl Future<Output = Result<AutoFocusSensitivity, Error>> + Send {
        async move {
            let cmd = AutoFocusSensitivityInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity {
                    sensitivity,
                }) => Ok(sensitivity),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_auto_focus_sensitivity_blocking(&mut self) -> Result<AutoFocusSensitivity, Error> {
        
            let cmd = AutoFocusSensitivityInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity {
                    sensitivity,
                }) => Ok(sensitivity),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_exposure_mode(&self) -> impl Future<Output = Result<ExposureMode, Error>> + Send {
        async move {
            let cmd = ExposureModeInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_exposure_mode_blocking(&mut self) -> Result<ExposureMode, Error> {
        
            let cmd = ExposureModeInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_exposure_compensation(&self) -> impl Future<Output = Result<i8, Error>> + Send {
        async move {
            let cmd = ExposureCompensationInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
                    Ok(value)
                }
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_exposure_compensation_blocking(&mut self) -> Result<i8, Error> {
        
            let cmd = ExposureCompensationInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
                    Ok(value)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_exposure_compensation_enabled(&self) -> impl Future<Output = Result<bool, Error>> + Send {
        async move {
            let cmd = ExposureCompensationModeInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => {
                    Ok(on)
                }
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_exposure_compensation_enabled_blocking(&mut self) -> Result<bool, Error> {
        
            let cmd = ExposureCompensationModeInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => {
                    Ok(on)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_iris(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = IrisInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_iris_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = IrisInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_shutter(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move {
            let cmd = ShutterInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_shutter_blocking(&mut self) -> Result<u16, Error> {
        
            let cmd = ShutterInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_brightness(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move {
            let cmd = BrightInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_brightness_blocking(&mut self) -> Result<u16, Error> {
        
            let cmd = BrightInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_gain(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = GainInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::GainLevel { gain }) => Ok(gain),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_gain_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = GainInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::GainLevel { gain }) => Ok(gain),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_gain_limit(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = GainLimitInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_gain_limit_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = GainLimitInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_anti_flicker(&self) -> impl Future<Output = Result<AntiFlickerMode, Error>> + Send {
        async move {
            let cmd = AntiFlickerInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_anti_flicker_blocking(&mut self) -> Result<AntiFlickerMode, Error> {
        
            let cmd = AntiFlickerInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_backlight(&self) -> impl Future<Output = Result<bool, Error>> + Send {
        async move {
            let cmd = BacklightInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Backlight { status }) => Ok(status),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_backlight_blocking(&mut self) -> Result<bool, Error> {
        
            let cmd = BacklightInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Backlight { status }) => Ok(status),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_dynamic_range(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = DynamicRangeInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => Ok(level),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_dynamic_range_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = DynamicRangeInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_white_balance_mode(&self) -> impl Future<Output = Result<WhiteBalanceMode, Error>> + Send {
        async move {
            let cmd = WhiteBalanceModeInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_white_balance_mode_blocking(&mut self) -> Result<WhiteBalanceMode, Error> {
        
            let cmd = WhiteBalanceModeInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_color_temperature(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move {
            let cmd = ColorTemperatureInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                    Ok(temperature)
                }
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_color_temperature_blocking(&mut self) -> Result<u16, Error> {
        
            let cmd = ColorTemperatureInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                    Ok(temperature)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_red_gain(&self) -> impl Future<Output = Result<i8, Error>> + Send {
        async move {
            let cmd = RedGainInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::RedChannel { gain }) => Ok(gain),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_red_gain_blocking(&mut self) -> Result<i8, Error> {
        
            let cmd = RedGainInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::RedChannel { gain }) => Ok(gain),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_blue_gain(&self) -> impl Future<Output = Result<i8, Error>> + Send {
        async move {
            let cmd = BlueGainInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::BlueChannel { gain }) => Ok(gain),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_blue_gain_blocking(&mut self) -> Result<i8, Error> {
        
            let cmd = BlueGainInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::BlueChannel { gain }) => Ok(gain),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_luminance(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = LuminanceInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Luminance(level)) => Ok(level),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_luminance_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = LuminanceInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Luminance(level)) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_contrast(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = ContrastInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Contrast(level)) => Ok(level),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_contrast_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = ContrastInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Contrast(level)) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_sharpness(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = SharpnessInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_sharpness_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = SharpnessInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_sharpness_mode(&self) -> impl Future<Output = Result<SharpnessMode, Error>> + Send {
        async move {
            let cmd = SharpnessModeInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_sharpness_mode_blocking(&mut self) -> Result<SharpnessMode, Error> {
        
            let cmd = SharpnessModeInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_saturation(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = SaturationInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_saturation_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = SaturationInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_hue(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = HueInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_hue_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = HueInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_noise_reduction_2d(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = NoiseReduction2DInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_noise_reduction_2d_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = NoiseReduction2DInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_noise_reduction_3d(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move {
            let cmd = NoiseReduction3DInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_noise_reduction_3d_blocking(&mut self) -> Result<u8, Error> {
        
            let cmd = NoiseReduction3DInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
    #[cfg(feature = "tokio")]
    fn get_black_white(&self) -> impl Future<Output = Result<bool, Error>> + Send {
        async move {
            let cmd = BlackWhiteInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }

    }

    #[cfg(not(feature = "tokio"))]
    fn get_black_white_blocking(&mut self) -> Result<bool, Error> {
        
            let cmd = BlackWhiteInquiry;
            let response = self.send_command_blocking(&cmd)?;
            match response {
                Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
    }
}





/// Pan/Tilt inquiry operations.
pub trait PanTiltInquiryOps: Sized
{
    /// Query the current pan and tilt position in degrees.
    #[cfg(feature = "tokio")]
    fn get_position_degrees(
        &self,
    ) -> impl Future<Output = Result<(Degrees, Degrees), Error>> + Send;
    
    /// Query the current pan and tilt position in degrees (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_position_degrees_blocking(&mut self) -> Result<(Degrees, Degrees), Error>;
}

impl PanTiltInquiryOps for Camera
{
    #[cfg(feature = "tokio")]
    fn get_position_degrees(
        &self,
    ) -> impl Future<Output = Result<(Degrees, Degrees), Error>> + Send {
        async move {
            let cmd = PanTiltPositionInquiry;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                    let (pan_deg, tilt_deg) = self.units_to_degrees(pan, tilt);
                    Ok((pan_deg, tilt_deg))
                }
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
    
    #[cfg(not(feature = "tokio"))]
    fn get_position_degrees_blocking(&mut self) -> Result<(Degrees, Degrees), Error> {
        let cmd = PanTiltPositionInquiry;
        let response = self.send_command_blocking(&cmd)?;
        match response {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                let (pan_deg, tilt_deg) = self.units_to_degrees(pan, tilt);
                Ok((pan_deg, tilt_deg))
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
