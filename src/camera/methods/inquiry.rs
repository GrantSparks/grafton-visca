//! Inquiry methods for querying camera state.

use crate::camera::Camera;
use crate::capabilities::{pan_tilt::PanTiltExt, PanTilt, ProfileMetadata};
use crate::command::inquiry::InquiryCommand;
use crate::command::{
    AntiFlickerMode, AutoFocusSensitivity, Command, ExposureMode, FocusZone, InquiryResponse,
    Response, SharpnessMode, WhiteBalanceMode,
};
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

    /// Query the focus zone setting.
    #[cfg(not(feature = "async"))]
    fn get_focus_zone(&mut self) -> Result<FocusZone, Error>;

    /// Query the focus zone setting.
    #[cfg(feature = "async")]
    async fn get_focus_zone(&self) -> Result<FocusZone, Error>;

    /// Query the auto-focus sensitivity setting.
    #[cfg(not(feature = "async"))]
    fn get_auto_focus_sensitivity(&mut self) -> Result<AutoFocusSensitivity, Error>;

    /// Query the auto-focus sensitivity setting.
    #[cfg(feature = "async")]
    async fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error>;

    // Exposure Queries

    /// Query the current exposure mode.
    #[cfg(not(feature = "async"))]
    fn get_exposure_mode(&mut self) -> Result<ExposureMode, Error>;

    /// Query the current exposure mode.
    #[cfg(feature = "async")]
    async fn get_exposure_mode(&self) -> Result<ExposureMode, Error>;

    /// Query the current exposure compensation value.
    #[cfg(not(feature = "async"))]
    fn get_exposure_compensation(&mut self) -> Result<i8, Error>;

    /// Query the current exposure compensation value.
    #[cfg(feature = "async")]
    async fn get_exposure_compensation(&self) -> Result<i8, Error>;

    /// Query whether exposure compensation is enabled.
    #[cfg(not(feature = "async"))]
    fn get_exposure_compensation_enabled(&mut self) -> Result<bool, Error>;

    /// Query whether exposure compensation is enabled.
    #[cfg(feature = "async")]
    async fn get_exposure_compensation_enabled(&self) -> Result<bool, Error>;

    /// Query the current iris setting.
    #[cfg(not(feature = "async"))]
    fn get_iris(&mut self) -> Result<u8, Error>;

    /// Query the current iris setting.
    #[cfg(feature = "async")]
    async fn get_iris(&self) -> Result<u8, Error>;

    /// Query the current shutter speed.
    #[cfg(not(feature = "async"))]
    fn get_shutter(&mut self) -> Result<u16, Error>;

    /// Query the current shutter speed.
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

    /// Query the current gain limit.
    #[cfg(not(feature = "async"))]
    fn get_gain_limit(&mut self) -> Result<u8, Error>;

    /// Query the current gain limit.
    #[cfg(feature = "async")]
    async fn get_gain_limit(&self) -> Result<u8, Error>;

    /// Query the anti-flicker mode.
    #[cfg(not(feature = "async"))]
    fn get_anti_flicker(&mut self) -> Result<AntiFlickerMode, Error>;

    /// Query the anti-flicker mode.
    #[cfg(feature = "async")]
    async fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error>;

    /// Query whether backlight compensation is enabled.
    #[cfg(not(feature = "async"))]
    fn get_backlight(&mut self) -> Result<bool, Error>;

    /// Query whether backlight compensation is enabled.
    #[cfg(feature = "async")]
    async fn get_backlight(&self) -> Result<bool, Error>;

    /// Query the dynamic range control level.
    #[cfg(not(feature = "async"))]
    fn get_dynamic_range(&mut self) -> Result<u8, Error>;

    /// Query the dynamic range control level.
    #[cfg(feature = "async")]
    async fn get_dynamic_range(&self) -> Result<u8, Error>;

    // White Balance Queries

    /// Query the current white balance mode.
    #[cfg(not(feature = "async"))]
    fn get_white_balance_mode(&mut self) -> Result<WhiteBalanceMode, Error>;

    /// Query the current white balance mode.
    #[cfg(feature = "async")]
    async fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error>;

    /// Query the current color temperature value.
    #[cfg(not(feature = "async"))]
    fn get_color_temperature(&mut self) -> Result<u16, Error>;

    /// Query the current color temperature value.
    #[cfg(feature = "async")]
    async fn get_color_temperature(&self) -> Result<u16, Error>;

    /// Query the red gain tuning value.
    #[cfg(not(feature = "async"))]
    fn get_red_gain(&mut self) -> Result<i8, Error>;

    /// Query the red gain tuning value.
    #[cfg(feature = "async")]
    async fn get_red_gain(&self) -> Result<i8, Error>;

    /// Query the blue gain tuning value.
    #[cfg(not(feature = "async"))]
    fn get_blue_gain(&mut self) -> Result<i8, Error>;

    /// Query the blue gain tuning value.
    #[cfg(feature = "async")]
    async fn get_blue_gain(&self) -> Result<i8, Error>;

    // Image Quality Queries

    /// Query the current luminance level.
    #[cfg(not(feature = "async"))]
    fn get_luminance(&mut self) -> Result<u8, Error>;

    /// Query the current luminance level.
    #[cfg(feature = "async")]
    async fn get_luminance(&self) -> Result<u8, Error>;

    /// Query the current contrast level.
    #[cfg(not(feature = "async"))]
    fn get_contrast(&mut self) -> Result<u8, Error>;

    /// Query the current contrast level.
    #[cfg(feature = "async")]
    async fn get_contrast(&self) -> Result<u8, Error>;

    /// Query the current sharpness level.
    #[cfg(not(feature = "async"))]
    fn get_sharpness(&mut self) -> Result<u8, Error>;

    /// Query the current sharpness level.
    #[cfg(feature = "async")]
    async fn get_sharpness(&self) -> Result<u8, Error>;

    /// Query the sharpness mode.
    #[cfg(not(feature = "async"))]
    fn get_sharpness_mode(&mut self) -> Result<SharpnessMode, Error>;

    /// Query the sharpness mode.
    #[cfg(feature = "async")]
    async fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error>;

    /// Query the current saturation level.
    #[cfg(not(feature = "async"))]
    fn get_saturation(&mut self) -> Result<u8, Error>;

    /// Query the current saturation level.
    #[cfg(feature = "async")]
    async fn get_saturation(&self) -> Result<u8, Error>;

    /// Query the current hue level.
    #[cfg(not(feature = "async"))]
    fn get_hue(&mut self) -> Result<u8, Error>;

    /// Query the current hue level.
    #[cfg(feature = "async")]
    async fn get_hue(&self) -> Result<u8, Error>;

    /// Query the 2D noise reduction setting.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_2d(&mut self) -> Result<u8, Error>;

    /// Query the 2D noise reduction setting.
    #[cfg(feature = "async")]
    async fn get_noise_reduction_2d(&self) -> Result<u8, Error>;

    /// Query the 3D noise reduction setting.
    #[cfg(not(feature = "async"))]
    fn get_noise_reduction_3d(&mut self) -> Result<u8, Error>;

    /// Query the 3D noise reduction setting.
    #[cfg(feature = "async")]
    async fn get_noise_reduction_3d(&self) -> Result<u8, Error>;

    // Image Effects Queries

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

