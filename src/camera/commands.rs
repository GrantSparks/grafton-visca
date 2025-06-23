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
        luminance_contrast_sharpness::{
            ContrastCommand, LuminanceCommand, SharpnessCommand, SharpnessMode,
        },
        pan_tilt::{PanTiltCommand, PanTiltDirection},
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand},
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        zoom::ZoomCommand,
    },
    types::{
        BlueGain, BlueTuning, BrightnessLevel, ColorTemperature, ContrastLevel, FStop,
        FocusPosition, GainLimit, GainValue, HueLevel, IrisLevel, LuminanceLevel,
        NoiseReduction2DLevel, NoiseReduction3DLevel, PanSpeed, RedGain, RedTuning,
        SaturationLevel, SharpnessLevel, ShutterSpeed, SpeedLevel, TiltSpeed, ZoomPosition,
    },
    visca_camera_method, visca_method, visca_method_custom,
};

use super::{Camera, CameraProfile};
use crate::units::{Degrees, Magnification, Normalized, Percentage};

// Phase 1: Simple methods replaced with procedural macro
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
{
    /// Power on the camera.
    #[visca_method]
    pub fn power_on(&self) {
        PowerCommand { power: Power::On }
    }

    /// Power off the camera.
    #[visca_method]
    pub fn power_off(&self) {
        PowerCommand {
            power: Power::Standby,
        }
    }

    /// Stop all camera movement.
    ///
    /// This is a convenience method that sends a pan-tilt move command with the
    /// Stop direction and zero speeds, which halts any ongoing movement.
    #[visca_method_custom]
    pub fn stop(&self) {
        PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)?,
            tilt_speed: TiltSpeed::new(0)?,
        }
    }

    /// Reset pan and tilt to home position.
    #[visca_method]
    pub fn home(&self) {
        PanTiltCommand::Home
    }

    /// Reset pan and tilt motors.
    ///
    /// This recalibrates the pan/tilt position sensors. The camera will perform
    /// a full range motion to determine its limits.
    #[visca_method]
    pub fn reset_pan_tilt(&self) {
        PanTiltCommand::Reset
    }

    /// Recall a stored preset position.
    ///
    /// Moves the camera to a previously saved preset position. The camera
    /// will move its pan, tilt, zoom, and focus to the stored values.
    #[visca_method_custom]
    pub fn recall_preset(&self, preset_id: u8) {
        PresetCommand::new::<P>(PresetAction::Recall, P::PresetId::try_from(preset_id)?)?
    }

    /// Save current position as a preset.
    ///
    /// Stores the current pan, tilt, zoom, and focus positions to the
    /// specified preset number for later recall.
    #[visca_method_custom]
    pub fn set_preset(&self, preset_id: u8) {
        PresetCommand::new::<P>(PresetAction::Set, P::PresetId::try_from(preset_id)?)?
    }

    /// Start zooming in (telephoto direction).
    #[visca_method]
    pub fn zoom_in(&self) {
        ZoomCommand::ZoomInStandard
    }

    /// Start zooming out (wide direction).
    #[visca_method]
    pub fn zoom_out(&self) {
        ZoomCommand::ZoomOutStandard
    }

    /// Stop zoom movement.
    #[visca_method]
    pub fn zoom_stop(&self) {
        ZoomCommand::Stop
    }

    /// Set zoom to direct position.
    #[visca_method]
    pub fn zoom_direct(&self, position: ZoomPosition) {
        ZoomCommand::Direct(position)
    }

    // Digital zoom functionality not yet implemented in ZoomCommand enum

    /// Focus on a near object.
    #[visca_method]
    pub fn focus_near(&self) {
        FocusCommand::FocusNearStandard
    }

    /// Focus on a far object.
    #[visca_method]
    pub fn focus_far(&self) {
        FocusCommand::FocusFarStandard
    }

    /// Stop focus adjustment.
    #[visca_method]
    pub fn focus_stop(&self) {
        FocusCommand::Stop
    }

    /// Set focus to direct position (manual mode).
    #[visca_method]
    pub fn focus_direct(&self, position: FocusPosition) {
        FocusCommand::Direct(position)
    }

    /// Set focus to auto mode.
    #[visca_method]
    pub fn focus_auto(&self) {
        FocusCommand::Auto
    }

    /// Set focus to manual mode.
    #[visca_method]
    pub fn focus_manual(&self) {
        FocusCommand::Manual
    }

    /// Trigger one-push autofocus.
    ///
    /// The camera will perform a single autofocus operation and then
    /// return to manual focus mode.
    #[visca_method]
    pub fn focus_one_push_trigger(&self) {
        FocusCommand::OnePushTrigger
    }

    /// Set white balance mode.
    #[visca_method]
    pub fn set_white_balance_mode(&self, mode: WhiteBalanceMode) {
        WhiteBalanceCommand { mode }
    }

    /// Trigger one-push white balance.
    ///
    /// The camera will measure the white balance on the current scene
    /// and apply the correction.
    #[visca_method]
    pub fn white_balance_one_push_trigger(&self) {
        OnePushTriggerCommand
    }

    /// Set exposure mode.
    #[visca_method]
    pub fn set_exposure_mode(&self, mode: ExposureMode) {
        ExposureCommand { mode }
    }

    /// Set iris level directly.
    #[visca_method]
    pub fn iris_direct(&self, level: IrisLevel) {
        IrisCommand::Direct(level)
    }

    /// Reset iris to default position.
    #[visca_method]
    pub fn iris_reset(&self) {
        IrisCommand::Reset
    }

    /// Set shutter speed directly.
    #[visca_method]
    pub fn shutter_direct(&self, speed: ShutterSpeed) {
        ShutterCommand::Direct(speed)
    }

    /// Reset shutter to default speed.
    #[visca_method]
    pub fn shutter_reset(&self) {
        ShutterCommand::Reset
    }

    /// Enable or disable backlight compensation.
    #[visca_method]
    pub fn set_backlight(&self, enabled: bool) {
        BacklightCommand { status: enabled }
    }

    /// Set anti-flicker mode.
    #[visca_method]
    pub fn set_anti_flicker(&self, mode: AntiFlickerMode) {
        AntiFlickerCommand { mode }
    }

    /// Set dynamic range (HDR).
    #[visca_method]
    pub fn set_dynamic_range(&self, level: DynamicRangeLevel) {
        DynamicRangeCommand::Direct(level)
    }

    /// Set luminance level.
    #[visca_method]
    pub fn set_luminance(&self, level: LuminanceLevel) {
        LuminanceCommand { value: level }
    }

    /// Reset contrast to default.
    #[visca_method_custom]
    pub fn contrast_reset(&self) {
        ContrastCommand {
            value: ContrastLevel::new(7)?,
        }
    }

    /// Reset sharpness to default.
    #[visca_method]
    pub fn sharpness_reset(&self) {
        SharpnessCommand::Reset
    }

    /// Set sharpness mode.
    #[visca_method]
    pub fn set_sharpness_mode(&self, mode: SharpnessMode) {
        SharpnessCommand::Mode(mode)
    }

    /// Enable or disable black and white mode.
    #[visca_method]
    pub fn set_black_white(&self, enabled: bool) {
        BlackWhiteCommand { on: enabled }
    }

    /// Reset saturation to default.
    #[visca_method_custom]
    pub fn saturation_reset(&self) {
        SaturationCommand {
            level: SaturationLevel::new(0x07)?,
        }
    }

    /// Reset hue to default.
    #[visca_method_custom]
    pub fn hue_reset(&self) {
        HueCommand {
            level: HueLevel::new(0x07)?,
        }
    }

    /// Set color temperature.
    #[visca_method]
    pub fn set_color_temperature(&self, temperature: ColorTemperature) {
        ColorTemperatureCommand::Direct(temperature)
    }

    /// Reset color temperature to default.
    #[visca_method]
    pub fn color_temperature_reset(&self) {
        ColorTemperatureCommand::Reset
    }

    /// Set red gain.
    #[visca_method]
    pub fn set_red_gain(&self, gain: RedGain) {
        RedGainCommand::Direct(gain)
    }

    /// Reset red gain to default.
    #[visca_method]
    pub fn red_gain_reset(&self) {
        RedGainCommand::Reset
    }

    /// Set blue gain.
    #[visca_method]
    pub fn set_blue_gain(&self, gain: BlueGain) {
        BlueGainCommand::Direct(gain)
    }

    /// Reset blue gain to default.
    #[visca_method]
    pub fn blue_gain_reset(&self) {
        BlueGainCommand::Reset
    }

    /// Set red tuning level.
    #[visca_method]
    pub fn set_red_tuning(&self, tuning: RedTuning) {
        RedTuningCommand { level: tuning }
    }

    /// Set blue tuning level.
    #[visca_method]
    pub fn set_blue_tuning(&self, tuning: BlueTuning) {
        BlueTuningCommand { level: tuning }
    }

    /// Set image flip mode.
    #[visca_method]
    pub fn set_flip(&self, mode: ImageFlipMode) {
        ImageFlipCombinedCommand { mode }
    }

    /// Set horizontal flip.
    #[visca_method_custom]
    pub fn set_flip_horizontal(&self, enabled: bool) {
        ImageFlipCommand {
            flip: if enabled { Flip::On } else { Flip::Off },
        }
    }

    /// Reset gain to default.
    #[visca_method]
    pub fn gain_reset(&self) {
        GainCommand::Reset
    }

    /// Set gain limit.
    #[visca_method]
    pub fn set_gain_limit(&self, limit: GainLimit) {
        GainLimitCommand { limit }
    }

    /// Set exposure compensation.
    #[visca_method]
    pub fn set_exposure_compensation(&self, level: ExposureCompensationLevel) {
        ExposureCompensationCommand::Direct(level)
    }

    /// Reset exposure compensation.
    #[visca_method]
    pub fn exposure_compensation_reset(&self) {
        ExposureCompensationCommand::Reset
    }

    /// Enable exposure compensation.
    #[visca_method]
    pub fn exposure_compensation_on(&self) {
        ExposureCompensationCommand::On
    }

    /// Disable exposure compensation.
    #[visca_method]
    pub fn exposure_compensation_off(&self) {
        ExposureCompensationCommand::Off
    }

    /// Reset brightness to default.
    #[visca_method]
    pub fn brightness_reset(&self) {
        BrightCommand::Reset
    }

    /// Set 2D noise reduction level.
    #[visca_method]
    pub fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) {
        NoiseReduction2DCommand::Level(level)
    }

    /// Reset 2D noise reduction to default.
    #[visca_method]
    pub fn noise_reduction_2d_reset(&self) {
        NoiseReduction2DCommand::Off
    }

    /// Set 3D noise reduction level.
    #[visca_method]
    pub fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) {
        NoiseReduction3DCommand::Level(level)
    }

    /// Move to absolute pan/tilt position.
    ///
    /// Both pan and tilt positions are specified in normalized units (0.0 to 1.0).
    #[visca_method_custom]
    pub fn pan_tilt(&self, pan: f32, tilt: f32) {
        PanTiltCommand::absolute_position_normalized::<P>(
            Normalized::new(pan),
            Normalized::new(tilt),
        )?
    }

    /// Move to absolute position specified in degrees.
    ///
    /// Both pan and tilt angles are specified in degrees.
    #[visca_method_custom]
    pub fn pan_tilt_degrees(&self, pan_degrees: f32, tilt_degrees: f32) {
        PanTiltCommand::absolute_position_degrees::<P>(Degrees(pan_degrees), Degrees(tilt_degrees))?
    }

    /// Move to absolute position specified in degrees.
    #[visca_method_custom]
    pub fn set_position(&self, pan: Degrees<f32>, tilt: Degrees<f32>) {
        PanTiltCommand::absolute_position_degrees::<P>(pan, tilt)?
    }
}

