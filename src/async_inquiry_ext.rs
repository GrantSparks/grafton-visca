//! Async extension trait providing convenience methods for camera state inquiries.

#![allow(deprecated)]

// Crate imports
use crate::{
    async_client::AsyncViscaClient,
    command::{
        exposure::ExposureMode,
        focus::{AFSensitivity, FocusZone},
        gain::AntiFlickerMode,
        inquiry::InquiryCommand,
        luminance_contrast_sharpness::SharpnessMode,
        white_balance::WhiteBalanceMode,
        ViscaInquiryResponse,
    },
    inquiry_ext::{
        CameraPosition, CameraState, ExposureState, ImageState, OpticsState, WhiteBalanceState,
    },
    ViscaError, ViscaResponse,
};

impl AsyncViscaClient {
    /// Get current power state.
    pub async fn get_power_state(&self) -> Result<bool, ViscaError> {
        match self.send(&InquiryCommand::Power).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current pan/tilt position in VISCA units.
    pub async fn get_pan_tilt_position(&self) -> Result<(i16, i16), ViscaError> {
        match self.send(&InquiryCommand::PanTiltPosition).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current zoom position (0x0000-0x4000 for most cameras).
    pub async fn get_zoom_position(&self) -> Result<u16, ViscaError> {
        match self.send(&InquiryCommand::ZoomPosition).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current focus position.
    pub async fn get_focus_position(&self) -> Result<u16, ViscaError> {
        match self.send(&InquiryCommand::FocusPosition).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusPosition { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current exposure mode.
    pub async fn get_exposure_mode(&self) -> Result<ExposureMode, ViscaError> {
        match self.send(&InquiryCommand::ExposureMode).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current white balance mode.
    pub async fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, ViscaError> {
        match self.send(&InquiryCommand::WhiteBalanceMode).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::WhiteBalance { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current luminance level.
    pub async fn get_luminance(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::Luminance).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Luminance(value)) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current contrast level.
    pub async fn get_contrast(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::Contrast).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Contrast(value)) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current sharpness value.
    pub async fn get_sharpness(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::Sharpness).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Sharpness { value }) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current exposure compensation value (-7 to +7).
    pub async fn get_exposure_compensation(&self) -> Result<i8, ViscaError> {
        match self.send(&InquiryCommand::ExposureCompensation).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensation {
                value,
            }) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get whether exposure compensation is enabled.
    pub async fn get_exposure_compensation_enabled(&self) -> Result<bool, ViscaError> {
        match self.send(&InquiryCommand::ExposureCompensationMode).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensationMode {
                on,
            }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current iris position.
    pub async fn get_iris_position(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::Iris).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Iris { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current shutter position.
    pub async fn get_shutter_position(&self) -> Result<u16, ViscaError> {
        match self.send(&InquiryCommand::Shutter).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Shutter { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current brightness position.
    pub async fn get_brightness_position(&self) -> Result<u16, ViscaError> {
        match self.send(&InquiryCommand::Bright).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Bright { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current gain value.
    pub async fn get_gain(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::Gain).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Gain { gain }) => Ok(gain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current gain limit.
    pub async fn get_gain_limit(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::GainLimit).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::GainLimit { limit }) => Ok(limit),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current anti-flicker mode.
    pub async fn get_anti_flicker_mode(&self) -> Result<AntiFlickerMode, ViscaError> {
        match self.send(&InquiryCommand::AntiFlicker).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::AntiFlicker { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current saturation level.
    pub async fn get_saturation(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::Saturation).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Saturation { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current hue value.
    pub async fn get_hue(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::Hue).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Hue { hue }) => Ok(hue),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current red gain value (-10 to +10).
    pub async fn get_red_gain(&self) -> Result<i8, ViscaError> {
        match self.send(&InquiryCommand::RedGain).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::RedGain { gain }) => Ok(gain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current blue gain value (-10 to +10).
    pub async fn get_blue_gain(&self) -> Result<i8, ViscaError> {
        match self.send(&InquiryCommand::BlueGain).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::BlueGain { gain }) => Ok(gain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current backlight compensation status.
    pub async fn get_backlight_status(&self) -> Result<bool, ViscaError> {
        match self.send(&InquiryCommand::Backlight).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Backlight { status }) => {
                Ok(status)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current image flip settings (vertical and horizontal).
    pub async fn get_image_flip(&self) -> Result<(bool, bool), ViscaError> {
        match self.send(&InquiryCommand::ImageFlip).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok((vertical, horizontal)),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current sharpness mode.
    pub async fn get_sharpness_mode(&self) -> Result<SharpnessMode, ViscaError> {
        match self.send(&InquiryCommand::SharpnessMode).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::SharpnessMode { mode }) => {
                Ok(mode)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current color temperature.
    pub async fn get_color_temperature(&self) -> Result<u16, ViscaError> {
        match self.send(&InquiryCommand::ColorTemperature).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ColorTemperature {
                temperature,
            }) => Ok(temperature),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current 2D noise reduction level.
    pub async fn get_noise_reduction_2d(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::NoiseReduction2D).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::NoiseReduction2D { level }) => {
                Ok(level)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current 3D noise reduction level.
    pub async fn get_noise_reduction_3d(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::NoiseReduction3D).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::NoiseReduction3D { level }) => {
                Ok(level)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get whether black & white mode is enabled.
    pub async fn get_black_white_mode(&self) -> Result<bool, ViscaError> {
        match self.send(&InquiryCommand::BlackWhite).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::BlackWhite { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current focus zone.
    pub async fn get_focus_zone(&self) -> Result<FocusZone, ViscaError> {
        match self.send(&InquiryCommand::FocusZone).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusZone { zone }) => Ok(zone),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current AF sensitivity.
    pub async fn get_af_sensitivity(&self) -> Result<AFSensitivity, ViscaError> {
        match self.send(&InquiryCommand::AFSensitivity).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::AFSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current focus near limit position.
    pub async fn get_focus_near_limit(&self) -> Result<u16, ViscaError> {
        match self.send(&InquiryCommand::FocusNearLimit).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusNearLimit { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current dynamic range level.
    pub async fn get_dynamic_range(&self) -> Result<u8, ViscaError> {
        match self.send(&InquiryCommand::DynamicRange).await? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::DynamicRange { level }) => {
                Ok(level)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get complete camera state with multiple queries.
    ///
    /// This method performs multiple inquiry commands to gather comprehensive
    /// camera state information. Note that these are sequential due to VISCA's
    /// command limitations.
    pub async fn get_camera_state(&self) -> Result<CameraState, ViscaError> {
        let power = self.get_power_state().await?;
        let (pan, tilt) = self.get_pan_tilt_position().await?;
        let zoom = self.get_zoom_position().await?;
        let focus = self.get_focus_position().await?;
        let exposure_mode = self.get_exposure_mode().await?;
        let white_balance_mode = self.get_white_balance_mode().await?;

        Ok(CameraState {
            power,
            position: CameraPosition { pan, tilt },
            optics: OpticsState { zoom, focus },
            exposure: ExposureState {
                mode: exposure_mode,
                compensation: if self.get_exposure_compensation_enabled().await? {
                    Some(self.get_exposure_compensation().await?)
                } else {
                    None
                },
            },
            white_balance: WhiteBalanceState {
                mode: white_balance_mode,
            },
            image: ImageState {
                luminance: self.get_luminance().await?,
                contrast: self.get_contrast().await?,
                sharpness: self.get_sharpness().await?,
                saturation: self.get_saturation().await?,
                hue: self.get_hue().await?,
            },
        })
    }
}
