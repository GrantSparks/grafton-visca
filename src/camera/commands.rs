//! Type-safe command methods for Camera.

use std::convert::{TryFrom, TryInto};

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

    /// Set white balance color temperature with flexible parameter types.
    ///
    /// Accepts color temperature as:
    /// - `ColorTemperature` - Direct temperature type
    /// - `Kelvin` - Temperature in Kelvin (2000-8000K)
    /// - `Raw<u8>` - Raw VISCA protocol value
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Kelvin, Raw};
    ///
    /// // Using Kelvin
    /// camera.set_white_balance(Kelvin(5600))?;  // Daylight
    /// camera.set_white_balance(Kelvin(3200))?;  // Tungsten
    ///
    /// // Using raw value
    /// camera.set_white_balance(Raw(0x38))?;
    ///
    /// // Using typed temperature
    /// camera.set_white_balance(ColorTemperature::new(0x40)?)?;
    /// ```
    pub fn set_white_balance<W>(value: W) where { W: TryInto<ColorTemperature>, W::Error: Into<crate::Error> } => {
        let temperature = value.try_into().map_err(Into::into)?;
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

    /// Set shutter speed with flexible parameter types.
    ///
    /// Accepts shutter speed as:
    /// - `ShutterSpeed` - Direct shutter speed type
    /// - `Fraction` - Fraction notation (e.g., 1/60, 1/1000)
    /// - `Raw<u8>` - Raw VISCA protocol value
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Fraction, Raw};
    ///
    /// // Using fraction notation
    /// camera.set_shutter(Fraction::new(1, 60))?;      // 1/60s
    /// camera.set_shutter(Fraction::new(1, 1000))?;    // 1/1000s
    ///
    /// // Using raw value
    /// camera.set_shutter(Raw(0x0C))?;
    ///
    /// // Using typed speed
    /// camera.set_shutter(ShutterSpeed::new(0x0A)?)?;
    /// ```
    pub fn set_shutter<S>(value: S) where { S: TryInto<ShutterSpeed>, S::Error: Into<crate::Error> } => {
        let speed = value.try_into().map_err(Into::into)?;
        ShutterCommand::SetSpeed(speed)
    };

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
    /// Set gain value with flexible parameter types.
    ///
    /// Accepts gain value as:
    /// - `GainValue` - Direct gain type
    /// - `Percentage<f32>` - 0-100% of gain range (0-21dB)
    /// - `Raw<u8>` - Raw VISCA protocol value (0x00-0x07)
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Percentage, Raw};
    ///
    /// // Using percentage
    /// camera.set_gain(Percentage(50.0))?;
    ///
    /// // Using raw value
    /// camera.set_gain(Raw(0x04))?;
    ///
    /// // Using typed value
    /// camera.set_gain(GainValue::new(0x03)?)?;
    /// ```
    pub fn set_gain<G>(value: G) where { G: TryInto<GainValue>, G::Error: Into<crate::Error> } => {
        let gain = value.try_into().map_err(Into::into)?;
        GainCommand::SetValue(gain)
    };

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

    /// Set brightness level with flexible parameter types.
    ///
    /// Accepts brightness level as:
    /// - `BrightnessLevel` - Direct type
    /// - `Percentage<f32>` - 0-100% of brightness range
    /// - `Raw<u8>` - Raw VISCA protocol value
    pub fn set_brightness<B>(value: B) where { B: TryInto<BrightnessLevel>, B::Error: Into<crate::Error> } => {
        let level = value.try_into().map_err(Into::into)?;
        BrightCommand::SetLevel(level)
    };
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
    /// Set contrast level with flexible parameter types.
    ///
    /// Accepts contrast level as:
    /// - `ContrastLevel` - Direct type
    /// - `Percentage<f32>` - 0-100% of contrast range
    /// - `Raw<u8>` - Raw VISCA protocol value
    pub fn set_contrast<C>(value: C) where { C: TryInto<ContrastLevel>, C::Error: Into<crate::Error> } => {
        let level = value.try_into().map_err(Into::into)?;
        ContrastCommand { value: level }
    };

    /// Set sharpness level with flexible parameter types.
    ///
    /// Accepts sharpness level as:
    /// - `SharpnessLevel` - Direct type
    /// - `Percentage<f32>` - 0-100% of sharpness range
    /// - `Raw<u8>` - Raw VISCA protocol value
    pub fn set_sharpness<S>(value: S) where { S: TryInto<SharpnessLevel>, S::Error: Into<crate::Error> } => {
        let level = value.try_into().map_err(Into::into)?;
        SharpnessCommand::SetLevel { value: level.into() }
    };

    /// Set saturation level with flexible parameter types.
    ///
    /// Accepts saturation level as:
    /// - `SaturationLevel` - Direct type
    /// - `Percentage<f32>` - 0-100% of saturation range
    /// - `Raw<u8>` - Raw VISCA protocol value
    pub fn set_saturation<S>(value: S) where { S: TryInto<SaturationLevel>, S::Error: Into<crate::Error> } => {
        let level = value.try_into().map_err(Into::into)?;
        SaturationCommand { level }
    };

    /// Set hue level with flexible parameter types.
    ///
    /// Accepts hue level as:
    /// - `HueLevel` - Direct type
    /// - `Percentage<f32>` - 0-100% of hue range
    /// - `Raw<u8>` - Raw VISCA protocol value
    pub fn set_hue<H>(value: H) where { H: TryInto<HueLevel>, H::Error: Into<crate::Error> } => {
        let level = value.try_into().map_err(Into::into)?;
        HueCommand { level }
    };
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
    /// Move to absolute position with flexible parameter types.
    ///
    /// Accepts position as:
    /// - `Degrees<f32>` - Angle in degrees
    /// - `f32` - Raw degrees value
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Degrees;
    ///
    /// // Using degrees
    /// camera.set_position(Degrees(45.0), Degrees(-20.0))?;
    ///
    /// // Using raw degree values
    /// camera.set_position(90.0, -45.0)?;
    /// ```
    pub fn set_position<P1, T1>(pan: P1, tilt: T1)
    where {
        P1: Into<Degrees<f32>>,
        T1: Into<Degrees<f32>>
    }
    => {
        let pan_deg = pan.into();
        let tilt_deg = tilt.into();
        PanTiltCommand::absolute_position_degrees::<P>(pan_deg, tilt_deg)?
    };
}