// Generic parameter methods for enhanced API ergonomics
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
{
    /// Move the camera continuously in a direction with flexible speed parameters.
    ///
    /// Accepts pan and tilt speeds as:
    /// - Raw u8 values (1-24 for pan, 1-20 for tilt)
    /// - PanSpeed/TiltSpeed types for type safety
    /// - SpeedLevel enum for intuitive speed selection
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw values
    /// camera.move_continuous(PanTiltDirection::Right, 10u8, 8u8).await?;
    ///
    /// // Using speed levels
    /// camera.move_continuous(PanTiltDirection::Up, SpeedLevel::Fast, SpeedLevel::Medium).await?;
    ///
    /// // Using typed speeds
    /// camera.move_continuous(PanTiltDirection::DownLeft, PanSpeed::new(15)?, TiltSpeed::new(12)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn move_continuous(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) {
        PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        }
    }

    /// Move camera continuously with speed values (0-24 for pan, 0-20 for tilt).
    #[visca_camera_method]
    pub fn move_continuous_raw(&self, direction: PanTiltDirection, pan_speed: u8, tilt_speed: u8) {
        let pan_speed = PanSpeed::new(pan_speed)?;
        let tilt_speed = TiltSpeed::new(tilt_speed)?;
        PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        }
    }

    /// Move camera continuously with speed level.
    #[visca_camera_method]
    pub fn move_continuous_level(&self, direction: PanTiltDirection, speed: SpeedLevel) {
        PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::from(speed),
            tilt_speed: TiltSpeed::from(speed),
        }
    }

    /// Set zoom to direct position with flexible parameter types.
    ///
    /// Accepts zoom position as:
    /// - Raw u16 values (0x0000-0x7000)
    /// - ZoomPosition type for type safety
    /// - f32 normalized values (0.0-1.0)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_zoom(0x3000u16).await?;
    ///
    /// // Using normalized value (50% zoom)
    /// camera.set_zoom(0.5f32).await?;
    ///
    /// // Using typed position
    /// camera.set_zoom(ZoomPosition::new(0x2000)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_zoom(&self, position: ZoomPosition) {
        ZoomCommand::Direct(position)
    }

    /// Set zoom position from raw value (0x0000-0x7000).
    #[visca_camera_method]
    pub fn set_zoom_raw(&self, position: u16) {
        let position = ZoomPosition::new(position)?;
        ZoomCommand::Direct(position)
    }

    /// Set zoom position from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_zoom_percentage(&self, percentage: Percentage<f32>) {
        let position = ZoomPosition::try_from(percentage)?;
        ZoomCommand::Direct(position)
    }

    /// Set zoom position from magnification factor.
    #[visca_camera_method]
    pub fn set_zoom_magnification(&self, magnification: Magnification<f32>) {
        let position = ZoomPosition::try_from(magnification)?;
        ZoomCommand::Direct(position)
    }

    /// Set focus to direct position (manual mode) with flexible parameter types.
    ///
    /// Accepts focus position as:
    /// - Raw u16 values (0x1000-0xF000)
    /// - FocusPosition type for type safety
    /// - f32 normalized values (0.0-1.0, where 0.0=infinity, 1.0=near)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_focus(0x8000u16).await?;
    ///
    /// // Using normalized value (75% to near)
    /// camera.set_focus(0.75f32).await?;
    ///
    /// // Using typed position
    /// camera.set_focus(FocusPosition::new(0x5000)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_focus(&self, position: FocusPosition) {
        FocusCommand::Direct(position)
    }

    /// Set focus position from raw value (0x1000-0xF000).
    #[visca_camera_method]
    pub fn set_focus_raw(&self, position: u16) {
        let position = FocusPosition::new(position)?;
        FocusCommand::Direct(position)
    }

    /// Set focus position from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_focus_percentage(&self, percentage: Percentage<f32>) {
        let position = FocusPosition::try_from(percentage)?;
        FocusCommand::Direct(position)
    }

    /// Set iris level directly with flexible parameter types.
    ///
    /// Accepts iris level as:
    /// - Raw u8 values (0x00-0x0C)
    /// - IrisLevel type for type safety
    /// - FStop enum for intuitive F-stop values
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_iris(0x09u8).await?;
    ///
    /// // Using F-stop enum
    /// camera.set_iris(FStop::F2_8).await?;
    ///
    /// // Using typed level
    /// camera.set_iris(IrisLevel::new(0x0B)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_iris(&self, level: IrisLevel) {
        IrisCommand::Direct(level)
    }

    /// Set iris level from raw value (0x00-0x0C).
    #[visca_camera_method]
    pub fn set_iris_raw(&self, level: u8) {
        let level = IrisLevel::new(level)?;
        IrisCommand::Direct(level)
    }

    /// Set iris from F-stop value.
    #[visca_camera_method]
    pub fn set_iris_fstop(&self, fstop: FStop) {
        IrisCommand::Direct(IrisLevel::from(fstop))
    }

    /// Set iris level from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_iris_percentage(&self, percentage: Percentage<f32>) {
        let level = IrisLevel::try_from(percentage)?;
        IrisCommand::Direct(level)
    }
}

