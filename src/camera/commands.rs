//! Type-safe command methods for Camera.

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

use super::{
    units::{Degrees, Normalized, ViscaUnits},
    Camera, CameraProfile,
};

impl<P: CameraProfile> Camera<P> {
    /// Power on the camera.
    pub async fn power_on(&mut self) -> Result<(), Error> {
        let command = PowerCommand { power: Power::On };
        self.send_and_wait(&command).await
    }

    /// Power off the camera.
    pub async fn power_off(&mut self) -> Result<(), Error> {
        let command = PowerCommand {
            power: Power::Standby,
        };
        self.send_and_wait(&command).await
    }

    /// Set the camera to an absolute pan/tilt position in degrees.
    pub async fn set_position(
        &mut self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
    ) -> Result<(), Error> {
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

        self.send_and_wait(&command).await
    }

    /// Set the camera position using normalized coordinates (-1.0 to 1.0).
    pub async fn set_position_normalized(
        &mut self,
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

        self.send_and_wait(&command).await
    }

    /// Stop all camera movement.
    pub async fn stop(&mut self) -> Result<(), Error> {
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
    pub async fn home(&mut self) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::Home).await
    }

    /// Recall a preset position.
    pub async fn recall_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Recall,
            preset_number,
        };
        self.send_and_wait(&command).await
    }

    /// Set a preset position.
    pub async fn set_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Set,
            preset_number,
        };
        self.send_and_wait(&command).await
    }

    /// Clear a preset position.
    pub async fn clear_preset(&mut self, preset: P::PresetId) -> Result<(), Error> {
        let id: u8 = preset.into();
        let preset_number = PresetNumber::new(id)?;
        let command = PresetCommand {
            action: PresetAction::Reset,
            preset_number,
        };
        self.send_and_wait(&command).await
    }

    /// Zoom in at standard speed.
    pub async fn zoom_in(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomInStandard).await
    }

    /// Zoom out at standard speed.
    pub async fn zoom_out(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomOutStandard).await
    }

    /// Stop zooming.
    pub async fn zoom_stop(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::Stop).await
    }

    /// Set zoom to direct position.
    pub async fn set_zoom(&mut self, position: u16) -> Result<(), Error> {
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
    pub async fn focus_auto(&mut self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Auto).await
    }

    /// Set focus mode to manual.
    pub async fn focus_manual(&mut self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Manual).await
    }

    /// Set focus to direct position (manual mode).
    pub async fn set_focus(&mut self, position: u16) -> Result<(), Error> {
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
    pub async fn set_exposure_mode(&mut self, mode: ExposureMode) -> Result<(), Error> {
        let command = ExposureCommand { mode };
        self.send_and_wait(&command).await
    }

    /// Set white balance mode.
    pub async fn set_white_balance_mode(&mut self, mode: WhiteBalanceMode) -> Result<(), Error> {
        let command = WhiteBalanceCommand { mode };
        self.send_and_wait(&command).await
    }

    /// Set gain value.
    pub async fn set_gain(&mut self, gain: P::GainValue) -> Result<(), Error> {
        let value: u8 = gain.into();
        let gain_value = GainValue::new(value)?;
        self.send_and_wait(&GainCommand::Direct(gain_value)).await
    }

    // Exposure Compensation Methods

    /// Enable exposure compensation.
    pub async fn exposure_compensation_on(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::On).await
    }

    /// Disable exposure compensation.
    pub async fn exposure_compensation_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Off).await
    }

    /// Reset exposure compensation to 0.
    pub async fn exposure_compensation_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Reset).await
    }

    /// Increase exposure compensation by one step.
    pub async fn exposure_compensation_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Up).await
    }

    /// Decrease exposure compensation by one step.
    pub async fn exposure_compensation_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Down).await
    }

    /// Set exposure compensation level directly (-7 to +7).
    pub async fn set_exposure_compensation(&mut self, level: i8) -> Result<(), Error> {
        let comp_level = ExposureCompensationLevel::new(level)?;
        self.send_and_wait(&ExposureCompensationCommand::Direct(comp_level)).await
    }

    // Dynamic Range Methods

    /// Set dynamic range level (0-8).
    pub async fn set_dynamic_range(&mut self, level: u8) -> Result<(), Error> {
        let dr_level = DynamicRangeLevel::new(level)?;
        self.send_and_wait(&DynamicRangeCommand::Direct(dr_level)).await
    }

    // Iris Methods

    /// Reset iris to default.
    pub async fn iris_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Reset).await
    }

    /// Increase iris opening by one step.
    pub async fn iris_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Up).await
    }

    /// Decrease iris opening by one step.
    pub async fn iris_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Down).await
    }

    /// Set iris level directly.
    pub async fn set_iris(&mut self, level: u8) -> Result<(), Error> {
        let iris_level = IrisLevel::new(level)?;
        self.send_and_wait(&IrisCommand::Direct(iris_level)).await
    }

    // Shutter Methods

    /// Reset shutter speed to default.
    pub async fn shutter_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Reset).await
    }

    /// Increase shutter speed by one step.
    pub async fn shutter_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Up).await
    }

    /// Decrease shutter speed by one step.
    pub async fn shutter_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Down).await
    }

    /// Set shutter speed directly.
    pub async fn set_shutter(&mut self, speed: u16) -> Result<(), Error> {
        let shutter_speed = ShutterSpeed::new(speed)?;
        self.send_and_wait(&ShutterCommand::Direct(shutter_speed)).await
    }

    // Brightness Methods

    /// Reset brightness to default.
    pub async fn brightness_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Reset).await
    }

    /// Increase brightness by one step.
    pub async fn brightness_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Up).await
    }

    /// Decrease brightness by one step.
    pub async fn brightness_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Down).await
    }

    /// Set brightness level directly.
    pub async fn set_brightness(&mut self, level: u16) -> Result<(), Error> {
        let brightness_level = BrightnessLevel::new(level)?;
        self.send_and_wait(&BrightCommand::Direct(brightness_level)).await
    }

    // Gain Methods (additional)

    /// Reset gain to default.
    pub async fn gain_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Reset).await
    }

    /// Increase gain by one step.
    pub async fn gain_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Up).await
    }

    /// Decrease gain by one step.
    pub async fn gain_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Down).await
    }

    /// Set gain limit.
    pub async fn set_gain_limit(&mut self, limit: u8) -> Result<(), Error> {
        let gain_limit = GainLimit::new(limit)?;
        self.send_and_wait(&GainLimitCommand { limit: gain_limit }).await
    }

    /// Set anti-flicker mode.
    pub async fn set_anti_flicker(&mut self, mode: AntiFlickerMode) -> Result<(), Error> {
        self.send_and_wait(&AntiFlickerCommand { mode }).await
    }

    // Image Adjustment Methods

    /// Enable backlight compensation.
    pub async fn backlight_on(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: true }).await
    }

    /// Disable backlight compensation.
    pub async fn backlight_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: false }).await
    }

    /// Disable 2D noise reduction.
    pub async fn noise_reduction_2d_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction2DCommand::Off).await
    }

    /// Set 2D noise reduction level (1-5).
    pub async fn set_noise_reduction_2d(&mut self, level: u8) -> Result<(), Error> {
        let nr_level = NoiseReduction2DLevel::new(level)?;
        self.send_and_wait(&NoiseReduction2DCommand::Level(nr_level)).await
    }

    /// Disable 3D noise reduction.
    pub async fn noise_reduction_3d_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction3DCommand::Off).await
    }

    /// Set 3D noise reduction level (1-5).
    pub async fn set_noise_reduction_3d(&mut self, level: u8) -> Result<(), Error> {
        let nr_level = NoiseReduction3DLevel::new(level)?;
        self.send_and_wait(&NoiseReduction3DCommand::Level(nr_level)).await
    }

    /// Enable black and white mode.
    pub async fn black_white_on(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: true }).await
    }

    /// Disable black and white mode (color mode).
    pub async fn black_white_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: false }).await
    }

    /// Set image flip mode.
    pub async fn set_image_flip(&mut self, mode: ImageFlipMode) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCombinedCommand { mode }).await
    }

    // Flip Methods (Simple vertical flip)

    /// Enable image flip (vertical).
    pub async fn flip_on(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: Flip::On }).await
    }

    /// Disable image flip (vertical).
    pub async fn flip_off(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: Flip::Off }).await
    }

    // Color Adjustment Methods

    /// Trigger one-push white balance adjustment.
    pub async fn one_push_white_balance(&mut self) -> Result<(), Error> {
        self.send_and_wait(&OnePushTriggerCommand).await
    }

    /// Set red tuning level (-10 to +10).
    pub async fn set_red_tuning(&mut self, level: i8) -> Result<(), Error> {
        self.send_and_wait(&RedTuningCommand { level }).await
    }

    /// Set blue tuning level (-10 to +10).
    pub async fn set_blue_tuning(&mut self, level: i8) -> Result<(), Error> {
        self.send_and_wait(&BlueTuningCommand { level }).await
    }

    /// Set saturation level (0x0 = 60%, 0xE = 200%).
    pub async fn set_saturation(&mut self, level: u8) -> Result<(), Error> {
        self.send_and_wait(&SaturationCommand { level }).await
    }

    /// Set hue level (0x0 to 0xE).
    pub async fn set_hue(&mut self, level: u8) -> Result<(), Error> {
        self.send_and_wait(&HueCommand { level }).await
    }

    /// Reset color temperature to default.
    pub async fn color_temperature_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Reset).await
    }

    /// Increase color temperature (cooler/bluer).
    pub async fn color_temperature_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Up).await
    }

    /// Decrease color temperature (warmer/redder).
    pub async fn color_temperature_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Down).await
    }

    /// Set color temperature directly (0x00 = 2500K to 0x37 = 8000K).
    pub async fn set_color_temperature(&mut self, temp: u16) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Direct(temp)).await
    }

    /// Reset red gain to default.
    pub async fn red_gain_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Reset).await
    }

    /// Increase red gain by one step.
    pub async fn red_gain_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Up).await
    }

    /// Decrease red gain by one step.
    pub async fn red_gain_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Down).await
    }

    /// Set red gain directly.
    pub async fn set_red_gain(&mut self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Direct(value)).await
    }

    /// Reset blue gain to default.
    pub async fn blue_gain_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Reset).await
    }

    /// Increase blue gain by one step.
    pub async fn blue_gain_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Up).await
    }

    /// Decrease blue gain by one step.
    pub async fn blue_gain_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Down).await
    }

    /// Set blue gain directly.
    pub async fn set_blue_gain(&mut self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Direct(value)).await
    }

    // Luminance, Contrast, and Sharpness Methods

    /// Set sharpness mode.
    pub async fn set_sharpness_mode(&mut self, mode: SharpnessMode) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Mode(mode)).await
    }

    /// Reset sharpness to default.
    pub async fn sharpness_reset(&mut self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Reset).await
    }

    /// Increase sharpness by one step.
    pub async fn sharpness_up(&mut self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Up).await
    }

    /// Decrease sharpness by one step.
    pub async fn sharpness_down(&mut self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Down).await
    }

    /// Set sharpness directly (0-11).
    pub async fn set_sharpness(&mut self, value: u8) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Direct { value }).await
    }

    /// Set luminance level.
    pub async fn set_luminance(&mut self, level: u8) -> Result<(), Error> {
        let luminance_level = LuminanceLevel::new(level)?;
        self.send_and_wait(&LuminanceCommand {
                value: luminance_level,
            }).await
    }

    /// Set contrast level.
    pub async fn set_contrast(&mut self, level: u8) -> Result<(), Error> {
        let contrast_level = ContrastLevel::new(level)?;
        self.send_and_wait(&ContrastCommand {
                value: contrast_level,
            }).await
    }
}
