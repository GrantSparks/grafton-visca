//! Extension trait providing convenience methods for camera state inquiries.

use crate::{
    command::{
        exposure::ExposureMode,
        focus::{AFSensitivity, FocusZone},
        gain::AntiFlickerMode,
        inquiry::InquiryCommand,
        luminance_contrast_sharpness::SharpnessMode,
        white_balance::WhiteBalanceMode,
        ViscaInquiryResponse,
    },
    ViscaDevice, ViscaError, ViscaResponse,
};

/// Extension trait providing convenient inquiry methods for camera state.
///
/// This trait is automatically implemented for all types that implement `ViscaDevice`,
/// providing a more ergonomic API for querying camera state.
///
/// # Example
/// ```no_run
/// # #[cfg(feature = "blocking-client")]
/// # fn example() -> Result<(), grafton_visca::ViscaError> {
/// # use grafton_visca::{ViscaClient, ViscaInquiryExt, ViscaError};
/// let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
///
/// // Simple one-line state queries
/// let (pan, tilt) = client.get_pan_tilt_position()?;
/// let zoom = client.get_zoom_position()?;
/// let is_powered_on = client.get_power_state()?;
/// # Ok(())
/// # }
/// ```
pub trait ViscaInquiryExt: ViscaDevice {
    /// Get current power state.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_power_state(&mut self) -> Result<bool, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Power)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Power { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current pan/tilt position in VISCA units.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_pan_tilt_position(&mut self) -> Result<(i16, i16), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::PanTiltPosition)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current zoom position (0x0000-0x4000 for most cameras).
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_zoom_position(&mut self) -> Result<u16, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::ZoomPosition)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ZoomPosition { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current focus position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_focus_position(&mut self) -> Result<u16, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::FocusPosition)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusPosition { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current exposure mode.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_exposure_mode(&mut self) -> Result<ExposureMode, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::ExposureMode)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureMode { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current white balance mode.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_white_balance_mode(&mut self) -> Result<WhiteBalanceMode, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::WhiteBalanceMode)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::WhiteBalance { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current luminance level.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_luminance(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Luminance)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Luminance(value)) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current contrast level.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_contrast(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Contrast)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Contrast(value)) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current sharpness value.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_sharpness(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Sharpness)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Sharpness { value }) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current exposure compensation value (-7 to +7).
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_exposure_compensation(&mut self) -> Result<i8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::ExposureCompensation)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensation {
                value,
            }) => Ok(value),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get whether exposure compensation is enabled.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_exposure_compensation_enabled(&mut self) -> Result<bool, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::ExposureCompensationMode)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ExposureCompensationMode {
                on,
            }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current iris position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_iris_position(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Iris)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Iris { position }) => Ok(position),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current shutter position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_shutter_position(&mut self) -> Result<u16, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Shutter)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Shutter { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current brightness position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_brightness_position(&mut self) -> Result<u16, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Bright)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Bright { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current gain value.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_gain(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Gain)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Gain { gain }) => Ok(gain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current gain limit.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_gain_limit(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::GainLimit)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::GainLimit { limit }) => Ok(limit),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current anti-flicker mode.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_anti_flicker_mode(&mut self) -> Result<AntiFlickerMode, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::AntiFlicker)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::AntiFlicker { mode }) => Ok(mode),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current saturation level.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_saturation(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Saturation)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Saturation { level }) => Ok(level),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current hue value.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_hue(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Hue)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Hue { hue }) => Ok(hue),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current red gain value (-10 to +10).
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_red_gain(&mut self) -> Result<i8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::RedGain)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::RedGain { gain }) => Ok(gain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current blue gain value (-10 to +10).
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_blue_gain(&mut self) -> Result<i8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::BlueGain)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::BlueGain { gain }) => Ok(gain),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current backlight compensation status.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_backlight_status(&mut self) -> Result<bool, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::Backlight)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::Backlight { status }) => {
                Ok(status)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current image flip settings (vertical and horizontal).
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_image_flip(&mut self) -> Result<(bool, bool), ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::ImageFlip)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok((vertical, horizontal)),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current sharpness mode.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_sharpness_mode(&mut self) -> Result<SharpnessMode, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::SharpnessMode)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::SharpnessMode { mode }) => {
                Ok(mode)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current color temperature.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_color_temperature(&mut self) -> Result<u16, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::ColorTemperature)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::ColorTemperature {
                temperature,
            }) => Ok(temperature),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current 2D noise reduction level.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_noise_reduction_2d(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::NoiseReduction2D)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::NoiseReduction2D { level }) => {
                Ok(level)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current 3D noise reduction level.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_noise_reduction_3d(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::NoiseReduction3D)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::NoiseReduction3D { level }) => {
                Ok(level)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get whether black & white mode is enabled.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_black_white_mode(&mut self) -> Result<bool, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::BlackWhite)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::BlackWhite { on }) => Ok(on),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current focus zone.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_focus_zone(&mut self) -> Result<FocusZone, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::FocusZone)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusZone { zone }) => Ok(zone),
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current AF sensitivity.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_af_sensitivity(&mut self) -> Result<AFSensitivity, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::AFSensitivity)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::AFSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current focus near limit position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_focus_near_limit(&mut self) -> Result<u16, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::FocusNearLimit)? {
            ViscaResponse::InquiryResponse(ViscaInquiryResponse::FocusNearLimit { position }) => {
                Ok(position)
            }
            ViscaResponse::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Get current dynamic range level.
    ///
    /// # Errors
    /// Returns `ViscaError` if the query command fails, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_dynamic_range(&mut self) -> Result<u8, ViscaError>
    where
        Self: Sized,
    {
        match self.execute_command(&InquiryCommand::DynamicRange)? {
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
    ///
    /// # Errors
    /// Returns `ViscaError` if any of the multiple query commands fail, response parsing fails,
    /// communication times out, or the camera returns an unexpected response type.
    fn get_camera_state(&mut self) -> Result<CameraState, ViscaError>
    where
        Self: Sized,
    {
        let power = self.get_power_state()?;
        let (pan, tilt) = self.get_pan_tilt_position()?;
        let zoom = self.get_zoom_position()?;
        let focus = self.get_focus_position()?;
        let exposure_mode = self.get_exposure_mode()?;
        let white_balance_mode = self.get_white_balance_mode()?;

        Ok(CameraState {
            power,
            position: CameraPosition { pan, tilt },
            optics: OpticsState { zoom, focus },
            exposure: ExposureState {
                mode: exposure_mode,
                compensation: if self.get_exposure_compensation_enabled()? {
                    Some(self.get_exposure_compensation()?)
                } else {
                    None
                },
            },
            white_balance: WhiteBalanceState {
                mode: white_balance_mode,
            },
            image: ImageState {
                luminance: self.get_luminance()?,
                contrast: self.get_contrast()?,
                sharpness: self.get_sharpness()?,
                saturation: self.get_saturation()?,
                hue: self.get_hue()?,
            },
        })
    }
}

// Blanket implementation for all types that implement ViscaTransport
impl<T: ViscaDevice + ?Sized> ViscaInquiryExt for T {}

/// Complete camera state snapshot.
#[derive(Debug, Clone)]
pub struct CameraState {
    /// Power state (on/off)
    pub power: bool,
    /// Pan/tilt position
    pub position: CameraPosition,
    /// Optical settings (zoom, focus)
    pub optics: OpticsState,
    /// Exposure settings
    pub exposure: ExposureState,
    /// White balance settings
    pub white_balance: WhiteBalanceState,
    /// Image quality settings
    pub image: ImageState,
}

/// Camera position information.
#[derive(Debug, Copy, Clone)]
pub struct CameraPosition {
    /// Pan position in VISCA units
    pub pan: i16,
    /// Tilt position in VISCA units
    pub tilt: i16,
}

/// Optical settings state.
#[derive(Debug, Copy, Clone)]
pub struct OpticsState {
    /// Zoom position (0x0000-0x4000 for most cameras)
    pub zoom: u16,
    /// Focus position
    pub focus: u16,
}

/// Exposure settings state.
#[derive(Debug, Copy, Clone)]
pub struct ExposureState {
    /// Exposure mode
    pub mode: ExposureMode,
    /// Exposure compensation value (-7 to +7) if enabled
    pub compensation: Option<i8>,
}

/// White balance settings state.
#[derive(Debug, Copy, Clone)]
pub struct WhiteBalanceState {
    /// White balance mode
    pub mode: WhiteBalanceMode,
}

/// Image quality settings state.
#[derive(Debug, Copy, Clone)]
pub struct ImageState {
    /// Luminance level
    pub luminance: u8,
    /// Contrast level
    pub contrast: u8,
    /// Sharpness value
    pub sharpness: u8,
    /// Saturation level
    pub saturation: u8,
    /// Hue value
    pub hue: u8,
}