// Additional semantic API methods for enhanced ergonomics
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
{
    /// Set white balance using color temperature in Kelvin.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Kelvin;
    ///
    /// // Set to daylight (5600K)
    /// camera.set_white_balance_kelvin(Kelvin(5600)).await?;
    ///
    /// // Set to tungsten (3200K)
    /// camera.set_white_balance_kelvin(Kelvin(3200)).await?;
    /// ```
    #[visca_method_custom]
    pub fn set_white_balance_kelvin(&self, kelvin: crate::units::Kelvin) {
        let temperature = ColorTemperature::try_from(kelvin)?;
        ColorTemperatureCommand::Direct(temperature)
    }

    /// Set shutter speed using fraction notation.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Fraction;
    ///
    /// // Set to 1/60s
    /// camera.set_shutter_fraction(Fraction::new(1, 60)).await?;
    ///
    /// // Set to 1/1000s
    /// camera.set_shutter_fraction(Fraction::new(1, 1000)).await?;
    /// ```
    #[visca_method_custom]
    pub fn set_shutter_fraction(&self, fraction: crate::units::Fraction) {
        let speed = ShutterSpeed::try_from(fraction)?;
        ShutterCommand::Direct(speed)
    }

    /// Set pan/tilt position using radians.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Radians;
    /// use std::f32::consts::PI;
    ///
    /// // Turn 90 degrees right and 45 degrees up
    /// camera.set_position_radians(Radians(PI / 2.0), Radians(PI / 4.0)).await?;
    /// ```
    #[visca_method_custom]
    pub fn set_position_radians(
        &self,
        pan: crate::units::Radians<f32>,
        tilt: crate::units::Radians<f32>,
    ) {
        let pan_degrees: Degrees<f32> = pan.into();
        let tilt_degrees: Degrees<f32> = tilt.into();
        PanTiltCommand::absolute_position_degrees::<P>(pan_degrees, tilt_degrees)?
    }

    /// Move camera at percentage of maximum speed.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Percentage;
    /// use grafton_visca::command::pan_tilt::PanTiltDirection;
    ///
    /// // Move right at 50% speed
    /// camera.move_percentage(PanTiltDirection::Right, Percentage(50.0), Percentage(0.0)).await?;
    ///
    /// // Move diagonally at 75% speed
    /// camera.move_percentage(PanTiltDirection::UpRight, Percentage(75.0), Percentage(75.0)).await?;
    /// ```
    #[visca_method_custom]
    pub fn move_percentage(
        &self,
        direction: PanTiltDirection,
        pan_speed: Percentage<f32>,
        tilt_speed: Percentage<f32>,
    ) {
        let pan = PanSpeed::try_from(pan_speed)?;
        let tilt = TiltSpeed::try_from(tilt_speed)?;
        PanTiltCommand::Move {
            direction,
            pan_speed: pan,
            tilt_speed: tilt,
        }
    }

    /// Set gain with flexible parameter types.
    ///
    /// Accepts gain value as:
    /// - Raw u8 values (0x00-0x0F)
    /// - GainValue type for type safety
    /// - `Percentage<f32>` values (0.0-100.0)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_gain(0x04u8).await?;
    ///
    /// // Using percentage (50% = ~10.5dB)
    /// camera.set_gain(Percentage(50.0)).await?;
    ///
    /// // Using typed value
    /// camera.set_gain(GainValue::new(0x08)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_gain(&self, gain: GainValue) {
        GainCommand::Direct(gain)
    }

    /// Set gain from raw value (0x00-0x07).
    #[visca_camera_method]
    pub fn set_gain_raw(&self, gain: u8) {
        let gain = GainValue::new(gain)?;
        GainCommand::Direct(gain)
    }

    /// Set gain from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_gain_percentage(&self, percentage: Percentage<f32>) {
        let gain = GainValue::try_from(percentage)?;
        GainCommand::Direct(gain)
    }

    /// Set sharpness with flexible parameter types.
    ///
    /// Accepts sharpness level as:
    /// - Raw u8 values (0x00-0x0E)
    /// - SharpnessLevel type for type safety
    /// - `Percentage<f32>` values (0.0-100.0)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_sharpness(5u8).await?;
    ///
    /// // Using percentage
    /// camera.set_sharpness(Percentage(70.0)).await?;
    ///
    /// // Using typed level
    /// camera.set_sharpness(SharpnessLevel::new(0x0A)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_sharpness(&self, level: SharpnessLevel) {
        SharpnessCommand::Direct {
            value: level.value(),
        }
    }

    /// Set sharpness from raw value (0x00-0x0B).
    #[visca_camera_method]
    pub fn set_sharpness_raw(&self, level: u8) {
        let level = SharpnessLevel::new(level)?;
        SharpnessCommand::Direct {
            value: level.value(),
        }
    }

    /// Set sharpness from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_sharpness_percentage(&self, percentage: Percentage<f32>) {
        let level = SharpnessLevel::try_from(percentage)?;
        SharpnessCommand::Direct {
            value: level.value(),
        }
    }

    /// Set brightness with flexible parameter types.
    ///
    /// Accepts brightness level as:
    /// - Raw u16 values (0x0000-0x0011)
    /// - BrightnessLevel type for type safety
    /// - `Percentage<f32>` values (0.0-100.0)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_brightness(0x0Cu16).await?;
    ///
    /// // Using percentage
    /// camera.set_brightness(Percentage(50.0)).await?;
    ///
    /// // Using typed level
    /// camera.set_brightness(BrightnessLevel::new(0x0009)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_brightness(&self, level: BrightnessLevel) {
        BrightCommand::Direct(level)
    }

    /// Set brightness from raw value (0x00-0x11).
    #[visca_camera_method]
    pub fn set_brightness_raw(&self, level: u16) {
        let level = BrightnessLevel::new(level)?;
        BrightCommand::Direct(level)
    }

    /// Set brightness from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_brightness_percentage(&self, percentage: Percentage<f32>) {
        let level = BrightnessLevel::try_from(percentage)?;
        BrightCommand::Direct(level)
    }

    /// Set contrast with flexible parameter types.
    ///
    /// Accepts contrast level as:
    /// - Raw u8 values (0x00-0x0E)
    /// - ContrastLevel type for type safety
    /// - `Percentage<f32>` values (0.0-100.0)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_contrast(8u8).await?;
    ///
    /// // Using percentage
    /// camera.set_contrast(Percentage(60.0)).await?;
    ///
    /// // Using typed level
    /// camera.set_contrast(ContrastLevel::new(0x0B)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_contrast(&self, level: ContrastLevel) {
        ContrastCommand { value: level }
    }

    /// Set contrast from raw value (0x00-0x0E).
    #[visca_camera_method]
    pub fn set_contrast_raw(&self, level: u8) {
        let value = ContrastLevel::new(level)?;
        ContrastCommand { value }
    }

    /// Set contrast from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_contrast_percentage(&self, percentage: Percentage<f32>) {
        let value = ContrastLevel::try_from(percentage)?;
        ContrastCommand { value }
    }

    /// Set saturation with flexible parameter types.
    ///
    /// Accepts saturation level as:
    /// - Raw u8 values (0x00-0x0E)
    /// - SaturationLevel type for type safety
    /// - `Percentage<f32>` values (0.0-100.0)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_saturation(10u8).await?;
    ///
    /// // Using percentage
    /// camera.set_saturation(Percentage(75.0)).await?;
    ///
    /// // Using typed level
    /// camera.set_saturation(SaturationLevel::new(0x0C)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_saturation(&self, level: SaturationLevel) {
        SaturationCommand { level }
    }

    /// Set saturation from raw value (0x00-0x0E).
    #[visca_camera_method]
    pub fn set_saturation_raw(&self, level: u8) {
        let level = SaturationLevel::new(level)?;
        SaturationCommand { level }
    }

    /// Set saturation from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_saturation_percentage(&self, percentage: Percentage<f32>) {
        let level = SaturationLevel::try_from(percentage)?;
        SaturationCommand { level }
    }

    /// Set hue with flexible parameter types.
    ///
    /// Accepts hue level as:
    /// - Raw u8 values (0x00-0x0E)
    /// - HueLevel type for type safety
    /// - `Percentage<f32>` values (0.0-100.0)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_hue(7u8).await?;
    ///
    /// // Using percentage
    /// camera.set_hue(Percentage(50.0)).await?;
    ///
    /// // Using typed level
    /// camera.set_hue(HueLevel::new(0x07)?).await?;
    /// ```
    #[visca_camera_method]
    pub fn set_hue(&self, level: HueLevel) {
        HueCommand { level }
    }

    /// Set hue from raw value (0x00-0x0E).
    #[visca_camera_method]
    pub fn set_hue_raw(&self, level: u8) {
        let level = HueLevel::new(level)?;
        HueCommand { level }
    }

    /// Set hue from percentage (0-100%).
    #[visca_camera_method]
    pub fn set_hue_percentage(&self, percentage: Percentage<f32>) {
        let level = HueLevel::try_from(percentage)?;
        HueCommand { level }
    }
}
