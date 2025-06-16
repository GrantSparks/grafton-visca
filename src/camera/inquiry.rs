//! Inquiry methods for `Camera<P>`.

use crate::{
    command::{
        exposure::ExposureMode,
        focus::{AutoFocusSensitivity, FocusZone},
        gain::AntiFlickerMode,
        inquiry::InquiryCommand,
        luminance_contrast_sharpness::SharpnessMode,
        response::{parse_response, Response},
        white_balance::WhiteBalanceMode,
        InquiryResponse,
    },
    error::Error,
    Command,
};

use super::{
    units::{Degrees, ViscaUnits},
    Camera, CameraProfile,
};

/// Camera state information retrieved from inquiries.
#[derive(Debug, Clone, Copy)]
pub struct CameraState {
    /// Power state.
    pub power: bool,
    /// Current position.
    pub position: Position,
    /// Current optics settings.
    pub optics: Optics,
    /// Current exposure settings.
    pub exposure: Exposure,
    /// Current white balance settings.
    pub white_balance: WhiteBalance,
    /// Current image settings.
    pub image: ImageSettings,
}

/// Position information.
#[derive(Debug, Clone, Copy)]
pub struct Position {
    /// Pan position in VISCA units.
    pub pan: i16,
    /// Tilt position in VISCA units.
    pub tilt: i16,
    /// Pan position in degrees.
    pub pan_degrees: f32,
    /// Tilt position in degrees.
    pub tilt_degrees: f32,
}

/// Optics settings.
#[derive(Debug, Clone, Copy)]
pub struct Optics {
    /// Zoom position.
    pub zoom: u16,
    /// Focus position.
    pub focus: u16,
}

/// Exposure settings.
#[derive(Debug, Clone, Copy)]
pub struct Exposure {
    /// Exposure mode.
    pub mode: ExposureMode,
    /// Exposure compensation value.
    pub compensation: Option<i8>,
    /// Iris setting.
    pub iris: Option<u8>,
    /// Shutter speed.
    pub shutter: Option<u8>,
    /// Brightness level.
    pub bright: Option<u8>,
    /// Gain level.
    pub gain: Option<u8>,
}

/// White balance settings.
#[derive(Debug, Clone, Copy)]
pub struct WhiteBalance {
    /// White balance mode.
    pub mode: WhiteBalanceMode,
    /// Red gain.
    pub red_gain: Option<i8>,
    /// Blue gain.
    pub blue_gain: Option<i8>,
}

/// Image quality settings.
#[derive(Debug, Clone, Copy)]
pub struct ImageSettings {
    /// Luminance level.
    pub luminance: u8,
    /// Contrast level.
    pub contrast: u8,
    /// Sharpness level.
    pub sharpness: u8,
    /// Saturation level.
    pub saturation: u8,
    /// Hue level.
    pub hue: u8,
}

impl<P: CameraProfile> Camera<P> {
    /// Send a command and wait for the response.
    ///
    /// This method is used internally for inquiry commands that need to receive data back.
    async fn send_and_receive(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // Send the command
        self.transport.send_command(command).await?;

        // Receive response frames
        let response_frames = self.transport.receive_response().await?;

        // Parse the response
        if response_frames.is_empty() {
            return Err(Error::NoResponse);
        }

        // For now, we'll process the first response frame
        // In a more complete implementation, we might need to handle multiple frames
        if let Some(response_type) = command.response_type() {
            parse_response(&response_frames[0], &response_type)
        } else {
            // For commands without a specific response type, parse as unknown
            Ok(Response::Unknown(response_frames[0].clone()))
        }
    }

    // Power inquiries

