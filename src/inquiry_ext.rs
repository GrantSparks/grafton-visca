//! Extension trait providing convenience methods for camera state inquiries.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{
        exposure::ExposureMode,
        focus::{AFSensitivity, FocusZone},
        gain::AntiFlickerMode,
        inquiry::InquiryCommand,
        luminance_contrast_sharpness::SharpnessMode,
        white_balance::WhiteBalanceMode,
        InquiryResponse,
    },
    error::Error as ViscaError,
    Response, Transport,
};

/// Extension trait providing convenient inquiry methods for camera state.
///
/// This trait is automatically implemented for all types that implement `Transport`,
/// providing a more ergonomic API for querying camera state.
///
/// # Example
/// ```no_run
/// # #[cfg(feature = "blocking-client")]
/// # fn example() -> Result<(), grafton_visca::Error> {
/// # use grafton_visca::{Client, InquiryExt, Error};
/// let mut client = Client::connect_udp("192.168.1.100:5678")?;
///
/// // Simple one-line state queries
/// let (pan, tilt) = client.get_pan_tilt_position()?;
/// let zoom = client.get_zoom_position()?;
/// let is_powered_on = client.get_power_state()?;
/// # Ok(())
/// # }
/// ```
pub trait InquiryExt: Transport {
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
            Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((pan, tilt))
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
                Ok(position)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => {
                Ok(position)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Luminance(value)) => Ok(value),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Contrast(value)) => Ok(value),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
                Ok(value)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => {
                Ok(on)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Gain { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::RedGain { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::BlueGain { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::Backlight { status }) => Ok(status),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok((vertical, horizontal)),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                Ok(temperature)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => {
                Ok(level)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => {
                Ok(level)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => Ok(zone),
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::AFSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => {
                Ok(position)
            }
            Response::Error(e) => Err(e),
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
            Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => Ok(level),
            Response::Error(e) => Err(e),
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

// Blanket implementation for all types that implement Transport
impl<T: Transport> InquiryExt for T {}

/// Complete camera state snapshot.
#[derive(Debug, Clone, Copy)]
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
