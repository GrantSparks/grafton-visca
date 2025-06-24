//! Type-safe command methods for Camera.

use std::convert::TryFrom;

// Common imports for both async and blocking
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
        image_adjustment::{ContrastCommand, LuminanceCommand, SharpnessCommand, SharpnessMode},
        pan_tilt::{PanTiltCommand, PanTiltDirection},
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand},
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        zoom::ZoomCommand,
    },
    types::{
        BlueGain, BlueTuning, BrightnessLevel, ColorTemperature, ContrastLevel, FocusPosition,
        GainLimit, GainValue, HueLevel, IrisLevel, LuminanceLevel, NoiseReduction2DLevel,
        NoiseReduction3DLevel, PanSpeed, RedGain, RedTuning, SaturationLevel, SharpnessLevel,
        ShutterSpeed, SpeedLevel, TiltSpeed, ZoomPosition,
    },
};

use super::{Camera, CameraProfile};
use crate::units::Degrees;

// Power and Movement Commands
camera_commands! {
    /// Power on the camera.
    pub fn power_on => PowerCommand { power: Power::On };

    /// Power off the camera.
    pub fn power_off => PowerCommand { power: Power::Standby };

    /// Stop all camera movement.
    ///
    /// This is a convenience method that sends a pan-tilt move command with the
    /// Stop direction and zero speeds, which halts any ongoing movement.
    pub fn stop => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        }
    };

    /// Reset pan and tilt to home position.
    pub fn home => PanTiltCommand::Home;

    /// Reset pan and tilt motors.
    ///
    /// This recalibrates the pan/tilt position sensors. The camera will perform
    /// a full range motion to determine its limits.
    pub fn reset_pan_tilt => PanTiltCommand::Reset;
}

// Preset Commands
camera_commands! {
    /// Recall a stored preset position.
    ///
    /// Moves the camera to a previously saved preset position. The camera
    /// will move its pan, tilt, zoom, and focus to the stored values.
    pub fn recall_preset(preset_id: u8) => {
        PresetCommand::new::<P>(PresetAction::Recall, P::PresetId::try_from(preset_id)?)?
    };

    /// Save current position as a preset.
    ///
    /// Stores the current pan, tilt, zoom, and focus positions to the
    /// specified preset number for later recall.
    pub fn set_preset(preset_id: u8) => {
        PresetCommand::new::<P>(PresetAction::Set, P::PresetId::try_from(preset_id)?)?
    };
}

// Zoom Commands
camera_commands! {
    /// Start zooming in (telephoto direction).
    pub fn zoom_in => ZoomCommand::TeleStandard;

    /// Start zooming out (wide direction).
    pub fn zoom_out => ZoomCommand::WideStandard;

    /// Stop zoom movement.
    pub fn zoom_stop => ZoomCommand::Stop;

    /// Set zoom to specific position.
    pub fn set_zoom_position(position: ZoomPosition) => ZoomCommand::Position(position);
}

// Focus Commands
camera_commands! {
    /// Focus on a near object.
    pub fn focus_near => FocusCommand::Near;

    /// Focus on a far object.
    pub fn focus_far => FocusCommand::Far;

    /// Stop focus adjustment.
    pub fn focus_stop => FocusCommand::Stop;

    /// Set focus to specific position (manual mode).
    pub fn set_focus_position(position: FocusPosition) => FocusCommand::Position(position);

    /// Set focus to auto mode.
    pub fn focus_auto => FocusCommand::Auto;

    /// Set focus to manual mode.
    pub fn focus_manual => FocusCommand::Manual;

    /// Trigger one-push autofocus.
    ///
    /// The camera will perform a single autofocus operation and then
    /// return to manual focus mode.
    pub fn focus_one_push_trigger => FocusCommand::OnePushTrigger;
}

// White Balance Commands
camera_commands! {
    /// Set white balance mode.
    pub fn set_white_balance_mode(mode: WhiteBalanceMode) => WhiteBalanceCommand { mode };

    /// Trigger one-push white balance.
    ///
    /// The camera will measure the white balance on the current scene
    /// and apply the correction.
    pub fn white_balance_one_push_trigger => OnePushTriggerCommand;

    /// Set white balance color temperature in Kelvin.
    pub fn set_white_balance_kelvin(kelvin: crate::units::Kelvin) => {
        let temperature = ColorTemperature::try_from(kelvin)?;
        ColorTemperatureCommand::SetTemperature(temperature)
    };
}

// Exposure Commands
camera_commands! {
    /// Set exposure mode.
    pub fn set_exposure_mode(mode: ExposureMode) => ExposureCommand { mode };

    /// Set iris level directly.
    pub fn set_iris_level(level: IrisLevel) => IrisCommand::SetAperture(level);

    /// Reset iris to default position.
    pub fn iris_reset => IrisCommand::Reset;

    /// Set shutter speed directly.
    pub fn set_shutter_speed(speed: ShutterSpeed) => ShutterCommand::SetSpeed(speed);

    /// Reset shutter to default speed.
    pub fn shutter_reset => ShutterCommand::Reset;

    /// Enable or disable backlight compensation.
    pub fn set_backlight(enabled: bool) => BacklightCommand { status: enabled };

    /// Set anti-flicker mode.
    pub fn set_anti_flicker(mode: AntiFlickerMode) => AntiFlickerCommand { mode };

    /// Set dynamic range (HDR).
    pub fn set_dynamic_range(level: DynamicRangeLevel) => DynamicRangeCommand::SetLevel(level);
}