    /// Get the current power state of the camera.
    pub async fn get_power_state(&mut self) -> Result<bool, Error> {
        match self.send_and_receive(&InquiryCommand::Power).await? {
            Response::InquiryResponse(InquiryResponse::Power { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Position inquiries

    /// Get the current pan/tilt position in degrees.
    pub async fn get_position(&mut self) -> Result<(Degrees<f32>, Degrees<f32>), Error> {
        match self
            .send_and_receive(&InquiryCommand::PanTiltPosition)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                let pan_deg = self.profile.pan_units_to_degrees(pan);
                let tilt_deg = self.profile.tilt_units_to_degrees(tilt);
                Ok((Degrees(pan_deg), Degrees(tilt_deg)))
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current pan/tilt position in VISCA units.
    pub async fn get_position_units(
        &mut self,
    ) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error> {
        match self
            .send_and_receive(&InquiryCommand::PanTiltPosition)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((ViscaUnits(pan), ViscaUnits(tilt)))
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Zoom and focus inquiries

    /// Get the current zoom position.
    pub async fn get_zoom_position(&mut self) -> Result<u16, Error> {
        match self.send_and_receive(&InquiryCommand::ZoomPosition).await? {
            Response::InquiryResponse(InquiryResponse::ZoomPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current focus position.
    pub async fn get_focus_position(&mut self) -> Result<u16, Error> {
        match self
            .send_and_receive(&InquiryCommand::FocusPosition)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::FocusPosition { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Exposure inquiries

    /// Get the current exposure mode.
    pub async fn get_exposure_mode(&mut self) -> Result<ExposureMode, Error> {
        match self.send_and_receive(&InquiryCommand::ExposureMode).await? {
            Response::InquiryResponse(InquiryResponse::ExposureMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current exposure compensation value.
    pub async fn get_exposure_compensation(&mut self) -> Result<i8, Error> {
        match self
            .send_and_receive(&InquiryCommand::ExposureCompensation)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::ExposureCompensation { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Check if exposure compensation is enabled.
    pub async fn get_exposure_compensation_enabled(&mut self) -> Result<bool, Error> {
        match self
            .send_and_receive(&InquiryCommand::ExposureCompensationMode)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::ExposureCompensationMode { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current iris setting.
    pub async fn get_iris(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Iris).await? {
            Response::InquiryResponse(InquiryResponse::Iris { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current shutter speed.
    pub async fn get_shutter_speed(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Shutter).await? {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current brightness level.
    pub async fn get_brightness(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Bright).await? {
            Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current gain level.
    pub async fn get_gain(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Gain).await? {
            Response::InquiryResponse(InquiryResponse::Gain { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current gain limit.
    pub async fn get_gain_limit(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::GainLimit).await? {
            Response::InquiryResponse(InquiryResponse::GainLimit { limit }) => Ok(limit),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // White balance inquiries

    /// Get the current white balance mode.
    pub async fn get_white_balance_mode(&mut self) -> Result<WhiteBalanceMode, Error> {
        match self
            .send_and_receive(&InquiryCommand::WhiteBalanceMode)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::WhiteBalance { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current red gain tuning value.
    pub async fn get_red_gain(&mut self) -> Result<i8, Error> {
        match self.send_and_receive(&InquiryCommand::RedGain).await? {
            Response::InquiryResponse(InquiryResponse::RedGain { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current blue gain tuning value.
    pub async fn get_blue_gain(&mut self) -> Result<i8, Error> {
        match self.send_and_receive(&InquiryCommand::BlueGain).await? {
            Response::InquiryResponse(InquiryResponse::BlueGain { gain }) => Ok(gain),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current color temperature.
    pub async fn get_color_temperature(&mut self) -> Result<u16, Error> {
        match self
            .send_and_receive(&InquiryCommand::ColorTemperature)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::ColorTemperature { temperature }) => {
                Ok(temperature)
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Image quality inquiries

    /// Get the current luminance level.
    pub async fn get_luminance(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Luminance).await? {
            Response::InquiryResponse(InquiryResponse::Luminance(level)) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current contrast level.
    pub async fn get_contrast(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Contrast).await? {
            Response::InquiryResponse(InquiryResponse::Contrast(level)) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current sharpness level.
    pub async fn get_sharpness(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Sharpness).await? {
            Response::InquiryResponse(InquiryResponse::Sharpness { value }) => Ok(value),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current sharpness mode.
    pub async fn get_sharpness_mode(&mut self) -> Result<SharpnessMode, Error> {
        match self
            .send_and_receive(&InquiryCommand::SharpnessMode)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::SharpnessMode { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current saturation level.
    pub async fn get_saturation(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Saturation).await? {
            Response::InquiryResponse(InquiryResponse::Saturation { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current hue level.
    pub async fn get_hue(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Hue).await? {
            Response::InquiryResponse(InquiryResponse::Hue { hue }) => Ok(hue),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Image processing inquiries

    /// Check if backlight compensation is enabled.
    pub async fn get_backlight_status(&mut self) -> Result<bool, Error> {
        match self.send_and_receive(&InquiryCommand::Backlight).await? {
            Response::InquiryResponse(InquiryResponse::Backlight { status }) => Ok(status),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current image flip state.
    pub async fn get_image_flip(&mut self) -> Result<(bool, bool), Error> {
        match self.send_and_receive(&InquiryCommand::ImageFlip).await? {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok((vertical, horizontal)),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Check if black and white mode is enabled.
    pub async fn get_black_white_mode(&mut self) -> Result<bool, Error> {
        match self.send_and_receive(&InquiryCommand::BlackWhite).await? {
            Response::InquiryResponse(InquiryResponse::BlackWhite { on }) => Ok(on),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the anti-flicker mode.
    pub async fn get_anti_flicker(&mut self) -> Result<AntiFlickerMode, Error> {
        match self.send_and_receive(&InquiryCommand::AntiFlicker).await? {
            Response::InquiryResponse(InquiryResponse::AntiFlicker { mode }) => Ok(mode),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the 2D noise reduction level.
    pub async fn get_noise_reduction_2d(&mut self) -> Result<u8, Error> {
        match self
            .send_and_receive(&InquiryCommand::NoiseReduction2D)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::NoiseReduction2D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the 3D noise reduction level.
    pub async fn get_noise_reduction_3d(&mut self) -> Result<u8, Error> {
        match self
            .send_and_receive(&InquiryCommand::NoiseReduction3D)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::NoiseReduction3D { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the dynamic range level.
    pub async fn get_dynamic_range(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::DynamicRange).await? {
            Response::InquiryResponse(InquiryResponse::DynamicRange { level }) => Ok(level),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Focus control inquiries

    /// Get the focus zone setting.
    pub async fn get_focus_zone(&mut self) -> Result<FocusZone, Error> {
        match self.send_and_receive(&InquiryCommand::FocusZone).await? {
            Response::InquiryResponse(InquiryResponse::FocusZone { zone }) => Ok(zone),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the auto-focus sensitivity.
    pub async fn get_auto_focus_sensitivity(&mut self) -> Result<AutoFocusSensitivity, Error> {
        match self
            .send_and_receive(&InquiryCommand::AutoFocusSensitivity)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::AutoFocusSensitivity { sensitivity }) => {
                Ok(sensitivity)
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the focus near limit.
    pub async fn get_focus_near_limit(&mut self) -> Result<u16, Error> {
        match self
            .send_and_receive(&InquiryCommand::FocusNearLimit)
            .await?
        {
            Response::InquiryResponse(InquiryResponse::FocusNearLimit { position }) => Ok(position),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Composite queries

    /// Get a complete snapshot of the camera state.
    ///
    /// This method queries multiple camera parameters and returns a comprehensive
    /// state object. Note that this performs multiple inquiries and may take some time.
    pub async fn get_camera_state(&mut self) -> Result<CameraState, Error> {
        // Get power state
        let power = self.get_power_state().await?;

        // Get position
        let (pan, tilt) = match self.get_position_units().await {
            Ok((p, t)) => (p.0, t.0),
            Err(_) => (0, 0), // Default if position query fails
        };

        let pan_degrees = self.profile.pan_units_to_degrees(pan);
        let tilt_degrees = self.profile.tilt_units_to_degrees(tilt);

        let position = Position {
            pan,
            tilt,
            pan_degrees,
            tilt_degrees,
        };

        // Get optics
        let zoom = self.get_zoom_position().await.unwrap_or(0);
        let focus = self.get_focus_position().await.unwrap_or(0);

        let optics = Optics { zoom, focus };

        // Get exposure settings
        let exposure_mode = self.get_exposure_mode().await.unwrap_or(ExposureMode::Auto);
        let compensation = self.get_exposure_compensation().await.ok();
        let iris = self.get_iris().await.ok();
        let shutter = self.get_shutter_speed().await.ok();
        let bright = self.get_brightness().await.ok();
        let gain = self.get_gain().await.ok();

        let exposure = Exposure {
            mode: exposure_mode,
            compensation,
            iris,
            shutter,
            bright,
            gain,
        };

        // Get white balance
        let wb_mode = self
            .get_white_balance_mode()
            .await
            .unwrap_or(WhiteBalanceMode::Auto);
        let red_gain = self.get_red_gain().await.ok();
        let blue_gain = self.get_blue_gain().await.ok();

        let white_balance = WhiteBalance {
            mode: wb_mode,
            red_gain,
            blue_gain,
        };

        // Get image settings
        let luminance = self.get_luminance().await.unwrap_or(7);
        let contrast = self.get_contrast().await.unwrap_or(7);
        let sharpness = self.get_sharpness().await.unwrap_or(5);
        let saturation = self.get_saturation().await.unwrap_or(7);
        let hue = self.get_hue().await.unwrap_or(7);

        let image = ImageSettings {
            luminance,
            contrast,
            sharpness,
            saturation,
            hue,
        };

        Ok(CameraState {
            power,
            position,
            optics,
            exposure,
            white_balance,
            image,
        })
    }
}
