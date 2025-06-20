//! Type-safe command methods for Camera.

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
        luminance_contrast_sharpness::{
            ContrastCommand, LuminanceCommand, SharpnessCommand, SharpnessMode,
        },
        pan_tilt::{PanTiltCommand, PanTiltDirection},
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand},
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        zoom::ZoomCommand,
    },
    define_camera_methods,
    types::{
        BrightnessLevel, ContrastLevel, GainLimit, IrisLevel, LuminanceLevel,
        NoiseReduction2DLevel, NoiseReduction3DLevel, ShutterSpeed, ZoomPosition, FocusPosition,
        ColorTemperature, RedGain, BlueGain, SaturationLevel, HueLevel, RedTuning, BlueTuning,
        SharpnessLevel,
    },
};

use super::{
    units::{Degrees, Normalized, ViscaUnits},
    Camera, CameraProfile,
};

// Phase 1: Simple methods replaced with unified macro
define_camera_methods! {
    /// Power on the camera.
    pub fn power_on(&self) -> Result<(), Error> {
        self.send_and_wait(&PowerCommand { power: Power::On })
    }

    /// Power off the camera.
    pub fn power_off(&self) -> Result<(), Error> {
        self.send_and_wait(&PowerCommand { power: Power::Standby })
    }

    /// Move camera to home position.
    pub fn home(&self) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::Home)
    }

    /// Zoom in at standard speed.
    pub fn zoom_in(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomInStandard)
    }

    /// Zoom out at standard speed.
    pub fn zoom_out(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomOutStandard)
    }

    /// Stop zooming.
    pub fn zoom_stop(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::Stop)
    }

    /// Set focus mode to auto.
    pub fn focus_auto(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Auto)
    }

    /// Set focus mode to manual.
    pub fn focus_manual(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Manual)
    }

    /// Enable exposure compensation.
    pub fn exposure_compensation_on(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::On)
    }

    /// Disable exposure compensation.
    pub fn exposure_compensation_off(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Off)
    }

    /// Reset exposure compensation to 0.
    pub fn exposure_compensation_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Reset)
    }

    /// Increase exposure compensation by one step.
    pub fn exposure_compensation_up(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Up)
    }

    /// Decrease exposure compensation by one step.
    pub fn exposure_compensation_down(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Down)
    }

    /// Reset gain to default.
    pub fn gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Reset)
    }

    /// Increase gain by one step.
    pub fn gain_up(&self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Up)
    }

    /// Decrease gain by one step.
    pub fn gain_down(&self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Down)
    }

    /// Reset iris to default.
    pub fn iris_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Reset)
    }

    /// Increase iris opening by one step.
    pub fn iris_up(&self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Up)
    }

    /// Decrease iris opening by one step.
    pub fn iris_down(&self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Down)
    }

    /// Reset shutter speed to default.
    pub fn shutter_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Reset)
    }

    /// Increase shutter speed by one step.
    pub fn shutter_up(&self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Up)
    }

    /// Decrease shutter speed by one step.
    pub fn shutter_down(&self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Down)
    }

    /// Reset brightness to default.
    pub fn brightness_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Reset)
    }

    /// Increase brightness by one step.
    pub fn brightness_up(&self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Up)
    }

    /// Decrease brightness by one step.
    pub fn brightness_down(&self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Down)
    }

    /// Enable backlight compensation.
    pub fn backlight_on(&self) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: true })
    }

    /// Disable backlight compensation.
    pub fn backlight_off(&self) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: false })
    }

    /// Disable 2D noise reduction.
    pub fn noise_reduction_2d_off(&self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction2DCommand::Off)
    }

    /// Disable 3D noise reduction.
    pub fn noise_reduction_3d_off(&self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction3DCommand::Off)
    }

    /// Enable black and white mode.
    pub fn black_white_on(&self) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: true })
    }

    /// Disable black and white mode (color mode).
    pub fn black_white_off(&self) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: false })
    }

    /// Enable image flip (vertical).
    pub fn flip_on(&self) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: Flip::On })
    }

    /// Disable image flip (vertical).
    pub fn flip_off(&self) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: Flip::Off })
    }

    /// Trigger one-push white balance adjustment.
    pub fn one_push_white_balance(&self) -> Result<(), Error> {
        self.send_and_wait(&OnePushTriggerCommand)
    }

    /// Reset color temperature to default.
    pub fn color_temperature_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Reset)
    }

    /// Increase color temperature (cooler/bluer).
    pub fn color_temperature_up(&self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Up)
    }

    /// Decrease color temperature (warmer/redder).
    pub fn color_temperature_down(&self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Down)
    }

    /// Reset red gain to default.
    pub fn red_gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Reset)
    }

    /// Increase red gain by one step.
    pub fn red_gain_up(&self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Up)
    }

    /// Decrease red gain by one step.
    pub fn red_gain_down(&self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Down)
    }

    /// Reset blue gain to default.
    pub fn blue_gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Reset)
    }

    /// Increase blue gain by one step.
    pub fn blue_gain_up(&self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Up)
    }

    /// Decrease blue gain by one step.
    pub fn blue_gain_down(&self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Down)
    }

    /// Reset sharpness to default.
    pub fn sharpness_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Reset)
    }

    /// Increase sharpness by one step.
    pub fn sharpness_up(&self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Up)
    }

    /// Decrease sharpness by one step.
    pub fn sharpness_down(&self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Down)
    }

    /// Set the camera position using normalized coordinates (-1.0 to 1.0).
    pub fn set_position_normalized(&self, pan: Normalized<f32>, tilt: Normalized<f32>) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::absolute_position_normalized::<P>(pan, tilt)?)
    }

    /// Set the camera to an absolute position using VISCA units.
    pub fn set_position_units(&self, pan: ViscaUnits<i16>, tilt: ViscaUnits<i16>) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::absolute_position::<P>(pan.0, tilt.0)?)
    }

    /// Move the camera continuously in a direction.
    pub fn move_continuous(&self, direction: PanTiltDirection, pan_speed: u8, tilt_speed: u8) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::continuous_move::<P>(direction, pan_speed, tilt_speed)?)
    }

    /// Stop all camera movement.
    pub fn stop(&self) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::stop()?)
    }

    /// Set zoom to direct position.
    pub fn set_zoom(&self, position: ZoomPosition) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::direct::<P>(position.value())?)
    }

    /// Set focus to direct position (manual mode).
    pub fn set_focus(&self, position: FocusPosition) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::direct::<P>(position.value())?)
    }

    /// Set gain value.
    pub fn set_gain(&self, gain: P::GainValue) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::direct::<P>(gain)?)
    }

    /// Recall a preset position.
    pub fn recall_preset(&self, preset: P::PresetId) -> Result<(), Error> {
        self.send_and_wait(&PresetCommand::new::<P>(PresetAction::Recall, preset)?)
    }

    /// Set a preset position.
    pub fn set_preset(&self, preset: P::PresetId) -> Result<(), Error> {
        self.send_and_wait(&PresetCommand::new::<P>(PresetAction::Set, preset)?)
    }

    /// Clear a preset position.
    pub fn clear_preset(&self, preset: P::PresetId) -> Result<(), Error> {
        self.send_and_wait(&PresetCommand::new::<P>(PresetAction::Reset, preset)?)
    }

    /// Set exposure compensation level directly.
    pub fn set_exposure_compensation(&self, level: ExposureCompensationLevel) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Direct(level))
    }

    /// Set dynamic range level.
    pub fn set_dynamic_range(&self, level: DynamicRangeLevel) -> Result<(), Error> {
        self.send_and_wait(&DynamicRangeCommand::Direct(level))
    }

    /// Set iris level directly.
    pub fn set_iris(&self, level: IrisLevel) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Direct(level))
    }

    /// Set shutter speed directly.
    pub fn set_shutter(&self, speed: ShutterSpeed) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Direct(speed))
    }

    /// Set brightness level directly.
    pub fn set_brightness(&self, level: BrightnessLevel) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Direct(level))
    }

    /// Set gain limit.
    pub fn set_gain_limit(&self, limit: GainLimit) -> Result<(), Error> {
        self.send_and_wait(&GainLimitCommand { limit })
    }


    /// Set 2D noise reduction level.
    pub fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction2DCommand::Level(level))
    }

    /// Set 3D noise reduction level.
    pub fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction3DCommand::Level(level))
    }

    /// Set luminance level.
    pub fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error> {
        self.send_and_wait(&LuminanceCommand { value: level })
    }

    /// Set contrast level.
    pub fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error> {
        self.send_and_wait(&ContrastCommand { value: level })
    }

    /// Set exposure mode.
    pub fn set_exposure_mode(&self, mode: ExposureMode) -> Result<(), Error> {
        self.send_and_wait(&ExposureCommand { mode })
    }

    /// Set white balance mode.
    pub fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error> {
        self.send_and_wait(&WhiteBalanceCommand { mode })
    }

    /// Set anti-flicker mode.
    pub fn set_anti_flicker(&self, mode: AntiFlickerMode) -> Result<(), Error> {
        self.send_and_wait(&AntiFlickerCommand { mode })
    }

    /// Set image flip mode.
    pub fn set_image_flip(&self, mode: ImageFlipMode) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCombinedCommand { mode })
    }

    /// Set red tuning level.
    pub fn set_red_tuning(&self, level: RedTuning) -> Result<(), Error> {
        self.send_and_wait(&RedTuningCommand { level: level.value() })
    }

    /// Set blue tuning level.
    pub fn set_blue_tuning(&self, level: BlueTuning) -> Result<(), Error> {
        self.send_and_wait(&BlueTuningCommand { level: level.value() })
    }

    /// Set saturation level.
    pub fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error> {
        self.send_and_wait(&SaturationCommand { level: level.value() })
    }

    /// Set hue level.
    pub fn set_hue(&self, level: HueLevel) -> Result<(), Error> {
        self.send_and_wait(&HueCommand { level: level.value() })
    }

    /// Set color temperature directly.
    pub fn set_color_temperature(&self, temp: ColorTemperature) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Direct(temp.value()))
    }

    /// Set red gain directly.
    pub fn set_red_gain(&self, value: RedGain) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Direct(value.value()))
    }

    /// Set blue gain directly.
    pub fn set_blue_gain(&self, value: BlueGain) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Direct(value.value()))
    }

    /// Set sharpness mode.
    pub fn set_sharpness_mode(&self, mode: SharpnessMode) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Mode(mode))
    }

    /// Set sharpness directly.
    pub fn set_sharpness(&self, value: SharpnessLevel) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Direct { value: value.value() })
    }

    /// Set the camera to an absolute pan/tilt position in degrees.
    pub fn set_position(&self, pan: Degrees<f32>, tilt: Degrees<f32>) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::absolute_position_degrees::<P>(pan, tilt)?)
    }
}
