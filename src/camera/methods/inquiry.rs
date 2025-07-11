//! Inquiry methods for querying camera state.

use crate::camera::Camera;
use crate::capabilities::{PanTilt, ProfileMetadata};
use crate::command::inquiry::InquiryCommand;
use crate::command::{
    AntiFlickerMode, AutoFocusSensitivity, Command, ExposureMode, FocusZone, InquiryResponse,
    Response, SharpnessMode, WhiteBalanceMode,
};
use crate::dual_native_inquiry;
use crate::units::{Degrees, ViscaUnits};
use crate::Error;

/// Extension trait that adds inquiry methods to cameras.
#[allow(async_fn_in_trait)]
pub trait InquiryMethodsExt {
    // Power and Basic State

    /// Query the camera's power state.
    #[cfg(not(feature = "async"))]
    fn get_power_state(&mut self) -> Result<bool, Error>;

    /// Query the camera's power state.
    #[cfg(feature = "async")]
    async fn get_power_state(&self) -> Result<bool, Error>;

    // Position Queries - return VISCA units for all cameras

    /// Query the current pan and tilt position in VISCA units.
    #[cfg(not(feature = "async"))]
    fn get_position(&mut self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error>;

    /// Query the current pan and tilt position in VISCA units.
    #[cfg(feature = "async")]
    async fn get_position(&self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error>;

    // Optics Queries

    /// Query the current zoom position.
    #[cfg(not(feature = "async"))]
    fn get_zoom_position(&mut self) -> Result<u16, Error>;

    /// Query the current zoom position.
    #[cfg(feature = "async")]
    async fn get_zoom_position(&self) -> Result<u16, Error>;

    /// Query the current focus position.
    #[cfg(not(feature = "async"))]
    fn get_focus_position(&mut self) -> Result<u16, Error>;

    /// Query the current focus position.
    #[cfg(feature = "async")]
    async fn get_focus_position(&self) -> Result<u16, Error>;

    /// Query the focus near limit position.
    #[cfg(not(feature = "async"))]
    fn get_focus_near_limit(&mut self) -> Result<u16, Error>;

    /// Query the focus near limit position.
    #[cfg(feature = "async")]
    async fn get_focus_near_limit(&self) -> Result<u16, Error>;

    /// Query the current focus zone.
    #[cfg(not(feature = "async"))]
    fn get_focus_zone(&mut self) -> Result<FocusZone, Error>;

    /// Query the current focus zone.
    #[cfg(feature = "async")]
    async fn get_focus_zone(&self) -> Result<FocusZone, Error>;

    /// Query the auto focus sensitivity.
    #[cfg(not(feature = "async"))]
    fn get_auto_focus_sensitivity(&mut self) -> Result<AutoFocusSensitivity, Error>;

    /// Query the auto focus sensitivity.
    #[cfg(feature = "async")]
    async fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error>;

    // Exposure Queries

    /// Query the current exposure mode.
    #[cfg(not(feature = "async"))]
    fn get_exposure_mode(&mut self) -> Result<ExposureMode, Error>;

    /// Query the current exposure mode.
    #[cfg(feature = "async")]
    async fn get_exposure_mode(&self) -> Result<ExposureMode, Error>;

    /// Query the exposure compensation value.
    #[cfg(not(feature = "async"))]
    fn get_exposure_compensation(&mut self) -> Result<i8, Error>;

    /// Query the exposure compensation value.
    #[cfg(feature = "async")]
    async fn get_exposure_compensation(&self) -> Result<i8, Error>;

    /// Query whether exposure compensation is enabled.
    #[cfg(not(feature = "async"))]
    fn get_exposure_compensation_enabled(&mut self) -> Result<bool, Error>;

    /// Query whether exposure compensation is enabled.
    #[cfg(feature = "async")]
    async fn get_exposure_compensation_enabled(&self) -> Result<bool, Error>;

    /// Query the current iris position.
    #[cfg(not(feature = "async"))]
    fn get_iris(&mut self) -> Result<u8, Error>;

    /// Query the current iris position.
    #[cfg(feature = "async")]
    async fn get_iris(&self) -> Result<u8, Error>;

    /// Query the current shutter position.
    #[cfg(not(feature = "async"))]
    fn get_shutter(&mut self) -> Result<u16, Error>;

    /// Query the current shutter position.
    #[cfg(feature = "async")]
    async fn get_shutter(&self) -> Result<u16, Error>;

    /// Query the current brightness level.
    #[cfg(not(feature = "async"))]
    fn get_brightness(&mut self) -> Result<u16, Error>;

    /// Query the current brightness level.
    #[cfg(feature = "async")]
    async fn get_brightness(&self) -> Result<u16, Error>;

    /// Query the current gain level.
    #[cfg(not(feature = "async"))]
    fn get_gain(&mut self) -> Result<u8, Error>;

    /// Query the current gain level.
    #[cfg(feature = "async")]
    async fn get_gain(&self) -> Result<u8, Error>;

    /// Query the gain limit.
    #[cfg(not(feature = "async"))]
    fn get_gain_limit(&mut self) -> Result<u8, Error>;

    /// Query the gain limit.
    #[cfg(feature = "async")]
    async fn get_gain_limit(&self) -> Result<u8, Error>;

    /// Query the anti-flicker mode.
    #[cfg(not(feature = "async"))]
    fn get_anti_flicker(&mut self) -> Result<AntiFlickerMode, Error>;

    /// Query the anti-flicker mode.
    #[cfg(feature = "async")]
    async fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error>;

    /// Query the backlight compensation status.
    #[cfg(not(feature = "async"))]
    fn get_backlight(&mut self) -> Result<bool, Error>;

    /// Query the backlight compensation status.
    #[cfg(feature = "async")]
    async fn get_backlight(&self) -> Result<bool, Error>;

    /// Query the dynamic range level.
    #[cfg(not(feature = "async"))]
    fn get_dynamic_range(&mut self) -> Result<u8, Error>;

    /// Query the dynamic range level.
    #[cfg(feature = "async")]
    async fn get_dynamic_range(&self) -> Result<u8, Error>;

    // Color Queries

    /// Query the current white balance mode.
    #[cfg(not(feature = "async"))]
    fn get_white_balance_mode(&mut self) -> Result<WhiteBalanceMode, Error>;

    /// Query the current white balance mode.
    #[cfg(feature = "async")]
    async fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error>;

    /// Query the color temperature (only valid in manual white balance mode).
    #[cfg(not(feature = "async"))]
    fn get_color_temperature(&mut self) -> Result<u16, Error>;

    /// Query the color temperature (only valid in manual white balance mode).
    #[cfg(feature = "async")]
    async fn get_color_temperature(&self) -> Result<u16, Error>;

    /// Query the red gain value.
    #[cfg(not(feature = "async"))]
    fn get_red_gain(&mut self) -> Result<i8, Error>;

    /// Query the red gain value.
    #[cfg(feature = "async")]
    async fn get_red_gain(&self) -> Result<i8, Error>;

    /// Query the blue gain value.
    #[cfg(not(feature = "async"))]
    fn get_blue_gain(&mut self) -> Result<i8, Error>;

    /// Query the blue gain value.
    #[cfg(feature = "async")]
    async fn get_blue_gain(&self) -> Result<i8, Error>;

    // Image Quality Queries

    /// Query the luminance level.
    #[cfg(not(feature = "async"))]
    fn get_luminance(&mut self) -> Result<u8, Error>;

    /// Query the luminance level.
    #[cfg(feature = "async")]
    async fn get_luminance(&self) -> Result<u8, Error>;

    /// Query the contrast level.
    #[cfg(not(feature = "async"))]
    fn get_contrast(&mut self) -> Result<u8, Error>;

    /// Query the contrast level.
    #[cfg(feature = "async")]
    async fn get_contrast(&self) -> Result<u8, Error>;

    /// Query the sharpness value.
    #[cfg(not(feature = "async"))]
    fn get_sharpness(&mut self) -> Result<u8, Error>;

    /// Query the sharpness value.
    #[cfg(feature = "async")]
    async fn get_sharpness(&self) -> Result<u8, Error>;

    /// Query the sharpness mode.
    #[cfg(not(feature = "async"))]
    fn get_sharpness_mode(&mut self) -> Result<SharpnessMode, Error>;

    /// Query the sharpness mode.
    #[cfg(feature = "async")]
    async fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error>;

    /// Query the saturation level.
    #[cfg(not(feature = "async"))]
    fn get_saturation(&mut self) -> Result<u8, Error>;

    /// Query the saturation level.
    #[cfg(feature = "async")]
    async fn get_saturation(&self) -> Result<u8, Error>;

    /// Query the hue value.
    #[cfg(not(feature = "async"))]
    fn get_hue(&mut self) -> Result<u8, Error>;

    /// Query the hue value.
    #[cfg(feature = "async")]
    async fn get_hue(&self) -> Result<u8, Error>;

    /// Query the 2D noise reduction level.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_2d(&mut self) -> Result<u8, Error>;

    /// Query the 2D noise reduction level.
    #[cfg(feature = "async")]
    async fn get_noise_reduction_2d(&self) -> Result<u8, Error>;

    /// Query the 3D noise reduction level.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_3d(&mut self) -> Result<u8, Error>;

    /// Query the 3D noise reduction level.
    #[cfg(feature = "async")]
    async fn get_noise_reduction_3d(&self) -> Result<u8, Error>;

    // Special Effects Queries

    /// Query the current image flip state.
    #[cfg(not(feature = "async"))]
    fn get_image_flip(&mut self) -> Result<(bool, bool), Error>;

    /// Query the current image flip state.
    #[cfg(feature = "async")]
    async fn get_image_flip(&self) -> Result<(bool, bool), Error>;

    /// Query whether black and white mode is enabled.
    #[cfg(not(feature = "async"))]
    fn get_black_white(&mut self) -> Result<bool, Error>;

    /// Query whether black and white mode is enabled.
    #[cfg(feature = "async")]
    async fn get_black_white(&self) -> Result<bool, Error>;

    // Camera Information

    /// Query the camera's version information.
    #[cfg(not(feature = "async"))]
    fn get_version(&mut self) -> Result<(u16, u16, u32, u8), Error>;

    /// Query the camera's version information.
    #[cfg(feature = "async")]
    async fn get_version(&self) -> Result<(u16, u16, u32, u8), Error>;
}

/// Extension trait that adds pan/tilt-specific inquiry methods to cameras with PanTilt capability.
/// This trait provides convenience methods for cameras that support pan/tilt operations,
/// allowing position queries to be returned in degrees.
#[allow(async_fn_in_trait)]
pub trait PanTiltInquiryMethodsExt {
    /// Query the current pan and tilt position in degrees.
    /// This method is only available for cameras that implement the PanTilt capability.
    #[cfg(not(feature = "async"))]
    fn get_position_degrees(&mut self) -> Result<(Degrees, Degrees), Error>;

    /// Query the current pan and tilt position in degrees.
    /// This method is only available for cameras that implement the PanTilt capability.
    #[cfg(feature = "async")]
    async fn get_position_degrees(&self) -> Result<(Degrees, Degrees), Error>;
}

// Implementation for all cameras - uses the dual_native_inquiry macro
#[cfg(not(feature = "async"))]
impl<P, T> InquiryMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::blocking::BlockingTransport,
{
    #[dual_native_inquiry]
    fn get_power_state(&mut self) -> Result<bool, Error> {
        let cmd = InquiryCommand::Power;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_position(&mut self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error> {
        let cmd = InquiryCommand::PanTiltPosition;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((ViscaUnits(pan), ViscaUnits(tilt)))
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_zoom_position(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::ZoomPosition;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_focus_position(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::FocusPosition;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_focus_near_limit(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::FocusNearLimit;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_focus_zone(&mut self) -> Result<FocusZone, Error> {
        let cmd = InquiryCommand::FocusZone;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => Ok(zone),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_auto_focus_sensitivity(&mut self) -> Result<AutoFocusSensitivity, Error> {
        let cmd = InquiryCommand::AutoFocusSensitivity;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_exposure_mode(&mut self) -> Result<ExposureMode, Error> {
        let cmd = InquiryCommand::ExposureMode;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_exposure_compensation(&mut self) -> Result<i8, Error> {
        let cmd = InquiryCommand::ExposureCompensation;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_exposure_compensation_enabled(&mut self) -> Result<bool, Error> {
        let cmd = InquiryCommand::ExposureCompensationMode;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_iris(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Iris;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_shutter(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::Shutter;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_brightness(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::Bright;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_gain(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Gain;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Gain { gain }) => Ok(gain),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_gain_limit(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::GainLimit;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_anti_flicker(&mut self) -> Result<AntiFlickerMode, Error> {
        let cmd = InquiryCommand::AntiFlicker;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_backlight(&mut self) -> Result<bool, Error> {
        let cmd = InquiryCommand::Backlight;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Backlight { status }) => Ok(status),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_dynamic_range(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::DynamicRange;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_white_balance_mode(&mut self) -> Result<WhiteBalanceMode, Error> {
        let cmd = InquiryCommand::WhiteBalanceMode;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_color_temperature(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::ColorTemperature;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                Ok(temperature)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_red_gain(&mut self) -> Result<i8, Error> {
        let cmd = InquiryCommand::RedGain;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::RedGain { gain }) => Ok(gain),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_blue_gain(&mut self) -> Result<i8, Error> {
        let cmd = InquiryCommand::BlueGain;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::BlueGain { gain }) => Ok(gain),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_luminance(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Luminance;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Luminance(level)) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_contrast(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Contrast;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Contrast(level)) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_sharpness(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Sharpness;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_sharpness_mode(&mut self) -> Result<SharpnessMode, Error> {
        let cmd = InquiryCommand::SharpnessMode;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_saturation(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Saturation;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_hue(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Hue;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_noise_reduction_2d(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::NoiseReduction2D;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_noise_reduction_3d(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::NoiseReduction3D;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_image_flip(&mut self) -> Result<(bool, bool), Error> {
        let cmd = InquiryCommand::ImageFlip;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                horizontal,
                vertical,
            }) => Ok((horizontal, vertical)),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_black_white(&mut self) -> Result<bool, Error> {
        let cmd = InquiryCommand::BlackWhite;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_version(&mut self) -> Result<(u16, u16, u32, u8), Error> {
        let cmd = InquiryCommand::Version;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            }) => Ok((vendor, model, rom_version, max_socket)),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Async implementation for all cameras - uses the same dual_native_inquiry macro
#[cfg(feature = "async")]
impl<P, T> InquiryMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::AsyncTransport,
{
    #[dual_native_inquiry]
    fn get_power_state(&mut self) -> Result<bool, Error> {
        let cmd = InquiryCommand::Power;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_position(&mut self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error> {
        let cmd = InquiryCommand::PanTiltPosition;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((ViscaUnits(pan), ViscaUnits(tilt)))
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_zoom_position(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::ZoomPosition;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_focus_position(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::FocusPosition;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_focus_near_limit(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::FocusNearLimit;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_focus_zone(&mut self) -> Result<FocusZone, Error> {
        let cmd = InquiryCommand::FocusZone;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => Ok(zone),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_auto_focus_sensitivity(&mut self) -> Result<AutoFocusSensitivity, Error> {
        let cmd = InquiryCommand::AutoFocusSensitivity;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_exposure_mode(&mut self) -> Result<ExposureMode, Error> {
        let cmd = InquiryCommand::ExposureMode;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_exposure_compensation(&mut self) -> Result<i8, Error> {
        let cmd = InquiryCommand::ExposureCompensation;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_exposure_compensation_enabled(&mut self) -> Result<bool, Error> {
        let cmd = InquiryCommand::ExposureCompensationMode;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_iris(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Iris;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_shutter(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::Shutter;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_brightness(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::Bright;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_gain(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Gain;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Gain { gain }) => Ok(gain),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_gain_limit(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::GainLimit;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_anti_flicker(&mut self) -> Result<AntiFlickerMode, Error> {
        let cmd = InquiryCommand::AntiFlicker;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_backlight(&mut self) -> Result<bool, Error> {
        let cmd = InquiryCommand::Backlight;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Backlight { status }) => Ok(status),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_dynamic_range(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::DynamicRange;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_white_balance_mode(&mut self) -> Result<WhiteBalanceMode, Error> {
        let cmd = InquiryCommand::WhiteBalanceMode;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_color_temperature(&mut self) -> Result<u16, Error> {
        let cmd = InquiryCommand::ColorTemperature;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                Ok(temperature)
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_red_gain(&mut self) -> Result<i8, Error> {
        let cmd = InquiryCommand::RedGain;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::RedGain { gain }) => Ok(gain),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_blue_gain(&mut self) -> Result<i8, Error> {
        let cmd = InquiryCommand::BlueGain;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::BlueGain { gain }) => Ok(gain),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_luminance(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Luminance;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Luminance(level)) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_contrast(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Contrast;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Contrast(level)) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_sharpness(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Sharpness;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_sharpness_mode(&mut self) -> Result<SharpnessMode, Error> {
        let cmd = InquiryCommand::SharpnessMode;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_saturation(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Saturation;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_hue(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::Hue;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_noise_reduction_2d(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::NoiseReduction2D;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_noise_reduction_3d(&mut self) -> Result<u8, Error> {
        let cmd = InquiryCommand::NoiseReduction3D;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_image_flip(&mut self) -> Result<(bool, bool), Error> {
        let cmd = InquiryCommand::ImageFlip;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                horizontal,
                vertical,
            }) => Ok((horizontal, vertical)),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_black_white(&mut self) -> Result<bool, Error> {
        let cmd = InquiryCommand::BlackWhite;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    #[dual_native_inquiry]
    fn get_version(&mut self) -> Result<(u16, u16, u32, u8), Error> {
        let cmd = InquiryCommand::Version;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::Version {
                vendor,
                model,
                rom_version,
                max_socket,
            }) => Ok((vendor, model, rom_version, max_socket)),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation for cameras with PanTilt capability
#[cfg(not(feature = "async"))]
impl<P, T> PanTiltInquiryMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + PanTilt,
    T: crate::transport::blocking::BlockingTransport,
{
    #[dual_native_inquiry(await_methods(get_position))]
    fn get_position_degrees(&mut self) -> Result<(Degrees, Degrees), Error> {
        let (pan_units, tilt_units) = self.get_position()?;
        Ok((
            Degrees(pan_units.0 as f32 / P::PAN_DEGREES_TO_UNITS),
            Degrees(tilt_units.0 as f32 / P::TILT_DEGREES_TO_UNITS),
        ))
    }
}

// Async implementation for cameras with PanTilt capability
#[cfg(feature = "async")]
impl<P, T> PanTiltInquiryMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + PanTilt,
    T: crate::transport::AsyncTransport,
{
    #[dual_native_inquiry(await_methods(get_position))]
    fn get_position_degrees(&mut self) -> Result<(Degrees, Degrees), Error> {
        let (pan_units, tilt_units) = self.get_position()?;
        Ok((
            Degrees(pan_units.0 as f32 / P::PAN_DEGREES_TO_UNITS),
            Degrees(tilt_units.0 as f32 / P::TILT_DEGREES_TO_UNITS),
        ))
    }
}