// Helper macro for async implementation
#[allow(unused_macros)]
macro_rules! impl_inquiry_method_async {
    // Simple variant - returns a single value directly from the response
    ($method:ident, $inquiry_variant:ident, $response_pattern:pat => $extract:expr, $ret_type:ty) => {
        async fn $method(&self) -> Result<$ret_type, Error> {
            let cmd = InquiryCommand::$inquiry_variant;
            let bytes = cmd.to_bytes()?;
            let response = self.send_raw(&bytes).await?;
            let parsed = Response::parse(&response)?;
            match parsed {
                Response::InquiryResponse($response_pattern) => Ok($extract),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    };
}

// Helper macro for blocking implementation
#[allow(unused_macros)]
macro_rules! impl_inquiry_method {
    // Simple variant - returns a single value directly from the response
    ($method:ident, $inquiry_variant:ident, $response_pattern:pat => $extract:expr, $ret_type:ty) => {
        fn $method(&mut self) -> Result<$ret_type, Error> {
            let cmd = InquiryCommand::$inquiry_variant;
            let bytes = cmd.to_bytes()?;
            let response = self.send_raw(&bytes)?;
            let parsed = Response::parse(&response)?;
            match parsed {
                Response::InquiryResponse($response_pattern) => Ok($extract),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    };
}

// Blocking implementation for all cameras
#[cfg(not(feature = "async"))]
impl<P, T> InquiryMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::blocking::BlockingTransport,
{
    impl_inquiry_method!(get_power_state, Power, InquiryResponse::Power { on } => on, bool);

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

    impl_inquiry_method!(get_zoom_position, ZoomPosition, InquiryResponse::ZoomPosition { position } => position, u16);
    impl_inquiry_method!(get_focus_position, FocusPosition, InquiryResponse::FocusPosition { position } => position, u16);
    impl_inquiry_method!(get_focus_near_limit, FocusNearLimit, InquiryResponse::FocusNearLimit { position } => position, u16);
    impl_inquiry_method!(get_focus_zone, FocusZone, InquiryResponse::FocusZone { zone } => zone, FocusZone);
    impl_inquiry_method!(get_auto_focus_sensitivity, AutoFocusSensitivity, InquiryResponse::AutoFocusSensitivity { sensitivity } => sensitivity, AutoFocusSensitivity);
    impl_inquiry_method!(get_exposure_mode, ExposureMode, InquiryResponse::ExposureMode { mode } => mode, ExposureMode);
    impl_inquiry_method!(get_exposure_compensation, ExposureCompensation, InquiryResponse::ExposureCompensation { value } => value, i8);
    impl_inquiry_method!(get_exposure_compensation_enabled, ExposureCompensationMode, InquiryResponse::ExposureCompensationMode { on } => on, bool);
    impl_inquiry_method!(get_iris, Iris, InquiryResponse::Iris { position } => position, u8);
    impl_inquiry_method!(get_shutter, Shutter, InquiryResponse::Shutter { position } => position, u16);
    impl_inquiry_method!(get_brightness, Bright, InquiryResponse::Bright { position } => position, u16);
    impl_inquiry_method!(get_gain, Gain, InquiryResponse::Gain { gain } => gain, u8);
    impl_inquiry_method!(get_gain_limit, GainLimit, InquiryResponse::GainLimit { limit } => limit, u8);
    impl_inquiry_method!(get_anti_flicker, AntiFlicker, InquiryResponse::AntiFlicker { mode } => mode, AntiFlickerMode);
    impl_inquiry_method!(get_backlight, Backlight, InquiryResponse::Backlight { status } => status, bool);
    impl_inquiry_method!(get_dynamic_range, DynamicRange, InquiryResponse::DynamicRange { level } => level, u8);
    impl_inquiry_method!(get_white_balance_mode, WhiteBalanceMode, InquiryResponse::WhiteBalance { mode } => mode, WhiteBalanceMode);
    impl_inquiry_method!(get_color_temperature, ColorTemperature, InquiryResponse::ColorTemperature { temperature } => temperature, u16);
    impl_inquiry_method!(get_red_gain, RedGain, InquiryResponse::RedGain { gain } => gain, i8);
    impl_inquiry_method!(get_blue_gain, BlueGain, InquiryResponse::BlueGain { gain } => gain, i8);
    impl_inquiry_method!(get_luminance, Luminance, InquiryResponse::Luminance(level) => level, u8);
    impl_inquiry_method!(get_contrast, Contrast, InquiryResponse::Contrast(level) => level, u8);
    impl_inquiry_method!(get_sharpness, Sharpness, InquiryResponse::Sharpness { value } => value, u8);
    impl_inquiry_method!(get_sharpness_mode, SharpnessMode, InquiryResponse::SharpnessMode { mode } => mode, SharpnessMode);
    impl_inquiry_method!(get_saturation, Saturation, InquiryResponse::Saturation { level } => level, u8);
    impl_inquiry_method!(get_hue, Hue, InquiryResponse::Hue { hue } => hue, u8);
    impl_inquiry_method!(get_noise_reduction_2d, NoiseReduction2D, InquiryResponse::NoiseReduction2D { level } => level, u8);
    impl_inquiry_method!(get_noise_reduction_3d, NoiseReduction3D, InquiryResponse::NoiseReduction3D { level } => level, u8);

    fn get_image_flip(&mut self) -> Result<(bool, bool), Error> {
        let cmd = InquiryCommand::ImageFlip;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes)?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok((vertical, horizontal)),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    impl_inquiry_method!(get_black_white, BlackWhite, InquiryResponse::BlackWhite { on } => on, bool);

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

// Async implementation for all cameras
#[cfg(feature = "async")]
impl<P, T> InquiryMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::AsyncTransport,
{
    impl_inquiry_method_async!(get_power_state, Power, InquiryResponse::Power { on } => on, bool);

    async fn get_position(&self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error> {
        let cmd = InquiryCommand::PanTiltPosition;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes).await?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((ViscaUnits(pan), ViscaUnits(tilt)))
            }
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    impl_inquiry_method_async!(get_zoom_position, ZoomPosition, InquiryResponse::ZoomPosition { position } => position, u16);
    impl_inquiry_method_async!(get_focus_position, FocusPosition, InquiryResponse::FocusPosition { position } => position, u16);
    impl_inquiry_method_async!(get_focus_near_limit, FocusNearLimit, InquiryResponse::FocusNearLimit { position } => position, u16);
    impl_inquiry_method_async!(get_focus_zone, FocusZone, InquiryResponse::FocusZone { zone } => zone, FocusZone);
    impl_inquiry_method_async!(get_auto_focus_sensitivity, AutoFocusSensitivity, InquiryResponse::AutoFocusSensitivity { sensitivity } => sensitivity, AutoFocusSensitivity);
    impl_inquiry_method_async!(get_exposure_mode, ExposureMode, InquiryResponse::ExposureMode { mode } => mode, ExposureMode);
    impl_inquiry_method_async!(get_exposure_compensation, ExposureCompensation, InquiryResponse::ExposureCompensation { value } => value, i8);
    impl_inquiry_method_async!(get_exposure_compensation_enabled, ExposureCompensationMode, InquiryResponse::ExposureCompensationMode { on } => on, bool);
    impl_inquiry_method_async!(get_iris, Iris, InquiryResponse::Iris { position } => position, u8);
    impl_inquiry_method_async!(get_shutter, Shutter, InquiryResponse::Shutter { position } => position, u16);
    impl_inquiry_method_async!(get_brightness, Bright, InquiryResponse::Bright { position } => position, u16);
    impl_inquiry_method_async!(get_gain, Gain, InquiryResponse::Gain { gain } => gain, u8);
    impl_inquiry_method_async!(get_gain_limit, GainLimit, InquiryResponse::GainLimit { limit } => limit, u8);
    impl_inquiry_method_async!(get_anti_flicker, AntiFlicker, InquiryResponse::AntiFlicker { mode } => mode, AntiFlickerMode);
    impl_inquiry_method_async!(get_backlight, Backlight, InquiryResponse::Backlight { status } => status, bool);
    impl_inquiry_method_async!(get_dynamic_range, DynamicRange, InquiryResponse::DynamicRange { level } => level, u8);
    impl_inquiry_method_async!(get_white_balance_mode, WhiteBalanceMode, InquiryResponse::WhiteBalance { mode } => mode, WhiteBalanceMode);
    impl_inquiry_method_async!(get_color_temperature, ColorTemperature, InquiryResponse::ColorTemperature { temperature } => temperature, u16);
    impl_inquiry_method_async!(get_red_gain, RedGain, InquiryResponse::RedGain { gain } => gain, i8);
    impl_inquiry_method_async!(get_blue_gain, BlueGain, InquiryResponse::BlueGain { gain } => gain, i8);
    impl_inquiry_method_async!(get_luminance, Luminance, InquiryResponse::Luminance(level) => level, u8);
    impl_inquiry_method_async!(get_contrast, Contrast, InquiryResponse::Contrast(level) => level, u8);
    impl_inquiry_method_async!(get_sharpness, Sharpness, InquiryResponse::Sharpness { value } => value, u8);
    impl_inquiry_method_async!(get_sharpness_mode, SharpnessMode, InquiryResponse::SharpnessMode { mode } => mode, SharpnessMode);
    impl_inquiry_method_async!(get_saturation, Saturation, InquiryResponse::Saturation { level } => level, u8);
    impl_inquiry_method_async!(get_hue, Hue, InquiryResponse::Hue { hue } => hue, u8);
    impl_inquiry_method_async!(get_noise_reduction_2d, NoiseReduction2D, InquiryResponse::NoiseReduction2D { level } => level, u8);
    impl_inquiry_method_async!(get_noise_reduction_3d, NoiseReduction3D, InquiryResponse::NoiseReduction3D { level } => level, u8);

    async fn get_image_flip(&self) -> Result<(bool, bool), Error> {
        let cmd = InquiryCommand::ImageFlip;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes).await?;
        let parsed = Response::parse(&response)?;
        match parsed {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok((vertical, horizontal)),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    impl_inquiry_method_async!(get_black_white, BlackWhite, InquiryResponse::BlackWhite { on } => on, bool);

    async fn get_version(&self) -> Result<(u16, u16, u32, u8), Error> {
        let cmd = InquiryCommand::Version;
        let bytes = cmd.to_bytes()?;
        let response = self.send_raw(&bytes).await?;
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
    P: ProfileMetadata + PanTilt + Default,
    T: crate::transport::blocking::BlockingTransport,
{
    fn get_position_degrees(&mut self) -> Result<(Degrees, Degrees), Error> {
        // Get position in VISCA units first
        let (pan_units, tilt_units) = self.get_position()?;

        // Create a dummy profile instance to use extension trait methods
        let profile = P::default();
        let pan_degrees = profile.pan_units_to_degrees(pan_units.0);
        let tilt_degrees = profile.tilt_units_to_degrees(tilt_units.0);

        Ok((Degrees(pan_degrees), Degrees(tilt_degrees)))
    }
}

// Async implementation for cameras with PanTilt capability
#[cfg(feature = "async")]
impl<P, T> PanTiltInquiryMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + PanTilt + Default,
    T: crate::transport::AsyncTransport,
{
    async fn get_position_degrees(&self) -> Result<(Degrees, Degrees), Error> {
        // Get position in VISCA units first
        let (pan_units, tilt_units) = self.get_position().await?;

        // Create a dummy profile instance to use extension trait methods
        let profile = P::default();
        let pan_degrees = profile.pan_units_to_degrees(pan_units.0);
        let tilt_degrees = profile.tilt_units_to_degrees(tilt_units.0);

        Ok((Degrees(pan_degrees), Degrees(tilt_degrees)))
    }
}
