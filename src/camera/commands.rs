//! Type-safe command methods for Camera.

// Common imports for both async and blocking
#[cfg(any(
    feature = "async-client",
    all(feature = "blocking-client", not(feature = "async-client"))
))]
use crate::{
    command::{
        color::{
            BlueGainCommand, BlueTuningCommand, ColorTemperatureCommand, HueCommand,
            OnePushTriggerCommand, RedGainCommand, RedTuningCommand, SaturationCommand,
        },
        exposure::{
            BrightCommand, DynamicRangeCommand, DynamicRangeLevel, ExposureCommand,
            ExposureCompensationCommand, ExposureCompensationLevel, ExposureMode, IrisCommand,
            ShutterCommand,
        },
        flip::{Flip, ImageFlipCommand},
        focus::FocusCommand,
        gain::{AntiFlickerCommand, AntiFlickerMode, GainCommand, GainLimitCommand},
        image::{
            BacklightCommand, BlackWhiteCommand, ImageFlipCombinedCommand, ImageFlipMode,
            NoiseReduction2DCommand, NoiseReduction3DCommand,
        },
        luminance_contrast_sharpness::{
            ContrastCommand, LuminanceCommand, SharpnessCommand, SharpnessMode,
        },
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand, PresetNumber},
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        zoom::ZoomCommand,
    },
    error::Error,
    types::{
        BrightnessLevel, ContrastLevel, GainLimit, GainValue, IrisLevel, LuminanceLevel,
        NoiseReduction2DLevel, NoiseReduction3DLevel, ShutterSpeed,
    },
};

// No longer need async-only imports since they're now common

#[cfg(any(
    feature = "async-client",
    all(feature = "blocking-client", not(feature = "async-client"))
))]
use super::{
    units::{Degrees, Normalized, ViscaUnits},
    Camera, CameraProfile,
};

#[cfg(feature = "async-client")]
impl<P: CameraProfile> Camera<P> {
    /// Power on the camera.
    pub async fn power_on(&self) -> Result<(), Error> {
        let command = PowerCommand { power: Power::On };
        self.send_and_wait(&command).await
    }

    /// Power off the camera.
    pub async fn power_off(&self) -> Result<(), Error> {
        let command = PowerCommand {
            power: Power::Standby,
        };
        self.send_and_wait(&command).await
    }

