//! Inquiry methods for querying camera state using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{PanTilt, ProfileMetadata},
    command::{
        inquiry::InquiryCommand, AntiFlickerMode, AutoFocusSensitivity, ExposureMode, FocusZone,
        InquiryResponse, Response, SharpnessMode, WhiteBalanceMode,
    },
    transport::gat_transport::Transport,
    units::{Degrees, ViscaUnits},
    Error,
};
use core::future::Future;

/// Extension trait for CameraCore - provides future-returning methods.
pub trait InquiryCoreExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    // Power and Basic State

    /// Query the camera's power state - returns a future.
    fn get_power_state(&self) -> impl Future<Output = Result<bool, Error>> + '_;

    // Position Queries - return VISCA units for all cameras

    /// Query the current pan and tilt position in VISCA units - returns a future.
    fn get_position(
        &self,
    ) -> impl Future<Output = Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error>> + '_;

    // Optics Queries

    /// Query the current zoom position - returns a future.
    fn get_zoom_position(&self) -> impl Future<Output = Result<u16, Error>> + '_;

    /// Query the current focus position - returns a future.
    fn get_focus_position(&self) -> impl Future<Output = Result<u16, Error>> + '_;

    /// Query the focus near limit position - returns a future.
    fn get_focus_near_limit(&self) -> impl Future<Output = Result<u16, Error>> + '_;

    /// Query the current focus zone - returns a future.
    fn get_focus_zone(&self) -> impl Future<Output = Result<FocusZone, Error>> + '_;

    /// Query the auto focus sensitivity - returns a future.
    fn get_auto_focus_sensitivity(
        &self,
    ) -> impl Future<Output = Result<AutoFocusSensitivity, Error>> + '_;

    // Exposure Queries

    /// Query the current exposure mode - returns a future.
    fn get_exposure_mode(&self) -> impl Future<Output = Result<ExposureMode, Error>> + '_;

    /// Query the exposure compensation value - returns a future.
    fn get_exposure_compensation(&self) -> impl Future<Output = Result<i8, Error>> + '_;

    /// Query whether exposure compensation is enabled - returns a future.
    fn get_exposure_compensation_enabled(&self) -> impl Future<Output = Result<bool, Error>> + '_;

    /// Query the current iris position - returns a future.
    fn get_iris(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the current shutter position - returns a future.
    fn get_shutter(&self) -> impl Future<Output = Result<u16, Error>> + '_;

    /// Query the current brightness level - returns a future.
    fn get_brightness(&self) -> impl Future<Output = Result<u16, Error>> + '_;

    /// Query the current gain level - returns a future.
    fn get_gain(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the gain limit - returns a future.
    fn get_gain_limit(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the anti-flicker mode - returns a future.
    fn get_anti_flicker(&self) -> impl Future<Output = Result<AntiFlickerMode, Error>> + '_;

    /// Query the backlight compensation status - returns a future.
    fn get_backlight(&self) -> impl Future<Output = Result<bool, Error>> + '_;

    /// Query the dynamic range level - returns a future.
    fn get_dynamic_range(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    // Color Queries

    /// Query the current white balance mode - returns a future.
    fn get_white_balance_mode(&self) -> impl Future<Output = Result<WhiteBalanceMode, Error>> + '_;

    /// Query the color temperature (only valid in manual white balance mode) - returns a future.
    fn get_color_temperature(&self) -> impl Future<Output = Result<u16, Error>> + '_;

    /// Query the red gain value - returns a future.
    fn get_red_gain(&self) -> impl Future<Output = Result<i8, Error>> + '_;

    /// Query the blue gain value - returns a future.
    fn get_blue_gain(&self) -> impl Future<Output = Result<i8, Error>> + '_;

    // Image Quality Queries

    /// Query the luminance level - returns a future.
    fn get_luminance(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the contrast level - returns a future.
    fn get_contrast(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the sharpness value - returns a future.
    fn get_sharpness(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the sharpness mode - returns a future.
    fn get_sharpness_mode(&self) -> impl Future<Output = Result<SharpnessMode, Error>> + '_;

    /// Query the saturation level - returns a future.
    fn get_saturation(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the hue value - returns a future.
    fn get_hue(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the 2D noise reduction level - returns a future.
    fn get_noise_reduction_2d(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    /// Query the 3D noise reduction level - returns a future.
    fn get_noise_reduction_3d(&self) -> impl Future<Output = Result<u8, Error>> + '_;

    // Special Effects Queries

    /// Query the current image flip state - returns a future.
    fn get_image_flip(&self) -> impl Future<Output = Result<(bool, bool), Error>> + '_;

    /// Query whether black and white mode is enabled - returns a future.
    fn get_black_white(&self) -> impl Future<Output = Result<bool, Error>> + '_;

    // Camera Information

    /// Query the camera's version information - returns a future.
    fn get_version(&self) -> impl Future<Output = Result<(u16, u16, u32, u8), Error>> + '_;
}

impl<P, T> InquiryCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn get_power_state(&self) -> impl Future<Output = Result<bool, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Power;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_position(
        &self,
    ) -> impl Future<Output = Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error>> + '_ {
        async move {
            let cmd = InquiryCommand::PanTiltPosition;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                    Ok((ViscaUnits(pan), ViscaUnits(tilt)))
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_zoom_position(&self) -> impl Future<Output = Result<u16, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::ZoomPosition;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_focus_position(&self) -> impl Future<Output = Result<u16, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::FocusPosition;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_focus_near_limit(&self) -> impl Future<Output = Result<u16, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::FocusNearLimit;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => {
                    Ok(position)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_focus_zone(&self) -> impl Future<Output = Result<FocusZone, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::FocusZone;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => Ok(zone),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_auto_focus_sensitivity(
        &self,
    ) -> impl Future<Output = Result<AutoFocusSensitivity, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::AutoFocusSensitivity;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity {
                    sensitivity,
                }) => Ok(sensitivity),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_exposure_mode(&self) -> impl Future<Output = Result<ExposureMode, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::ExposureMode;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_exposure_compensation(&self) -> impl Future<Output = Result<i8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::ExposureCompensation;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => {
                    Ok(value)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_exposure_compensation_enabled(&self) -> impl Future<Output = Result<bool, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::ExposureCompensationMode;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => {
                    Ok(on)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_iris(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Iris;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_shutter(&self) -> impl Future<Output = Result<u16, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Shutter;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_brightness(&self) -> impl Future<Output = Result<u16, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Bright;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_gain(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Gain;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Gain { gain }) => Ok(gain),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_gain_limit(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::GainLimit;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_anti_flicker(&self) -> impl Future<Output = Result<AntiFlickerMode, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::AntiFlicker;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_backlight(&self) -> impl Future<Output = Result<bool, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Backlight;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Backlight { status }) => Ok(status),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_dynamic_range(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::DynamicRange;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_white_balance_mode(&self) -> impl Future<Output = Result<WhiteBalanceMode, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::WhiteBalanceMode;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_color_temperature(&self) -> impl Future<Output = Result<u16, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::ColorTemperature;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                    Ok(temperature)
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_red_gain(&self) -> impl Future<Output = Result<i8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::RedGain;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::RedGain { gain }) => Ok(gain),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_blue_gain(&self) -> impl Future<Output = Result<i8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::BlueGain;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::BlueGain { gain }) => Ok(gain),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_luminance(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Luminance;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Luminance(level)) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_contrast(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Contrast;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Contrast(level)) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_sharpness(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Sharpness;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_sharpness_mode(&self) -> impl Future<Output = Result<SharpnessMode, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::SharpnessMode;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_saturation(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Saturation;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_hue(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Hue;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_noise_reduction_2d(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::NoiseReduction2D;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_noise_reduction_3d(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::NoiseReduction3D;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_image_flip(&self) -> impl Future<Output = Result<(bool, bool), Error>> + '_ {
        async move {
            let cmd = InquiryCommand::ImageFlip;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::ImageFlip {
                    horizontal,
                    vertical,
                }) => Ok((horizontal, vertical)),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_black_white(&self) -> impl Future<Output = Result<bool, Error>> + '_ {
        async move {
            let cmd = InquiryCommand::BlackWhite;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_version(&self) -> impl Future<Output = Result<(u16, u16, u32, u8), Error>> + '_ {
        async move {
            let cmd = InquiryCommand::Version;
            let response = self.send_command(&cmd).await?;
            match response {
                Response::InquiryResponse(InquiryResponse::Version {
                    vendor,
                    model,
                    rom_version,
                    max_socket,
                }) => Ok((vendor, model, rom_version, max_socket)),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for async Camera facade.
pub trait InquiryAsyncExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    // Power and Basic State
    fn get_power_state(&self) -> impl Future<Output = Result<bool, Error>> + Send;

    // Position Queries
    fn get_position(
        &self,
    ) -> impl Future<Output = Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error>> + Send;

    // Optics Queries
    fn get_zoom_position(&self) -> impl Future<Output = Result<u16, Error>> + Send;
    fn get_focus_position(&self) -> impl Future<Output = Result<u16, Error>> + Send;
    fn get_focus_near_limit(&self) -> impl Future<Output = Result<u16, Error>> + Send;
    fn get_focus_zone(&self) -> impl Future<Output = Result<FocusZone, Error>> + Send;
    fn get_auto_focus_sensitivity(
        &self,
    ) -> impl Future<Output = Result<AutoFocusSensitivity, Error>> + Send;

    // Exposure Queries
    fn get_exposure_mode(&self) -> impl Future<Output = Result<ExposureMode, Error>> + Send;
    fn get_exposure_compensation(&self) -> impl Future<Output = Result<i8, Error>> + Send;
    fn get_exposure_compensation_enabled(&self)
        -> impl Future<Output = Result<bool, Error>> + Send;
    fn get_iris(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_shutter(&self) -> impl Future<Output = Result<u16, Error>> + Send;
    fn get_brightness(&self) -> impl Future<Output = Result<u16, Error>> + Send;
    fn get_gain(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_gain_limit(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_anti_flicker(&self) -> impl Future<Output = Result<AntiFlickerMode, Error>> + Send;
    fn get_backlight(&self) -> impl Future<Output = Result<bool, Error>> + Send;
    fn get_dynamic_range(&self) -> impl Future<Output = Result<u8, Error>> + Send;

    // Color Queries
    fn get_white_balance_mode(
        &self,
    ) -> impl Future<Output = Result<WhiteBalanceMode, Error>> + Send;
    fn get_color_temperature(&self) -> impl Future<Output = Result<u16, Error>> + Send;
    fn get_red_gain(&self) -> impl Future<Output = Result<i8, Error>> + Send;
    fn get_blue_gain(&self) -> impl Future<Output = Result<i8, Error>> + Send;

    // Image Quality Queries
    fn get_luminance(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_contrast(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_sharpness(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_sharpness_mode(&self) -> impl Future<Output = Result<SharpnessMode, Error>> + Send;
    fn get_saturation(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_hue(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_noise_reduction_2d(&self) -> impl Future<Output = Result<u8, Error>> + Send;
    fn get_noise_reduction_3d(&self) -> impl Future<Output = Result<u8, Error>> + Send;

    // Special Effects Queries
    fn get_image_flip(&self) -> impl Future<Output = Result<(bool, bool), Error>> + Send;
    fn get_black_white(&self) -> impl Future<Output = Result<bool, Error>> + Send;

    // Camera Information
    fn get_version(&self) -> impl Future<Output = Result<(u16, u16, u32, u8), Error>> + Send;
}

impl<P, T> InquiryAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn get_power_state(&self) -> impl Future<Output = Result<bool, Error>> + Send {
        async move { self.core().get_power_state().await }
    }

    fn get_position(
        &self,
    ) -> impl Future<Output = Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error>> + Send {
        async move { self.core().get_position().await }
    }

    fn get_zoom_position(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move { self.core().get_zoom_position().await }
    }

    fn get_focus_position(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move { self.core().get_focus_position().await }
    }

    fn get_focus_near_limit(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move { self.core().get_focus_near_limit().await }
    }

    fn get_focus_zone(&self) -> impl Future<Output = Result<FocusZone, Error>> + Send {
        async move { self.core().get_focus_zone().await }
    }

    fn get_auto_focus_sensitivity(
        &self,
    ) -> impl Future<Output = Result<AutoFocusSensitivity, Error>> + Send {
        async move { self.core().get_auto_focus_sensitivity().await }
    }

    fn get_exposure_mode(&self) -> impl Future<Output = Result<ExposureMode, Error>> + Send {
        async move { self.core().get_exposure_mode().await }
    }

    fn get_exposure_compensation(&self) -> impl Future<Output = Result<i8, Error>> + Send {
        async move { self.core().get_exposure_compensation().await }
    }

    fn get_exposure_compensation_enabled(
        &self,
    ) -> impl Future<Output = Result<bool, Error>> + Send {
        async move { self.core().get_exposure_compensation_enabled().await }
    }

    fn get_iris(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_iris().await }
    }

    fn get_shutter(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move { self.core().get_shutter().await }
    }

    fn get_brightness(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move { self.core().get_brightness().await }
    }

    fn get_gain(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_gain().await }
    }

    fn get_gain_limit(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_gain_limit().await }
    }

    fn get_anti_flicker(&self) -> impl Future<Output = Result<AntiFlickerMode, Error>> + Send {
        async move { self.core().get_anti_flicker().await }
    }

    fn get_backlight(&self) -> impl Future<Output = Result<bool, Error>> + Send {
        async move { self.core().get_backlight().await }
    }

    fn get_dynamic_range(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_dynamic_range().await }
    }

    fn get_white_balance_mode(
        &self,
    ) -> impl Future<Output = Result<WhiteBalanceMode, Error>> + Send {
        async move { self.core().get_white_balance_mode().await }
    }

    fn get_color_temperature(&self) -> impl Future<Output = Result<u16, Error>> + Send {
        async move { self.core().get_color_temperature().await }
    }

    fn get_red_gain(&self) -> impl Future<Output = Result<i8, Error>> + Send {
        async move { self.core().get_red_gain().await }
    }

    fn get_blue_gain(&self) -> impl Future<Output = Result<i8, Error>> + Send {
        async move { self.core().get_blue_gain().await }
    }

    fn get_luminance(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_luminance().await }
    }

    fn get_contrast(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_contrast().await }
    }

    fn get_sharpness(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_sharpness().await }
    }

    fn get_sharpness_mode(&self) -> impl Future<Output = Result<SharpnessMode, Error>> + Send {
        async move { self.core().get_sharpness_mode().await }
    }

    fn get_saturation(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_saturation().await }
    }

    fn get_hue(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_hue().await }
    }

    fn get_noise_reduction_2d(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_noise_reduction_2d().await }
    }

    fn get_noise_reduction_3d(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_noise_reduction_3d().await }
    }

    fn get_image_flip(&self) -> impl Future<Output = Result<(bool, bool), Error>> + Send {
        async move { self.core().get_image_flip().await }
    }

    fn get_black_white(&self) -> impl Future<Output = Result<bool, Error>> + Send {
        async move { self.core().get_black_white().await }
    }

    fn get_version(&self) -> impl Future<Output = Result<(u16, u16, u32, u8), Error>> + Send {
        async move { self.core().get_version().await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait InquiryBlockingExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    // Power and Basic State
    fn get_power_state(&self) -> Result<bool, Error>;

    // Position Queries
    fn get_position(&self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error>;

    // Optics Queries
    fn get_zoom_position(&self) -> Result<u16, Error>;
    fn get_focus_position(&self) -> Result<u16, Error>;
    fn get_focus_near_limit(&self) -> Result<u16, Error>;
    fn get_focus_zone(&self) -> Result<FocusZone, Error>;
    fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error>;

    // Exposure Queries
    fn get_exposure_mode(&self) -> Result<ExposureMode, Error>;
    fn get_exposure_compensation(&self) -> Result<i8, Error>;
    fn get_exposure_compensation_enabled(&self) -> Result<bool, Error>;
    fn get_iris(&self) -> Result<u8, Error>;
    fn get_shutter(&self) -> Result<u16, Error>;
    fn get_brightness(&self) -> Result<u16, Error>;
    fn get_gain(&self) -> Result<u8, Error>;
    fn get_gain_limit(&self) -> Result<u8, Error>;
    fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error>;
    fn get_backlight(&self) -> Result<bool, Error>;
    fn get_dynamic_range(&self) -> Result<u8, Error>;

    // Color Queries
    fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error>;
    fn get_color_temperature(&self) -> Result<u16, Error>;
    fn get_red_gain(&self) -> Result<i8, Error>;
    fn get_blue_gain(&self) -> Result<i8, Error>;

    // Image Quality Queries
    fn get_luminance(&self) -> Result<u8, Error>;
    fn get_contrast(&self) -> Result<u8, Error>;
    fn get_sharpness(&self) -> Result<u8, Error>;
    fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error>;
    fn get_saturation(&self) -> Result<u8, Error>;
    fn get_hue(&self) -> Result<u8, Error>;
    fn get_noise_reduction_2d(&self) -> Result<u8, Error>;
    fn get_noise_reduction_3d(&self) -> Result<u8, Error>;

    // Special Effects Queries
    fn get_image_flip(&self) -> Result<(bool, bool), Error>;
    fn get_black_white(&self) -> Result<bool, Error>;

    // Camera Information
    fn get_version(&self) -> Result<(u16, u16, u32, u8), Error>;
}

impl<P, T> InquiryBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn get_power_state(&self) -> Result<bool, Error> {
        block_on(self.core().get_power_state())
    }

    fn get_position(&self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error> {
        block_on(self.core().get_position())
    }

    fn get_zoom_position(&self) -> Result<u16, Error> {
        block_on(self.core().get_zoom_position())
    }

    fn get_focus_position(&self) -> Result<u16, Error> {
        block_on(self.core().get_focus_position())
    }

    fn get_focus_near_limit(&self) -> Result<u16, Error> {
        block_on(self.core().get_focus_near_limit())
    }

    fn get_focus_zone(&self) -> Result<FocusZone, Error> {
        block_on(self.core().get_focus_zone())
    }

    fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error> {
        block_on(self.core().get_auto_focus_sensitivity())
    }

    fn get_exposure_mode(&self) -> Result<ExposureMode, Error> {
        block_on(self.core().get_exposure_mode())
    }

    fn get_exposure_compensation(&self) -> Result<i8, Error> {
        block_on(self.core().get_exposure_compensation())
    }

    fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        block_on(self.core().get_exposure_compensation_enabled())
    }

    fn get_iris(&self) -> Result<u8, Error> {
        block_on(self.core().get_iris())
    }

    fn get_shutter(&self) -> Result<u16, Error> {
        block_on(self.core().get_shutter())
    }

    fn get_brightness(&self) -> Result<u16, Error> {
        block_on(self.core().get_brightness())
    }

    fn get_gain(&self) -> Result<u8, Error> {
        block_on(self.core().get_gain())
    }

    fn get_gain_limit(&self) -> Result<u8, Error> {
        block_on(self.core().get_gain_limit())
    }

    fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error> {
        block_on(self.core().get_anti_flicker())
    }

    fn get_backlight(&self) -> Result<bool, Error> {
        block_on(self.core().get_backlight())
    }

    fn get_dynamic_range(&self) -> Result<u8, Error> {
        block_on(self.core().get_dynamic_range())
    }

    fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error> {
        block_on(self.core().get_white_balance_mode())
    }

    fn get_color_temperature(&self) -> Result<u16, Error> {
        block_on(self.core().get_color_temperature())
    }

    fn get_red_gain(&self) -> Result<i8, Error> {
        block_on(self.core().get_red_gain())
    }

    fn get_blue_gain(&self) -> Result<i8, Error> {
        block_on(self.core().get_blue_gain())
    }

    fn get_luminance(&self) -> Result<u8, Error> {
        block_on(self.core().get_luminance())
    }

    fn get_contrast(&self) -> Result<u8, Error> {
        block_on(self.core().get_contrast())
    }

    fn get_sharpness(&self) -> Result<u8, Error> {
        block_on(self.core().get_sharpness())
    }

    fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error> {
        block_on(self.core().get_sharpness_mode())
    }

    fn get_saturation(&self) -> Result<u8, Error> {
        block_on(self.core().get_saturation())
    }

    fn get_hue(&self) -> Result<u8, Error> {
        block_on(self.core().get_hue())
    }

    fn get_noise_reduction_2d(&self) -> Result<u8, Error> {
        block_on(self.core().get_noise_reduction_2d())
    }

    fn get_noise_reduction_3d(&self) -> Result<u8, Error> {
        block_on(self.core().get_noise_reduction_3d())
    }

    fn get_image_flip(&self) -> Result<(bool, bool), Error> {
        block_on(self.core().get_image_flip())
    }

    fn get_black_white(&self) -> Result<bool, Error> {
        block_on(self.core().get_black_white())
    }

    fn get_version(&self) -> Result<(u16, u16, u32, u8), Error> {
        block_on(self.core().get_version())
    }
}

/// Extension trait for CameraCore that adds pan/tilt-specific inquiry methods.
pub trait PanTiltInquiryCoreExt<P, T>
where
    P: ProfileMetadata + PanTilt,
    T: Transport,
{
    /// Query the current pan and tilt position in degrees - returns a future.
    fn get_position_degrees(&self) -> impl Future<Output = Result<(Degrees, Degrees), Error>> + '_;
}

impl<P, T> PanTiltInquiryCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + PanTilt,
    T: Transport,
{
    fn get_position_degrees(&self) -> impl Future<Output = Result<(Degrees, Degrees), Error>> + '_ {
        async move {
            let (pan_units, tilt_units) = self.get_position().await?;
            Ok((
                Degrees(pan_units.0 as f32 / P::PAN_DEGREES_TO_UNITS),
                Degrees(tilt_units.0 as f32 / P::TILT_DEGREES_TO_UNITS),
            ))
        }
    }
}

/// Extension trait for async Camera facade with pan/tilt capability.
pub trait PanTiltInquiryAsyncExt<P, T>
where
    P: ProfileMetadata + PanTilt,
    T: Transport,
{
    /// Query the current pan and tilt position in degrees.
    fn get_position_degrees(
        &self,
    ) -> impl Future<Output = Result<(Degrees, Degrees), Error>> + Send;
}

impl<P, T> PanTiltInquiryAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + PanTilt + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn get_position_degrees(
        &self,
    ) -> impl Future<Output = Result<(Degrees, Degrees), Error>> + Send {
        async move { self.core().get_position_degrees().await }
    }
}

/// Extension trait for blocking Camera facade with pan/tilt capability.
pub trait PanTiltInquiryBlockingExt<P, T>
where
    P: ProfileMetadata + PanTilt,
    T: Transport,
{
    /// Query the current pan and tilt position in degrees.
    fn get_position_degrees(&self) -> Result<(Degrees, Degrees), Error>;
}

impl<P, T> PanTiltInquiryBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + PanTilt,
    T: Transport,
{
    fn get_position_degrees(&self) -> Result<(Degrees, Degrees), Error> {
        block_on(self.core().get_position_degrees())
    }
}