// Continuous Movement Commands
camera_commands! {
    /// Move camera continuously in specified direction with given speeds.
    ///
    /// Accepts speeds as:
    /// - `PanSpeed`/`TiltSpeed` - Direct speed types
    /// - `Percentage<f32>` - 0-100% of maximum speed
    /// - `SpeedLevel` - Convenient speed levels
    /// - `Raw<u8>` - Raw VISCA protocol values
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Percentage, Raw};
    /// use grafton_visca::types::{PanSpeed, TiltSpeed, SpeedLevel};
    /// use grafton_visca::command::pan_tilt::PanTiltDirection;
    ///
    /// // Using typed speeds
    /// camera.move_continuous(PanTiltDirection::DownLeft, PanSpeed::new(15)?, TiltSpeed::new(12)?)?;
    ///
    /// // Using percentages
    /// camera.move_continuous(PanTiltDirection::Right, Percentage(50.0), Percentage(0.0))?;
    ///
    /// // Using speed levels
    /// camera.move_continuous(PanTiltDirection::UpRight, SpeedLevel::Medium, SpeedLevel::Medium)?;
    /// ```
    pub fn move_continuous<PS, TS>(
        direction: PanTiltDirection,
        pan_speed: PS,
        tilt_speed: TS,
    ) where {
        PS: TryInto<PanSpeed>,
        PS::Error: Into<crate::Error>,
        TS: TryInto<TiltSpeed>,
        TS::Error: Into<crate::Error>
    }
    => {
        let pan = pan_speed.try_into().map_err(Into::into)?;
        let tilt = tilt_speed.try_into().map_err(Into::into)?;
        PanTiltCommand::Move {
            direction,
            pan_speed: pan,
            tilt_speed: tilt,
        }
    };
}