// Image Quality Commands
camera_commands! {
    /// Set luminance level.
    pub fn set_luminance(level: LuminanceLevel) => LuminanceCommand { value: level };

    /// Reset contrast to default.
    pub fn contrast_reset => {
        ContrastCommand {
            value: ContrastLevel::new(7)?,
        }
    };

    /// Reset sharpness to default.
    pub fn sharpness_reset => SharpnessCommand::Reset;

    /// Set sharpness mode.
    pub fn set_sharpness_mode(mode: SharpnessMode) => SharpnessCommand::Mode(mode);

    /// Enable or disable black and white mode.
    pub fn set_black_white(enabled: bool) => BlackWhiteCommand { on: enabled };
}

// Color Adjustment Commands
camera_commands! {
    /// Reset saturation to default.
    pub fn saturation_reset => {
        SaturationCommand {
            level: SaturationLevel::new(0x07)?,
        }
    };

    /// Reset hue to default.
    pub fn hue_reset => {
        HueCommand {
            level: HueLevel::new(0x07)?,
        }
    };

    /// Set color temperature.
    pub fn set_color_temperature(temperature: ColorTemperature) => {
        ColorTemperatureCommand::SetTemperature(temperature)
    };

    /// Reset color temperature to default.
    pub fn color_temperature_reset => ColorTemperatureCommand::Reset;
}

// Image Adjustment Commands
camera_commands! {
    /// Enable or disable horizontal image flip.
    pub fn set_flip_horizontal(enabled: bool) => {
        ImageFlipCommand {
            flip: if enabled { Flip::On } else { Flip::Off },
        }
    };

    /// Set combined image flip mode.
    pub fn set_flip_mode(mode: ImageFlipMode) => ImageFlipCombinedCommand { mode };
}

// Noise Reduction Commands
camera_commands! {
    /// Set 2D noise reduction level.
    pub fn set_noise_reduction_2d(level: NoiseReduction2DLevel) => {
        NoiseReduction2DCommand::Level(level)
    };

    /// Set 3D noise reduction level.
    pub fn set_noise_reduction_3d(level: NoiseReduction3DLevel) => {
        NoiseReduction3DCommand::Level(level)
    };
}

// Gain Commands
camera_commands! {
    /// Set gain value.
    pub fn set_gain(value: GainValue) => GainCommand::SetValue(value);

    /// Set gain limit.
    pub fn set_gain_limit(limit: GainLimit) => GainLimitCommand { limit };
}

// Additional Exposure Commands
camera_commands! {
    /// Set exposure compensation mode.
    pub fn set_exposure_compensation_enabled(enabled: bool) => {
        if enabled { ExposureCompensationCommand::On } else { ExposureCompensationCommand::Off }
    };

    /// Set exposure compensation level.
    pub fn set_exposure_compensation(level: ExposureCompensationLevel) => {
        ExposureCompensationCommand::SetLevel(level)
    };

    /// Reset exposure compensation.
    pub fn exposure_compensation_reset => ExposureCompensationCommand::Reset;

    /// Set brightness level.
    pub fn set_brightness(level: BrightnessLevel) => BrightCommand::SetLevel(level);
}

// Color Gain Commands
camera_commands! {
    /// Set red gain.
    pub fn set_red_gain(gain: RedGain) => RedGainCommand::SetValue(gain);

    /// Reset red gain to default.
    pub fn red_gain_reset => RedGainCommand::Reset;

    /// Set blue gain.
    pub fn set_blue_gain(gain: BlueGain) => BlueGainCommand::SetValue(gain);

    /// Reset blue gain to default.
    pub fn blue_gain_reset => BlueGainCommand::Reset;
}

// Additional Image Adjustment Commands
camera_commands! {
    /// Set contrast level.
    pub fn set_contrast(level: ContrastLevel) => ContrastCommand { value: level };

    /// Set sharpness level.
    pub fn set_sharpness(level: SharpnessLevel) => SharpnessCommand::SetLevel { value: level.into() };

    /// Set saturation level.
    pub fn set_saturation(level: SaturationLevel) => SaturationCommand { level };

    /// Set hue level.
    pub fn set_hue(level: HueLevel) => HueCommand { level };
}