    /// Set the camera to an absolute pan/tilt position in degrees.
    pub async fn set_position(&self, pan: Degrees<f32>, tilt: Degrees<f32>) -> Result<(), Error> {
        // Convert degrees to VISCA units using the camera profile
        let pan_units = self.profile.pan_degrees_to_units(pan.0);
        let tilt_units = self.profile.tilt_degrees_to_units(tilt.0);

        // Validate ranges
        if !P::PAN_RANGE.contains(&pan_units) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: pan_units as i32,
                min: *P::PAN_RANGE.start() as i32,
                max: *P::PAN_RANGE.end() as i32,
            });
        }

        if !P::TILT_RANGE.contains(&tilt_units) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: tilt_units as i32,
                min: *P::TILT_RANGE.start() as i32,
                max: *P::TILT_RANGE.end() as i32,
            });
        }

        // Create and send the absolute position command
        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?, // Use half speed for smooth movement
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: pan_units,
            tilt: tilt_units,
        };

        self.send_and_wait(&command).await
    }

    /// Set the camera to an absolute position using VISCA units.
    pub async fn set_position_units(
        &self,
        pan: ViscaUnits<i16>,
        tilt: ViscaUnits<i16>,
    ) -> Result<(), Error> {
        // Validate ranges
        if !P::PAN_RANGE.contains(&pan.0) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: pan.0 as i32,
                min: *P::PAN_RANGE.start() as i32,
                max: *P::PAN_RANGE.end() as i32,
            });
        }

        if !P::TILT_RANGE.contains(&tilt.0) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: tilt.0 as i32,
                min: *P::TILT_RANGE.start() as i32,
                max: *P::TILT_RANGE.end() as i32,
            });
        }

        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?,
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: pan.0,
            tilt: tilt.0,
        };

        self.send_and_wait(&command).await
    }

    /// Set the camera position using normalized coordinates (-1.0 to 1.0).
    pub async fn set_position_normalized(
        &self,
        pan: Normalized<f32>,
        tilt: Normalized<f32>,
    ) -> Result<(), Error> {
        // Clamp normalized values to -1.0 to 1.0
        let pan_norm = pan.0.clamp(-1.0, 1.0);
        let tilt_norm = tilt.0.clamp(-1.0, 1.0);

        // Convert normalized to VISCA units
        let pan_range = P::PAN_RANGE.end() - P::PAN_RANGE.start();
        let pan_units = (pan_norm * pan_range as f32 / 2.0) as i16;

        let tilt_range = P::TILT_RANGE.end() - P::TILT_RANGE.start();
        let tilt_units = (tilt_norm * tilt_range as f32 / 2.0) as i16;

        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?,
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: pan_units,
            tilt: tilt_units,
        };

        self.send_and_wait(&command).await
    }

    /// Move the camera continuously in a direction.
    pub async fn move_continuous(
        &self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error> {
        // Ensure speeds are within valid range - 0 is always valid
        let safe_pan_speed = pan_speed.min(P::MAX_PAN_SPEED);
        let safe_tilt_speed = tilt_speed.min(P::MAX_TILT_SPEED);

        let command = PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::new(safe_pan_speed).map_err(|_| {
                Error::InvalidParameter(format!("Invalid pan speed: {}", safe_pan_speed))
            })?,
            tilt_speed: TiltSpeed::new(safe_tilt_speed).map_err(|_| {
                Error::InvalidParameter(format!("Invalid tilt speed: {}", safe_tilt_speed))
            })?,
        };

        self.send_and_wait(&command).await
    }

    /// Stop all camera movement.
    pub async fn stop(&self) -> Result<(), Error> {
        let command = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)
                .map_err(|_| Error::InvalidParameter("Invalid pan speed: 0".to_string()))?,
            tilt_speed: TiltSpeed::new(0)
                .map_err(|_| Error::InvalidParameter("Invalid tilt speed: 0".to_string()))?,
        };
        self.send_and_wait(&command).await
    }

    /// Move camera to home position.
    pub async fn home(&self) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::Home).await
    }

    /// Recall a preset position.
    pub async fn recall_preset(&self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number,
        };
        self.send_and_wait(&command).await
    }

    /// Set a preset position.
    pub async fn set_preset(&self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number,
        };
        self.send_and_wait(&command).await
    }

    /// Clear a preset position.
    pub async fn clear_preset(&self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number,
        };
        self.send_and_wait(&command).await
    }

    /// Zoom in at standard speed.
    pub async fn zoom_in(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomInStandard).await
    }

    /// Zoom out at standard speed.
    pub async fn zoom_out(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomOutStandard).await
    }

    /// Stop zooming.
    pub async fn zoom_stop(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::Stop).await
    }

    /// Set zoom to direct position.
    pub async fn set_zoom(&self, position: u16) -> Result<(), Error> {
        if !P::ZOOM_RANGE.contains(&position) {
            return Err(Error::ParameterOutOfRange {
                parameter: "zoom".to_string(),
                value: position as i32,
                min: *P::ZOOM_RANGE.start() as i32,
                max: *P::ZOOM_RANGE.end() as i32,
            });
        }

        self.send_and_wait(&ZoomCommand::Direct(position)).await
    }

    /// Set focus mode to auto.
    pub async fn focus_auto(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Auto).await
    }

    /// Set focus mode to manual.
    pub async fn focus_manual(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Manual).await
    }

    /// Set focus to direct position (manual mode).
    pub async fn set_focus(&self, position: u16) -> Result<(), Error> {
        if !P::FOCUS_RANGE.contains(&position) {
            return Err(Error::ParameterOutOfRange {
                parameter: "focus".to_string(),
                value: position as i32,
                min: *P::FOCUS_RANGE.start() as i32,
                max: *P::FOCUS_RANGE.end() as i32,
            });
        }

        self.send_and_wait(&FocusCommand::Direct(position)).await
    }

    /// Set exposure mode.
    pub async fn set_exposure_mode(&self, mode: ExposureMode) -> Result<(), Error> {
        let command = ExposureCommand { mode };
        self.send_and_wait(&command).await
    }

    /// Set white balance mode.
    pub async fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error> {
        let command = WhiteBalanceCommand { mode };
        self.send_and_wait(&command).await
    }

    /// Set gain value.
    pub async fn set_gain(&self, gain: P::GainValue) -> Result<(), Error> {
        let value: u8 = gain.into();
        let gain_value = GainValue::new(value)?;
        self.send_and_wait(&GainCommand::Direct(gain_value)).await
    }

    // Exposure Compensation Methods

    /// Enable exposure compensation.
    pub async fn exposure_compensation_on(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::On).await
    }

    /// Disable exposure compensation.
    pub async fn exposure_compensation_off(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Off).await
    }

    /// Reset exposure compensation to 0.
    pub async fn exposure_compensation_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Reset)
            .await
    }

    /// Increase exposure compensation by one step.
    pub async fn exposure_compensation_up(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Up).await
    }

    /// Decrease exposure compensation by one step.
    pub async fn exposure_compensation_down(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Down).await
    }

    /// Set exposure compensation level directly (-7 to +7).
    pub async fn set_exposure_compensation(&self, level: i8) -> Result<(), Error> {
        let comp_level = ExposureCompensationLevel::new(level)?;
        self.send_and_wait(&ExposureCompensationCommand::Direct(comp_level))
            .await
    }

    // Dynamic Range Methods

    /// Set dynamic range level (0-8).
    pub async fn set_dynamic_range(&self, level: u8) -> Result<(), Error> {
        let dr_level = DynamicRangeLevel::new(level)?;
        self.send_and_wait(&DynamicRangeCommand::Direct(dr_level))
            .await
    }

    // Iris Methods

    /// Reset iris to default.
    pub async fn iris_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Reset).await
    }

    /// Increase iris opening by one step.
    pub async fn iris_up(&self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Up).await
    }

    /// Decrease iris opening by one step.
    pub async fn iris_down(&self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Down).await
    }

    /// Set iris level directly.
    pub async fn set_iris(&self, level: u8) -> Result<(), Error> {
        let iris_level = IrisLevel::new(level)?;
        self.send_and_wait(&IrisCommand::Direct(iris_level)).await
    }

    // Shutter Methods

    /// Reset shutter speed to default.
    pub async fn shutter_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Reset).await
    }

    /// Increase shutter speed by one step.
    pub async fn shutter_up(&self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Up).await
    }

    /// Decrease shutter speed by one step.
    pub async fn shutter_down(&self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Down).await
    }

    /// Set shutter speed directly.
    pub async fn set_shutter(&self, speed: u16) -> Result<(), Error> {
        let shutter_speed = ShutterSpeed::new(speed)?;
        self.send_and_wait(&ShutterCommand::Direct(shutter_speed))
            .await
    }

    // Brightness Methods

    /// Reset brightness to default.
    pub async fn brightness_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Reset).await
    }

    /// Increase brightness by one step.
    pub async fn brightness_up(&self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Up).await
    }

    /// Decrease brightness by one step.
    pub async fn brightness_down(&self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Down).await
    }

    /// Set brightness level directly.
    pub async fn set_brightness(&self, level: u16) -> Result<(), Error> {
        let brightness_level = BrightnessLevel::new(level)?;
        self.send_and_wait(&BrightCommand::Direct(brightness_level))
            .await
    }

    // Gain Methods (additional)

    /// Reset gain to default.
    pub async fn gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Reset).await
    }

    /// Increase gain by one step.
    pub async fn gain_up(&self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Up).await
    }

    /// Decrease gain by one step.
    pub async fn gain_down(&self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Down).await
    }

    /// Set gain limit.
    pub async fn set_gain_limit(&self, limit: u8) -> Result<(), Error> {
        let gain_limit = GainLimit::new(limit)?;
        self.send_and_wait(&GainLimitCommand { limit: gain_limit })
            .await
    }

    /// Set anti-flicker mode.
    pub async fn set_anti_flicker(&self, mode: AntiFlickerMode) -> Result<(), Error> {
        self.send_and_wait(&AntiFlickerCommand { mode }).await
    }

    // Image Adjustment Methods

    /// Enable backlight compensation.
    pub async fn backlight_on(&self) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: true }).await
    }

    /// Disable backlight compensation.
    pub async fn backlight_off(&self) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: false })
            .await
    }

    /// Disable 2D noise reduction.
    pub async fn noise_reduction_2d_off(&self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction2DCommand::Off).await
    }

    /// Set 2D noise reduction level (1-5).
    pub async fn set_noise_reduction_2d(&self, level: u8) -> Result<(), Error> {
        let nr_level = NoiseReduction2DLevel::new(level)?;
        self.send_and_wait(&NoiseReduction2DCommand::Level(nr_level))
            .await
    }

    /// Disable 3D noise reduction.
    pub async fn noise_reduction_3d_off(&self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction3DCommand::Off).await
    }

    /// Set 3D noise reduction level (1-5).
    pub async fn set_noise_reduction_3d(&self, level: u8) -> Result<(), Error> {
        let nr_level = NoiseReduction3DLevel::new(level)?;
        self.send_and_wait(&NoiseReduction3DCommand::Level(nr_level))
            .await
    }

    /// Enable black and white mode.
    pub async fn black_white_on(&self) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: true }).await
    }

    /// Disable black and white mode (color mode).
    pub async fn black_white_off(&self) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: false }).await
    }

    /// Set image flip mode.
    pub async fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCombinedCommand { mode }).await
    }

    // Flip Methods (Simple vertical flip)

    /// Enable image flip (vertical).
    pub async fn flip_on(&self) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: Flip::On })
            .await
    }

    /// Disable image flip (vertical).
    pub async fn flip_off(&self) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: Flip::Off })
            .await
    }

    // Color Adjustment Methods

    /// Trigger one-push white balance adjustment.
    pub async fn one_push_white_balance(&self) -> Result<(), Error> {
        self.send_and_wait(&OnePushTriggerCommand).await
    }

    /// Set red tuning level (-10 to +10).
    pub async fn set_red_tuning(&self, level: i8) -> Result<(), Error> {
        self.send_and_wait(&RedTuningCommand { level }).await
    }

    /// Set blue tuning level (-10 to +10).
    pub async fn set_blue_tuning(&self, level: i8) -> Result<(), Error> {
        self.send_and_wait(&BlueTuningCommand { level }).await
    }

    /// Set saturation level (0x0 = 60%, 0xE = 200%).
    pub async fn set_saturation(&self, level: u8) -> Result<(), Error> {
        self.send_and_wait(&SaturationCommand { level }).await
    }

    /// Set hue level (0x0 to 0xE).
    pub async fn set_hue(&self, level: u8) -> Result<(), Error> {
        self.send_and_wait(&HueCommand { level }).await
    }

    /// Reset color temperature to default.
    pub async fn color_temperature_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Reset).await
    }

    /// Increase color temperature (cooler/bluer).
    pub async fn color_temperature_up(&self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Up).await
    }

    /// Decrease color temperature (warmer/redder).
    pub async fn color_temperature_down(&self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Down).await
    }

    /// Set color temperature directly (0x00 = 2500K to 0x37 = 8000K).
    pub async fn set_color_temperature(&self, temp: u16) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Direct(temp))
            .await
    }

    /// Reset red gain to default.
    pub async fn red_gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Reset).await
    }

    /// Increase red gain by one step.
    pub async fn red_gain_up(&self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Up).await
    }

    /// Decrease red gain by one step.
    pub async fn red_gain_down(&self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Down).await
    }

    /// Set red gain directly.
    pub async fn set_red_gain(&self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Direct(value)).await
    }

    /// Reset blue gain to default.
    pub async fn blue_gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Reset).await
    }

    /// Increase blue gain by one step.
    pub async fn blue_gain_up(&self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Up).await
    }

    /// Decrease blue gain by one step.
    pub async fn blue_gain_down(&self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Down).await
    }

    /// Set blue gain directly.
    pub async fn set_blue_gain(&self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Direct(value)).await
    }

    // Luminance, Contrast, and Sharpness Methods

    /// Set sharpness mode.
    pub async fn set_sharpness_mode(&self, mode: SharpnessMode) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Mode(mode)).await
    }

    /// Reset sharpness to default.
    pub async fn sharpness_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Reset).await
    }

    /// Increase sharpness by one step.
    pub async fn sharpness_up(&self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Up).await
    }

    /// Decrease sharpness by one step.
    pub async fn sharpness_down(&self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Down).await
    }

    /// Set sharpness directly (0-11).
    pub async fn set_sharpness(&self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Direct { value })
            .await
    }

    /// Set luminance level.
    pub async fn set_luminance(&self, level: u8) -> Result<(), Error> {
        let luminance_level = LuminanceLevel::new(level)?;
        self.send_and_wait(&LuminanceCommand {
            value: luminance_level,
        })
        .await
    }

    /// Set contrast level.
    pub async fn set_contrast(&self, level: u8) -> Result<(), Error> {
        let contrast_level = ContrastLevel::new(level)?;
        self.send_and_wait(&ContrastCommand {
            value: contrast_level,
        })
        .await
    }
}

