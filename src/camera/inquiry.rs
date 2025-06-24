//! Inquiry methods for `Camera<P>`.

use crate::{
    command::{
        exposure::ExposureMode,
        focus::{AutoFocusSensitivity, FocusZone},
        gain::AntiFlickerMode,
        image_adjustment::SharpnessMode,
        inquiry::InquiryCommand,
        white_balance::WhiteBalanceMode,
    },
    units::Degrees,
    visca_inquiry, Command, Error, InquiryResponse, Response, ViscaUnits,
};

use super::{Camera, CameraProfile};

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

#[cfg(feature = "async")]
impl<P: CameraProfile, T> Camera<P, T>
where
    T: crate::transport::AsyncTransport,
{
    /// Send a command and wait for the response.
    ///
    /// This method is used internally for inquiry commands that need to receive data back.
    #[cfg(feature = "async")]
    async fn send_and_receive(&self, command: &dyn Command) -> Result<Response, Error> {
        self.send_command(command).await
    }

    // Power inquiries

    /// Get the current power state of the camera.
    #[visca_inquiry]
    pub async fn get_power_state(&self) -> Result<bool, Error> {
        InquiryCommand::Power
    }

    // Position inquiries

    /// Get the current pan/tilt position in degrees.
    #[cfg(feature = "async")]
    pub async fn get_position(&self) -> Result<(Degrees<f32>, Degrees<f32>), Error> {
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
    #[cfg(feature = "async")]
    pub async fn get_position_units(&self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error> {
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
    #[visca_inquiry]
    pub async fn get_zoom_position(&self) -> Result<u16, Error> {
        InquiryCommand::ZoomPosition
    }

    /// Get the current focus position.
    #[visca_inquiry]
    pub async fn get_focus_position(&self) -> Result<u16, Error> {
        InquiryCommand::FocusPosition
    }

    // Exposure inquiries

    /// Get the current exposure mode.
    #[visca_inquiry]
    pub async fn get_exposure_mode(&self) -> Result<ExposureMode, Error> {
        InquiryCommand::ExposureMode
    }

    /// Get the current exposure compensation value.
    #[visca_inquiry]
    pub async fn get_exposure_compensation(&self) -> Result<i8, Error> {
        InquiryCommand::ExposureCompensation
    }

    /// Check if exposure compensation is enabled.
    #[visca_inquiry]
    pub async fn get_exposure_compensation_enabled(&self) -> Result<bool, Error> {
        InquiryCommand::ExposureCompensationMode
    }

    /// Get the current iris setting.
    #[visca_inquiry]
    pub async fn get_iris(&self) -> Result<u8, Error> {
        InquiryCommand::Iris
    }

    /// Get the current shutter speed.
    #[cfg(feature = "async")]
    pub async fn get_shutter_speed(&self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Shutter).await? {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current brightness level.
    #[cfg(feature = "async")]
    pub async fn get_brightness(&self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Bright).await? {
            Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current gain.
    #[visca_inquiry]
    pub async fn get_gain(&self) -> Result<u8, Error> {
        InquiryCommand::Gain
    }

    /// Get the current gain limit.
    #[visca_inquiry]
    pub async fn get_gain_limit(&self) -> Result<u8, Error> {
        InquiryCommand::GainLimit
    }

    /// Get the current iris position.
    #[visca_inquiry]
    pub async fn get_iris_position(&self) -> Result<u8, Error> {
        InquiryCommand::Iris
    }

    // White balance inquiries

    /// Get the current white balance mode.
    #[visca_inquiry]
    pub async fn get_white_balance_mode(&self) -> Result<WhiteBalanceMode, Error> {
        InquiryCommand::WhiteBalanceMode
    }

    /// Get the current red gain tuning value.
    #[visca_inquiry]
    pub async fn get_red_gain(&self) -> Result<i8, Error> {
        InquiryCommand::RedGain
    }

    /// Get the current blue gain tuning value.
    #[visca_inquiry]
    pub async fn get_blue_gain(&self) -> Result<i8, Error> {
        InquiryCommand::BlueGain
    }

    /// Get the current color temperature.
    #[visca_inquiry]
    pub async fn get_color_temperature(&self) -> Result<u16, Error> {
        InquiryCommand::ColorTemperature
    }

    // Image quality inquiries

    /// Get the current luminance level.
    #[visca_inquiry]
    pub async fn get_luminance(&self) -> Result<u8, Error> {
        InquiryCommand::Luminance
    }

    /// Get the current contrast level.
    #[visca_inquiry]
    pub async fn get_contrast(&self) -> Result<u8, Error> {
        InquiryCommand::Contrast
    }

    /// Get the current sharpness level.
    #[visca_inquiry]
    pub async fn get_sharpness(&self) -> Result<u8, Error> {
        InquiryCommand::Sharpness
    }

    /// Get the current sharpness mode.
    #[visca_inquiry]
    pub async fn get_sharpness_mode(&self) -> Result<SharpnessMode, Error> {
        InquiryCommand::SharpnessMode
    }

    /// Get the current saturation level.
    #[visca_inquiry]
    pub async fn get_saturation(&self) -> Result<u8, Error> {
        InquiryCommand::Saturation
    }

    /// Get the current hue level.
    #[visca_inquiry]
    pub async fn get_hue(&self) -> Result<u8, Error> {
        InquiryCommand::Hue
    }

    // Image processing inquiries

    /// Check if backlight compensation is enabled.
    #[visca_inquiry]
    pub async fn get_backlight_status(&self) -> Result<bool, Error> {
        InquiryCommand::Backlight
    }

    /// Get the current image flip state.
    #[cfg(feature = "async")]
    pub async fn get_image_flip(&self) -> Result<(bool, bool), Error> {
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
    #[visca_inquiry]
    pub async fn get_black_white_mode(&self) -> Result<bool, Error> {
        InquiryCommand::BlackWhite
    }

    /// Get the anti-flicker mode.
    #[visca_inquiry]
    pub async fn get_anti_flicker(&self) -> Result<AntiFlickerMode, Error> {
        InquiryCommand::AntiFlicker
    }

    /// Get the 2D noise reduction level.
    #[visca_inquiry]
    pub async fn get_noise_reduction_2d(&self) -> Result<u8, Error> {
        InquiryCommand::NoiseReduction2D
    }

    /// Get the 3D noise reduction level.
    #[visca_inquiry]
    pub async fn get_noise_reduction_3d(&self) -> Result<u8, Error> {
        InquiryCommand::NoiseReduction3D
    }

    /// Get the dynamic range level.
    #[visca_inquiry]
    pub async fn get_dynamic_range(&self) -> Result<u8, Error> {
        InquiryCommand::DynamicRange
    }

    // Focus control inquiries

    /// Get the focus zone setting.
    #[visca_inquiry]
    pub async fn get_focus_zone(&self) -> Result<FocusZone, Error> {
        InquiryCommand::FocusZone
    }

    /// Get the auto-focus sensitivity.
    #[visca_inquiry]
    pub async fn get_auto_focus_sensitivity(&self) -> Result<AutoFocusSensitivity, Error> {
        InquiryCommand::AutoFocusSensitivity
    }

    /// Get the focus near limit.
    #[visca_inquiry]
    pub async fn get_focus_near_limit(&self) -> Result<u16, Error> {
        InquiryCommand::FocusNearLimit
    }

    // Composite queries

    /// Get a complete snapshot of the camera state.
    ///
    /// This method queries multiple camera parameters and returns a comprehensive
    /// state object. Note that this performs multiple inquiries and may take some time.
    #[cfg(feature = "async")]
    pub async fn get_camera_state(&self) -> Result<CameraState, Error> {
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

// Blocking implementations
#[cfg(not(feature = "async"))]
impl<P: CameraProfile, T> Camera<P, T>
where
    T: crate::transport::blocking::BlockingTransport,
{
    /// Send a command and wait for the response.
    ///
    /// This method is used internally for inquiry commands that need to receive data back.
    fn send_and_receive(&mut self, command: &dyn Command) -> Result<Response, Error> {
        self.send_command(command)
    }

    // Power inquiries

    // get_power_state is handled by the #[visca_inquiry] macro in the async block

    // Position inquiries

    /// Get the current pan/tilt position in degrees.
    pub fn get_position(&mut self) -> Result<(Degrees<f32>, Degrees<f32>), Error> {
        match self.send_and_receive(&InquiryCommand::PanTiltPosition)? {
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
    pub fn get_position_units(&mut self) -> Result<(ViscaUnits<i16>, ViscaUnits<i16>), Error> {
        match self.send_and_receive(&InquiryCommand::PanTiltPosition)? {
            Response::InquiryResponse(InquiryResponse::PanTiltPosition { pan, tilt }) => {
                Ok((ViscaUnits(pan), ViscaUnits(tilt)))
            }
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // Zoom and focus inquiries

    // get_zoom_position is handled by the #[visca_inquiry] macro in the async block
    // get_focus_position is handled by the #[visca_inquiry] macro in the async block

    // Exposure inquiries

    // The following methods are handled by the #[visca_inquiry] macro in the async block:
    // - get_exposure_mode
    // - get_exposure_compensation
    // - get_exposure_compensation_enabled
    // - get_iris
    // - get_shutter_speed (has type conversion, see below)
    // - get_brightness (has type conversion, see below)
    // - get_gain
    // - get_gain_limit
    // - get_iris_position

    /// Get the current shutter speed.
    pub fn get_shutter_speed(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Shutter)? {
            Response::InquiryResponse(InquiryResponse::Shutter { position }) => Ok(position as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    /// Get the current brightness level.
    pub fn get_brightness(&mut self) -> Result<u8, Error> {
        match self.send_and_receive(&InquiryCommand::Bright)? {
            Response::InquiryResponse(InquiryResponse::Bright { position }) => Ok(position as u8),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // White balance inquiries

    // The following methods are handled by the #[visca_inquiry] macro in the async block:
    // - get_white_balance_mode
    // - get_red_gain
    // - get_blue_gain
    // - get_color_temperature

    // Image quality inquiries

    // The following methods are handled by the #[visca_inquiry] macro in the async block:
    // - get_luminance
    // - get_contrast
    // - get_sharpness
    // - get_saturation
    // - get_hue

    /// Get the current image flip status.
    pub fn get_image_flip(&mut self) -> Result<(bool, bool), Error> {
        match self.send_and_receive(&InquiryCommand::ImageFlip)? {
            Response::InquiryResponse(InquiryResponse::ImageFlip {
                vertical,
                horizontal,
            }) => Ok((vertical, horizontal)),
            Response::Error(e) => Err(e),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    // The following methods are handled by the #[visca_inquiry] macro in the async block:
    // - get_backlight_status
    // - get_black_white_mode

    // The following methods are handled by the #[visca_inquiry] macro in the async block:
    // - get_anti_flicker
    // - get_sharpness_mode
    // - get_focus_zone
    // - get_auto_focus_sensitivity
    // - get_focus_near_limit
    // - get_noise_reduction_2d
    // - get_noise_reduction_3d
    // - get_dynamic_range

    // Composite queries

    /// Get a complete snapshot of the camera state.
    ///
    /// This method queries multiple camera parameters and returns a comprehensive
    /// state object. Note that this performs multiple inquiries and may take some time.
    pub fn get_camera_state(&mut self) -> Result<CameraState, Error> {
        // Get power state
        let power = self.get_power_state()?;

        // Get position
        let (pan, tilt) = match self.get_position_units() {
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
        let zoom = self.get_zoom_position().unwrap_or(0);
        let focus = self.get_focus_position().unwrap_or(0);

        let optics = Optics { zoom, focus };

        // Get exposure settings
        let exposure_mode = self.get_exposure_mode().unwrap_or(ExposureMode::Auto);
        let compensation = self.get_exposure_compensation().ok();
        let iris = self.get_iris().ok();
        let shutter = self.get_shutter_speed().ok();
        let bright = self.get_brightness().ok();
        let gain = self.get_gain().ok();

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
            .unwrap_or(WhiteBalanceMode::Auto);
        let red_gain = self.get_red_gain().ok();
        let blue_gain = self.get_blue_gain().ok();

        let white_balance = WhiteBalance {
            mode: wb_mode,
            red_gain,
            blue_gain,
        };

        // Get image settings
        let luminance = self.get_luminance().unwrap_or(7);
        let contrast = self.get_contrast().unwrap_or(7);
        let sharpness = self.get_sharpness().unwrap_or(5);
        let saturation = self.get_saturation().unwrap_or(7);
        let hue = self.get_hue().unwrap_or(7);

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