// Pan/Tilt Movement Commands
camera_commands! {
    /// Move pan left.
    pub fn pan_left(speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::Left,
            pan_speed: PanSpeed::new(speed.to_pan_speed())?,
            tilt_speed: TiltSpeed::new(0)?,
        }
    };

    /// Move pan right.
    pub fn pan_right(speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::Right,
            pan_speed: PanSpeed::new(speed.to_pan_speed())?,
            tilt_speed: TiltSpeed::new(0)?,
        }
    };

    /// Move tilt up.
    pub fn tilt_up(speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::Up,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(speed.to_tilt_speed())?,
        }
    };

    /// Move tilt down.
    pub fn tilt_down(speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::Down,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(speed.to_tilt_speed())?,
        }
    };

    /// Move pan/tilt up-left.
    pub fn move_up_left(pan_speed: SpeedLevel, tilt_speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::UpLeft,
            pan_speed: PanSpeed::new(pan_speed.to_pan_speed())?,
            tilt_speed: TiltSpeed::new(tilt_speed.to_tilt_speed())?,
        }
    };

    /// Move pan/tilt up-right.
    pub fn move_up_right(pan_speed: SpeedLevel, tilt_speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::UpRight,
            pan_speed: PanSpeed::new(pan_speed.to_pan_speed())?,
            tilt_speed: TiltSpeed::new(tilt_speed.to_tilt_speed())?,
        }
    };

    /// Move pan/tilt down-left.
    pub fn move_down_left(pan_speed: SpeedLevel, tilt_speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::DownLeft,
            pan_speed: PanSpeed::new(pan_speed.to_pan_speed())?,
            tilt_speed: TiltSpeed::new(tilt_speed.to_tilt_speed())?,
        }
    };

    /// Move pan/tilt down-right.
    pub fn move_down_right(pan_speed: SpeedLevel, tilt_speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction: PanTiltDirection::DownRight,
            pan_speed: PanSpeed::new(pan_speed.to_pan_speed())?,
            tilt_speed: TiltSpeed::new(tilt_speed.to_tilt_speed())?,
        }
    };
}

// Position Commands
camera_commands! {
    /// Move to absolute position specified in degrees.
    ///
    /// Both pan and tilt angles are specified in degrees.
    pub fn pan_tilt_degrees(pan_degrees: f32, tilt_degrees: f32) => {
        PanTiltCommand::absolute_position_degrees::<P>(Degrees(pan_degrees), Degrees(tilt_degrees))?
    };

    /// Move to absolute position specified in degrees.
    pub fn set_position(pan: Degrees<f32>, tilt: Degrees<f32>) => {
        PanTiltCommand::absolute_position_degrees::<P>(pan, tilt)?
    };
}

// Continuous Movement Commands
camera_commands! {
    /// Move camera continuously in specified direction with given speeds.
    ///
    /// # Examples
    /// ```ignore
    /// // Using typed speeds
    /// camera.move_continuous(PanTiltDirection::DownLeft, PanSpeed::new(15)?, TiltSpeed::new(12)?)?;
    /// ```
    pub fn move_continuous(
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) => {
        PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        }
    };

    /// Move camera continuously with speed level.
    pub fn move_continuous_level(direction: PanTiltDirection, speed: SpeedLevel) => {
        PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::from(speed),
            tilt_speed: TiltSpeed::from(speed),
        }
    };
}

// Additional Zoom Commands
camera_commands! {
    /// Set zoom to direct position with flexible parameter types.
    ///
    /// Accepts zoom position as:
    /// - u16 raw VISCA values
    /// - f32 normalized values (0.0-1.0)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_zoom(0x3000u16)?;
    ///
    /// // Using normalized value (50% zoom)
    /// camera.set_zoom(0.5f32)?;
    ///
    /// // Using typed position
    /// camera.set_zoom(ZoomPosition::new(0x2000)?)?;
    /// ```
    pub fn set_zoom(position: ZoomPosition) => ZoomCommand::Position(position);
}

// Additional Focus Commands
camera_commands! {
    /// Set focus to specific position (manual mode).
    pub fn set_focus(position: FocusPosition) => FocusCommand::Position(position);
}

// Iris Commands (Alias)
camera_commands! {
    /// Set iris level directly (alias for set_iris_level).
    pub fn set_iris(level: IrisLevel) => IrisCommand::SetAperture(level);
}

// Pan/Tilt helper
camera_commands! {
    /// Pan and tilt by normalized values (-1.0 to 1.0).
    pub fn pan_tilt(pan: f32, tilt: f32) => {
        let pan_degrees = pan * 170.0;  // PTZOptics typical pan range
        let tilt_degrees = tilt * 90.0;  // PTZOptics typical tilt range
        PanTiltCommand::absolute_position_degrees::<P>(Degrees(pan_degrees), Degrees(tilt_degrees))?
    };
}

// Unit Conversion Methods (using extension trait approach)
// These require generics and will be handled separately

// Color Tuning Commands
camera_commands! {
    /// Set red tuning.
    pub fn set_red_tuning(tuning: RedTuning) => RedTuningCommand { level: tuning };

    /// Set blue tuning.
    pub fn set_blue_tuning(tuning: BlueTuning) => BlueTuningCommand { level: tuning };
}