// Additional Zoom Commands
camera_commands! {
    /// Set zoom position with flexible parameter types.
    ///
    /// Accepts zoom position as:
    /// - `ZoomPosition` - Direct position type
    /// - `Percentage<f32>` - 0-100% of zoom range
    /// - `Magnification<f32>` - Magnification factor (e.g., 2.5x)
    /// - `Normalized<f32>` - 0.0-1.0 normalized value
    /// - `Raw<u16>` - Raw VISCA protocol value
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Percentage, Magnification, Raw};
    ///
    /// // Using percentage
    /// camera.set_zoom(Percentage(50.0))?;
    ///
    /// // Using magnification
    /// camera.set_zoom(Magnification(2.5))?;
    ///
    /// // Using raw value
    /// camera.set_zoom(Raw(0x4000))?;
    ///
    /// // Using typed position
    /// camera.set_zoom(ZoomPosition::new(0x2000)?)?;
    /// ```
    pub fn set_zoom<Z>(value: Z) where { Z: TryInto<ZoomPosition>, Z::Error: Into<crate::Error> } => {
        let position = value.try_into().map_err(Into::into)?;
        ZoomCommand::Position(position)
    };
}

// Additional Focus Commands
camera_commands! {
    /// Set focus position with flexible parameter types.
    ///
    /// Accepts focus position as:
    /// - `FocusPosition` - Direct position type
    /// - `Percentage<f32>` - 0-100% of focus range
    /// - `Normalized<f32>` - 0.0-1.0 normalized value
    /// - `Raw<u16>` - Raw VISCA protocol value
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Percentage, Raw};
    ///
    /// // Using percentage
    /// camera.set_focus(Percentage(75.0))?;
    ///
    /// // Using raw value
    /// camera.set_focus(Raw(0x8000))?;
    ///
    /// // Using typed position
    /// camera.set_focus(FocusPosition::new(0x7000)?)?;
    /// ```
    pub fn set_focus<F>(value: F) where { F: TryInto<FocusPosition>, F::Error: Into<crate::Error> } => {
        let position = value.try_into().map_err(Into::into)?;
        FocusCommand::Position(position)
    };
}

// Note: set_iris method is implemented separately below to support generic IntoIrisLevel trait

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

// Semantic API Methods - Unit Conversions
camera_commands! {}

// Generic parameter methods that require special handling
// These cannot be handled by the camera_commands! macro due to generic trait bounds

#[cfg(not(feature = "async"))]
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
    T: crate::transport::blocking::BlockingTransport,
{
    /// Set iris level with flexible parameter types.
    ///
    /// Accepts iris level as:
    /// - Raw u8 values (0x00-0x0C)
    /// - IrisLevel type for type safety
    /// - `Percentage<f32>` values (0.0-100.0)
    /// - FStop enum values (F1_8, F2_8, etc.)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_iris(0x09u8)?;
    ///
    /// // Using typed level
    /// camera.set_iris(IrisLevel::new(0x0B)?)?;
    ///
    /// // Using percentage
    /// camera.set_iris(Percentage(75.0))?;
    ///
    /// // Using F-stop
    /// camera.set_iris(FStop::F2_8)?;
    /// ```
    pub fn set_iris(
        &mut self,
        level: impl crate::types::IntoIrisLevel,
    ) -> Result<(), crate::Error> {
        let cmd = IrisCommand::SetAperture(level.into_iris_level()?);
        self.send_and_wait(&cmd)
    }
}

#[cfg(feature = "async")]
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
    T: crate::transport::AsyncTransport,
{
    /// Set iris level with flexible parameter types.
    ///
    /// Accepts iris level as:
    /// - Raw u8 values (0x00-0x0C)
    /// - IrisLevel type for type safety
    /// - `Percentage<f32>` values (0.0-100.0)
    /// - FStop enum values (F1_8, F2_8, etc.)
    ///
    /// # Examples
    /// ```ignore
    /// // Using raw value
    /// camera.set_iris(0x09u8).await?;
    ///
    /// // Using typed level
    /// camera.set_iris(IrisLevel::new(0x0B)?).await?;
    ///
    /// // Using percentage
    /// camera.set_iris(Percentage(75.0)).await?;
    ///
    /// // Using F-stop
    /// camera.set_iris(FStop::F2_8).await?;
    /// ```
    pub async fn set_iris(
        &self,
        level: impl crate::types::IntoIrisLevel,
    ) -> Result<(), crate::Error> {
        let cmd = IrisCommand::SetAperture(level.into_iris_level()?);
        self.send_and_wait(&cmd).await
    }
}