// Blocking implementations
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
impl<P: CameraProfile> Camera<P> {
    /// Power on the camera.
    pub fn power_on(&mut self) -> Result<(), Error> {
        let command = PowerCommand { power: Power::On };
        self.send_and_wait(&command)
    }

    /// Power off the camera.
    pub fn power_off(&mut self) -> Result<(), Error> {
        let command = PowerCommand {
            power: Power::Standby,
        };
        self.send_and_wait(&command)
    }

    /// Stop all camera movement.
    pub fn stop(&mut self) -> Result<(), Error> {
        let command = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)
                .map_err(|_| Error::InvalidParameter("Invalid pan speed: 0".to_string()))?,
            tilt_speed: TiltSpeed::new(0)
                .map_err(|_| Error::InvalidParameter("Invalid tilt speed: 0".to_string()))?,
        };
        self.send_and_wait(&command)
    }

    /// Move camera to home position.
    pub fn home(&mut self) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::Home)
    }

    /// Zoom in at standard speed.
    pub fn zoom_in(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomInStandard)
    }

    /// Zoom out at standard speed.
    pub fn zoom_out(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomOutStandard)
    }

    /// Stop zooming.
    pub fn zoom_stop(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::Stop)
    }

    /// Set zoom to direct position.
    pub fn set_zoom(&mut self, position: u16) -> Result<(), Error> {
        if !P::ZOOM_RANGE.contains(&position) {
            return Err(Error::ParameterOutOfRange {
                parameter: "zoom".to_string(),
                value: position as i32,
                min: *P::ZOOM_RANGE.start() as i32,
                max: *P::ZOOM_RANGE.end() as i32,
            });
        }

        self.send_and_wait(&ZoomCommand::Direct(position))
    }

    /// Set focus mode to auto.
    pub fn focus_auto(&mut self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Auto)
    }

    /// Set focus mode to manual.
    pub fn focus_manual(&mut self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Manual)
    }

    /// Set focus to direct position (manual mode).
    pub fn set_focus(&mut self, position: u16) -> Result<(), Error> {
        if !P::FOCUS_RANGE.contains(&position) {
            return Err(Error::ParameterOutOfRange {
                parameter: "focus".to_string(),
                value: position as i32,
                min: *P::FOCUS_RANGE.start() as i32,
                max: *P::FOCUS_RANGE.end() as i32,
            });
        }

        self.send_and_wait(&FocusCommand::Direct(position))
    }

    /// Set the camera to an absolute pan/tilt position in degrees.
    pub fn set_position(&mut self, pan: Degrees<f32>, tilt: Degrees<f32>) -> Result<(), Error> {
        // Convert degrees to VISCA units using the camera profile
        let pan_units = self.profile.pan_degrees_to_units(pan.0);
        let tilt_units = self.profile.tilt_degrees_to_units(tilt.0);

        // Validate ranges
        if !P::PAN_RANGE.contains(&pan_units) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: pan_units as i32,
                min: *P::PAN_RANGE.start() as i32,
                max: *P::PAN_RANGE.end() as i32,
            });
        }

        if !P::TILT_RANGE.contains(&tilt_units) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: tilt_units as i32,
                min: *P::TILT_RANGE.start() as i32,
                max: *P::TILT_RANGE.end() as i32,
            });
        }

        // Create and send the absolute position command
        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?, // Use half speed for smooth movement
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: pan_units,
            tilt: tilt_units,
        };

        self.send_and_wait(&command)
    }

    /// Move the camera continuously in a direction.
    pub fn move_continuous(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error> {
        // Ensure speeds are within valid range - 0 is always valid
        let safe_pan_speed = pan_speed.min(P::MAX_PAN_SPEED);
        let safe_tilt_speed = tilt_speed.min(P::MAX_TILT_SPEED);

        let command = PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::new(safe_pan_speed).map_err(|_| {
                Error::InvalidParameter(format!("Invalid pan speed: {}", safe_pan_speed))
            })?,
            tilt_speed: TiltSpeed::new(safe_tilt_speed).map_err(|_| {
                Error::InvalidParameter(format!("Invalid tilt speed: {}", safe_tilt_speed))
            })?,
        };

        self.send_and_wait(&command)
    }

    /// Recall a preset position.
    pub fn recall_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number,
        };
        self.send_and_wait(&command)
    }

    /// Set a preset position.
    pub fn set_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number,
        };
        self.send_and_wait(&command)
    }

    /// Set exposure mode.
    pub fn set_exposure_mode(&mut self, mode: ExposureMode) -> Result<(), Error> {
        let command = ExposureCommand { mode };
        self.send_and_wait(&command)
    }

    /// Set white balance mode.
    pub fn set_white_balance_mode(&mut self, mode: WhiteBalanceMode) -> Result<(), Error> {
        let command = WhiteBalanceCommand { mode };
        self.send_and_wait(&command)
    }

    /// Set gain value.
    pub fn set_gain(&mut self, gain: P::GainValue) -> Result<(), Error> {
        let value: u8 = gain.into();
        let gain_value = GainValue::new(value)?;
        self.send_and_wait(&GainCommand::Direct(gain_value))
    }

    // Position control methods
    /// Set the camera to an absolute pan/tilt position in VISCA units.
    pub fn set_position_units(
        &mut self,
        pan: ViscaUnits<i16>,
        tilt: ViscaUnits<i16>,
    ) -> Result<(), Error> {
        // Validate ranges
        if !P::PAN_RANGE.contains(&pan.0) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: pan.0 as i32,
                min: *P::PAN_RANGE.start() as i32,
                max: *P::PAN_RANGE.end() as i32,
            });
        }

        if !P::TILT_RANGE.contains(&tilt.0) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: tilt.0 as i32,
                min: *P::TILT_RANGE.start() as i32,
                max: *P::TILT_RANGE.end() as i32,
            });
        }

        let command = PanTiltCommand::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?,
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: pan.0,
            tilt: tilt.0,
        };

        self.send_and_wait(&command)
    }

    /// Set the camera to a normalized position (-1.0 to 1.0).
    pub fn set_position_normalized(
        &mut self,
        pan: Normalized<f32>,
        tilt: Normalized<f32>,
    ) -> Result<(), Error> {
        // Convert normalized (-1.0 to 1.0) to VISCA units using camera profile ranges
        let pan_range = P::PAN_RANGE;
        let tilt_range = P::TILT_RANGE;

        // Map normalized value to unit range
        let pan_units = ((pan.0 + 1.0) / 2.0 * (pan_range.end() - pan_range.start()) as f32
            + *pan_range.start() as f32) as i16;
        let tilt_units = ((tilt.0 + 1.0) / 2.0 * (tilt_range.end() - tilt_range.start()) as f32
            + *tilt_range.start() as f32) as i16;

        self.set_position_units(ViscaUnits(pan_units), ViscaUnits(tilt_units))
    }

    // Preset methods
    /// Clear a preset position.
    pub fn clear_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number,
        };
        self.send_and_wait(&command)
    }

    // Exposure control methods
    /// Turn on exposure compensation.
    pub fn exposure_compensation_on(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::On)
    }

    /// Turn off exposure compensation.
    pub fn exposure_compensation_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Off)
    }

    /// Reset exposure compensation.
    pub fn exposure_compensation_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Reset)
    }

    /// Increase exposure compensation.
    pub fn exposure_compensation_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Up)
    }

    /// Decrease exposure compensation.
    pub fn exposure_compensation_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Down)
    }

    /// Set exposure compensation level.
    pub fn set_exposure_compensation(
        &mut self,
        level: ExposureCompensationLevel,
    ) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Direct(level))
    }

    /// Set dynamic range level.
    pub fn set_dynamic_range(&mut self, level: DynamicRangeLevel) -> Result<(), Error> {
        self.send_and_wait(&DynamicRangeCommand::Direct(level))
    }

    // Iris control methods
    /// Reset iris value.
    pub fn iris_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Reset)
    }

    /// Increase iris opening.
    pub fn iris_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Up)
    }

    /// Decrease iris opening.
    pub fn iris_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Down)
    }

    /// Set iris level directly.
    pub fn set_iris(&mut self, level: IrisLevel) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Direct(level))
    }

    // Shutter control methods
    /// Reset shutter speed.
    pub fn shutter_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Reset)
    }

    /// Increase shutter speed.
    pub fn shutter_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Up)
    }

    /// Decrease shutter speed.
    pub fn shutter_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Down)
    }

    /// Set shutter speed directly.
    pub fn set_shutter(&mut self, speed: ShutterSpeed) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Direct(speed))
    }

    // Brightness control methods
    /// Reset brightness.
    pub fn brightness_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Reset)
    }

    /// Increase brightness.
    pub fn brightness_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Up)
    }

    /// Decrease brightness.
    pub fn brightness_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Down)
    }

    /// Set brightness level directly.
    pub fn set_brightness(&mut self, level: BrightnessLevel) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Direct(level))
    }

    // Gain control methods
    /// Reset gain.
    pub fn gain_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Reset)
    }

    /// Increase gain.
    pub fn gain_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Up)
    }

    /// Decrease gain.
    pub fn gain_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Down)
    }

    /// Set gain limit.
    pub fn set_gain_limit(&mut self, limit: GainLimit) -> Result<(), Error> {
        self.send_and_wait(&GainLimitCommand { limit })
    }

    /// Set anti-flicker mode.
    pub fn set_anti_flicker(&mut self, mode: AntiFlickerMode) -> Result<(), Error> {
        self.send_and_wait(&AntiFlickerCommand { mode })
    }

    // White balance methods
    /// Trigger one-push white balance adjustment.
    pub fn one_push_white_balance(&mut self) -> Result<(), Error> {
        self.send_and_wait(&OnePushTriggerCommand)
    }

    // Color adjustment methods
    /// Set red tuning level.
    pub fn set_red_tuning(&mut self, level: i8) -> Result<(), Error> {
        self.send_and_wait(&RedTuningCommand { level })
    }

    /// Set blue tuning level.
    pub fn set_blue_tuning(&mut self, level: i8) -> Result<(), Error> {
        self.send_and_wait(&BlueTuningCommand { level })
    }

    /// Set saturation level.
    pub fn set_saturation(&mut self, level: u8) -> Result<(), Error> {
        self.send_and_wait(&SaturationCommand { level })
    }

    /// Set hue level.
    pub fn set_hue(&mut self, level: u8) -> Result<(), Error> {
        self.send_and_wait(&HueCommand { level })
    }

    /// Reset color temperature.
    pub fn color_temperature_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Reset)
    }

    /// Increase color temperature.
    pub fn color_temperature_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Up)
    }

    /// Decrease color temperature.
    pub fn color_temperature_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Down)
    }

    /// Set color temperature directly.
    pub fn set_color_temperature(&mut self, temperature: u16) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Direct(temperature))
    }

    /// Reset red gain.
    pub fn red_gain_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Reset)
    }

    /// Increase red gain.
    pub fn red_gain_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Up)
    }

    /// Decrease red gain.
    pub fn red_gain_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Down)
    }

    /// Set red gain directly.
    pub fn set_red_gain(&mut self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Direct(value))
    }

    /// Reset blue gain.
    pub fn blue_gain_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Reset)
    }

    /// Increase blue gain.
    pub fn blue_gain_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Up)
    }

    /// Decrease blue gain.
    pub fn blue_gain_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Down)
    }

    /// Set blue gain directly.
    pub fn set_blue_gain(&mut self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Direct(value))
    }

    // Image adjustment methods
    /// Turn backlight compensation on.
    pub fn backlight_on(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: true })
    }

    /// Turn backlight compensation off.
    pub fn backlight_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: false })
    }

    /// Turn off 2D noise reduction.
    pub fn noise_reduction_2d_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction2DCommand::Off)
    }

    /// Set 2D noise reduction level.
    pub fn set_noise_reduction_2d(&mut self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction2DCommand::Level(level))
    }

    /// Turn off 3D noise reduction.
    pub fn noise_reduction_3d_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction3DCommand::Off)
    }

    /// Set 3D noise reduction level.
    pub fn set_noise_reduction_3d(&mut self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction3DCommand::Level(level))
    }

    /// Turn on black and white mode.
    pub fn black_white_on(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: true })
    }

    /// Turn off black and white mode.
    pub fn black_white_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: false })
    }

    // Flip control methods
    /// Turn on image flip.
    pub fn flip_on(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: Flip::On })
    }

    /// Turn off image flip.
    pub fn flip_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: Flip::Off })
    }

    /// Set image flip mode.
    pub fn set_image_flip(&mut self, mode: ImageFlipMode) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCombinedCommand { mode })
    }

    // Luminance, contrast, and sharpness methods
    /// Set sharpness mode.
    pub fn set_sharpness_mode(&mut self, mode: SharpnessMode) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Mode(mode))
    }

    /// Reset sharpness.
    pub fn sharpness_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Reset)
    }

    /// Increase sharpness.
    pub fn sharpness_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Up)
    }

    /// Decrease sharpness.
    pub fn sharpness_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Down)
    }

    /// Set sharpness value directly.
    pub fn set_sharpness(&mut self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Direct { value })
    }

    /// Set luminance level.
    pub fn set_luminance(&mut self, level: u8) -> Result<(), Error> {
        let luminance_level = LuminanceLevel::new(level)?;
        self.send_and_wait(&LuminanceCommand {
            value: luminance_level,
        })
    }

    /// Set contrast level.
    pub fn set_contrast(&mut self, level: u8) -> Result<(), Error> {
        let contrast_level = ContrastLevel::new(level)?;
        self.send_and_wait(&ContrastCommand {
            value: contrast_level,
        })
    }
}
