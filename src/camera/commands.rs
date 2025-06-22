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
        BlueGain, BlueTuning, BrightnessLevel, ColorTemperature, ContrastLevel, FocusPosition,
        GainLimit, GainValue, HueLevel, IrisLevel, LuminanceLevel, NoiseReduction2DLevel,
        NoiseReduction3DLevel, RedGain, RedTuning, SaturationLevel, SharpnessLevel, ShutterSpeed,
        ZoomPosition,
    },
};

use super::{Camera, CameraProfile};
use crate::units::{Degrees, Normalized};

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

    /// Stop all camera movement.
    ///
    /// This is a convenience method that sends a pan-tilt move command with the
    /// Stop direction and zero speeds, which halts any ongoing movement.
    pub fn stop(&self) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: crate::types::PanSpeed::new(0)?,
            tilt_speed: crate::types::TiltSpeed::new(0)?,
        })
    }

    /// Reset pan and tilt to home position.
    pub fn home(&self) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::Home)
    }

    /// Reset pan and tilt motors.
    ///
    /// This recalibrates the pan/tilt position sensors. The camera will perform
    /// a full range motion to determine its limits.
    pub fn reset_pan_tilt(&self) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::Reset)
    }

    /// Recall a stored preset position.
    ///
    /// Moves the camera to a previously saved preset position. The camera
    /// will move its pan, tilt, zoom, and focus to the stored values.
    pub fn recall_preset(&self, preset_id: u8) -> Result<(), Error> {
        self.send_and_wait(&PresetCommand::new::<P>(PresetAction::Recall, P::PresetId::try_from(preset_id)?)?)
    }

    /// Save current position as a preset.
    ///
    /// Stores the current pan, tilt, zoom, and focus positions to the
    /// specified preset number for later recall.
    pub fn set_preset(&self, preset_id: u8) -> Result<(), Error> {
        self.send_and_wait(&PresetCommand::new::<P>(PresetAction::Set, P::PresetId::try_from(preset_id)?)?)
    }

    /// Start zooming in (telephoto direction).
    pub fn zoom_in(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomInStandard)
    }

    /// Start zooming out (wide direction).
    pub fn zoom_out(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::ZoomOutStandard)
    }

    /// Stop zoom movement.
    pub fn zoom_stop(&self) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::Stop)
    }

    /// Set zoom to direct position.
    pub fn zoom_direct(&self, position: ZoomPosition) -> Result<(), Error> {
        self.send_and_wait(&ZoomCommand::Direct(position))
    }

    // Digital zoom functionality not yet implemented in ZoomCommand enum

    /// Focus on a near object.
    pub fn focus_near(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::FocusNearStandard)
    }

    /// Focus on a far object.
    pub fn focus_far(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::FocusFarStandard)
    }

    /// Stop focus adjustment.
    pub fn focus_stop(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Stop)
    }

    /// Set focus to direct position (manual mode).
    pub fn focus_direct(&self, position: FocusPosition) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Direct(position))
    }

    /// Set focus to auto mode.
    pub fn focus_auto(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Auto)
    }

    /// Set focus to manual mode.
    pub fn focus_manual(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::Manual)
    }

    /// Trigger one-push autofocus.
    ///
    /// The camera will perform a single autofocus operation and then
    /// return to manual focus mode.
    pub fn focus_one_push_trigger(&self) -> Result<(), Error> {
        self.send_and_wait(&FocusCommand::OnePushTrigger)
    }

    /// Set white balance mode.
    pub fn set_white_balance_mode(&self, mode: WhiteBalanceMode) -> Result<(), Error> {
        self.send_and_wait(&WhiteBalanceCommand { mode })
    }

    /// Trigger one-push white balance.
    ///
    /// The camera will measure the white balance on the current scene
    /// and apply the correction.
    pub fn white_balance_one_push_trigger(&self) -> Result<(), Error> {
        self.send_and_wait(&OnePushTriggerCommand)
    }

    /// Set exposure mode.
    pub fn set_exposure_mode(&self, mode: ExposureMode) -> Result<(), Error> {
        self.send_and_wait(&ExposureCommand { mode })
    }

    /// Set iris level directly.
    pub fn iris_direct(&self, level: IrisLevel) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Direct(level))
    }

    /// Reset iris to default position.
    pub fn iris_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&IrisCommand::Reset)
    }

    /// Set shutter speed directly.
    pub fn shutter_direct(&self, speed: ShutterSpeed) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Direct(speed))
    }

    /// Reset shutter to default speed.
    pub fn shutter_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ShutterCommand::Reset)
    }

    /// Enable or disable backlight compensation.
    pub fn set_backlight(&self, enabled: bool) -> Result<(), Error> {
        self.send_and_wait(&BacklightCommand { status: enabled })
    }

    /// Set anti-flicker mode.
    pub fn set_anti_flicker(&self, mode: AntiFlickerMode) -> Result<(), Error> {
        self.send_and_wait(&AntiFlickerCommand { mode })
    }

    /// Set dynamic range (HDR).
    pub fn set_dynamic_range(&self, level: DynamicRangeLevel) -> Result<(), Error> {
        self.send_and_wait(&DynamicRangeCommand::Direct(level))
    }

    /// Set luminance level.
    pub fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error> {
        self.send_and_wait(&LuminanceCommand { value: level })
    }

    /// Set contrast level.
    pub fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error> {
        self.send_and_wait(&ContrastCommand { value: level })
    }

    /// Reset contrast to default.
    pub fn contrast_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ContrastCommand { value: ContrastLevel::new(7)? })
    }

    /// Set sharpness level.
    pub fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Direct { value: level.value() })
    }

    /// Reset sharpness to default.
    pub fn sharpness_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Reset)
    }

    /// Set sharpness mode.
    pub fn set_sharpness_mode(&self, mode: SharpnessMode) -> Result<(), Error> {
        self.send_and_wait(&SharpnessCommand::Mode(mode))
    }

    /// Enable or disable black and white mode.
    pub fn set_black_white(&self, enabled: bool) -> Result<(), Error> {
        self.send_and_wait(&BlackWhiteCommand { on: enabled })
    }

    /// Set color saturation level.
    pub fn set_saturation(&self, level: SaturationLevel) -> Result<(), Error> {
        self.send_and_wait(&SaturationCommand { level })
    }

    /// Reset saturation to default.
    pub fn saturation_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&SaturationCommand { level: SaturationLevel::new(0x07)? })
    }

    /// Set hue level.
    pub fn set_hue(&self, level: HueLevel) -> Result<(), Error> {
        self.send_and_wait(&HueCommand { level })
    }

    /// Reset hue to default.
    pub fn hue_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&HueCommand { level: HueLevel::new(0x07)? })
    }

    /// Set color temperature.
    pub fn set_color_temperature(&self, temperature: ColorTemperature) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Direct(temperature))
    }

    /// Reset color temperature to default.
    pub fn color_temperature_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ColorTemperatureCommand::Reset)
    }

    /// Set red gain.
    pub fn set_red_gain(&self, gain: RedGain) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Direct(gain))
    }

    /// Reset red gain to default.
    pub fn red_gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&RedGainCommand::Reset)
    }

    /// Set blue gain.
    pub fn set_blue_gain(&self, gain: BlueGain) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Direct(gain))
    }

    /// Reset blue gain to default.
    pub fn blue_gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&BlueGainCommand::Reset)
    }

    /// Set red tuning level.
    pub fn set_red_tuning(&self, tuning: RedTuning) -> Result<(), Error> {
        self.send_and_wait(&RedTuningCommand { level: tuning })
    }

    /// Set blue tuning level.
    pub fn set_blue_tuning(&self, tuning: BlueTuning) -> Result<(), Error> {
        self.send_and_wait(&BlueTuningCommand { level: tuning })
    }

    /// Set image flip mode.
    pub fn set_flip(&self, mode: ImageFlipMode) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCombinedCommand { mode })
    }

    /// Set horizontal flip.
    pub fn set_flip_horizontal(&self, enabled: bool) -> Result<(), Error> {
        self.send_and_wait(&ImageFlipCommand { flip: if enabled { Flip::On } else { Flip::Off }})
    }

    /// Set auto gain control value.
    pub fn set_gain(&self, gain: GainValue) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Direct(gain))
    }

    /// Reset gain to default.
    pub fn gain_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&GainCommand::Reset)
    }

    /// Set gain limit.
    pub fn set_gain_limit(&self, limit: GainLimit) -> Result<(), Error> {
        self.send_and_wait(&GainLimitCommand { limit })
    }

    /// Set exposure compensation.
    pub fn set_exposure_compensation(&self, level: ExposureCompensationLevel) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Direct(level))
    }

    /// Reset exposure compensation.
    pub fn exposure_compensation_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Reset)
    }

    /// Enable exposure compensation.
    pub fn exposure_compensation_on(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::On)
    }

    /// Disable exposure compensation.
    pub fn exposure_compensation_off(&self) -> Result<(), Error> {
        self.send_and_wait(&ExposureCompensationCommand::Off)
    }

    /// Set brightness level directly.
    pub fn set_brightness(&self, level: BrightnessLevel) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Direct(level))
    }

    /// Reset brightness to default.
    pub fn brightness_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&BrightCommand::Reset)
    }

    /// Set 2D noise reduction level.
    pub fn set_noise_reduction_2d(&self, level: NoiseReduction2DLevel) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction2DCommand::Level(level))
    }

    /// Reset 2D noise reduction to default.
    pub fn noise_reduction_2d_reset(&self) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction2DCommand::Off)
    }

    /// Set 3D noise reduction level.
    pub fn set_noise_reduction_3d(&self, level: NoiseReduction3DLevel) -> Result<(), Error> {
        self.send_and_wait(&NoiseReduction3DCommand::Level(level))
    }

    /// Move to absolute pan/tilt position.
    ///
    /// Both pan and tilt positions are specified in normalized units (0.0 to 1.0).
    pub fn pan_tilt(&self, pan: f32, tilt: f32) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::absolute_position_normalized::<P>(Normalized::new(pan), Normalized::new(tilt))?)
    }

    /// Move to absolute position specified in degrees.
    ///
    /// Both pan and tilt angles are specified in degrees.
    pub fn pan_tilt_degrees(&self, pan_degrees: f32, tilt_degrees: f32) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::absolute_position_degrees::<P>(Degrees(pan_degrees), Degrees(tilt_degrees))?)
    }

    /// Move to absolute position specified in degrees.
    pub fn set_position(&self, pan: Degrees<f32>, tilt: Degrees<f32>) -> Result<(), Error> {
        self.send_and_wait(&PanTiltCommand::absolute_position_degrees::<P>(pan, tilt)?)
    }
}

// Generic parameter methods for enhanced API ergonomics
// These need manual implementation due to multi-statement bodies
#[cfg(feature = "async")]
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
    T: crate::transport::AsyncTransport,
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
    pub async fn move_continuous<PS, TS>(
        &self,
        direction: PanTiltDirection,
        pan_speed: PS,
        tilt_speed: TS,
    ) -> Result<(), crate::Error>
    where
        PS: TryInto<crate::types::PanSpeed>,
        PS::Error: Into<crate::Error>,
        TS: TryInto<crate::types::TiltSpeed>,
        TS::Error: Into<crate::Error>,
    {
        let pan_speed = pan_speed.try_into().map_err(Into::into)?;
        let tilt_speed = tilt_speed.try_into().map_err(Into::into)?;
        match self
            .send_command(&PanTiltCommand::Move {
                direction,
                pan_speed,
                tilt_speed,
            })
            .await?
        {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
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
    pub async fn set_zoom<Z>(&self, position: Z) -> Result<(), crate::Error>
    where
        Z: TryInto<ZoomPosition>,
        Z::Error: Into<crate::Error>,
    {
        let position = position.try_into().map_err(Into::into)?;
        match self.send_command(&ZoomCommand::Direct(position)).await? {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
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
    pub async fn set_focus<F>(&self, position: F) -> Result<(), crate::Error>
    where
        F: TryInto<FocusPosition>,
        F::Error: Into<crate::Error>,
    {
        let position = position.try_into().map_err(Into::into)?;
        match self.send_command(&FocusCommand::Direct(position)).await? {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
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
    pub async fn set_iris<I>(&self, level: I) -> Result<(), crate::Error>
    where
        I: TryInto<IrisLevel>,
        I::Error: Into<crate::Error>,
    {
        let level = level.try_into().map_err(Into::into)?;
        match self.send_command(&IrisCommand::Direct(level)).await? {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }
}

#[cfg(not(feature = "async"))]
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
    T: crate::transport::blocking::Transport,
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
    /// camera.move_continuous(PanTiltDirection::Right, 10u8, 8u8)?;
    ///
    /// // Using speed levels
    /// camera.move_continuous(PanTiltDirection::Up, SpeedLevel::Fast, SpeedLevel::Medium)?;
    ///
    /// // Using typed speeds
    /// camera.move_continuous(PanTiltDirection::DownLeft, PanSpeed::new(15)?, TiltSpeed::new(12)?)?;
    /// ```
    pub fn move_continuous<PS, TS>(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: PS,
        tilt_speed: TS,
    ) -> Result<(), crate::Error>
    where
        PS: TryInto<crate::types::PanSpeed>,
        PS::Error: Into<crate::Error>,
        TS: TryInto<crate::types::TiltSpeed>,
        TS::Error: Into<crate::Error>,
    {
        let pan_speed = pan_speed.try_into().map_err(Into::into)?;
        let tilt_speed = tilt_speed.try_into().map_err(Into::into)?;
        self.send_and_wait(&PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        })
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
    /// camera.set_zoom(0x3000u16)?;
    ///
    /// // Using normalized value (50% zoom)
    /// camera.set_zoom(0.5f32)?;
    ///
    /// // Using typed position
    /// camera.set_zoom(ZoomPosition::new(0x2000)?)?;
    /// ```
    pub fn set_zoom<Z>(&mut self, position: Z) -> Result<(), crate::Error>
    where
        Z: TryInto<ZoomPosition>,
        Z::Error: Into<crate::Error>,
    {
        let position = position.try_into().map_err(Into::into)?;
        self.send_and_wait(&ZoomCommand::Direct(position))
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
    /// camera.set_focus(0x8000u16)?;
    ///
    /// // Using normalized value (75% to near)
    /// camera.set_focus(0.75f32)?;
    ///
    /// // Using typed position
    /// camera.set_focus(FocusPosition::new(0x5000)?)?;
    /// ```
    pub fn set_focus<F>(&mut self, position: F) -> Result<(), crate::Error>
    where
        F: TryInto<FocusPosition>,
        F::Error: Into<crate::Error>,
    {
        let position = position.try_into().map_err(Into::into)?;
        self.send_and_wait(&FocusCommand::Direct(position))
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
    /// camera.set_iris(0x09u8)?;
    ///
    /// // Using F-stop enum
    /// camera.set_iris(FStop::F2_8)?;
    ///
    /// // Using typed level
    /// camera.set_iris(IrisLevel::new(0x0B)?)?;
    /// ```
    pub fn set_iris<I>(&mut self, level: I) -> Result<(), crate::Error>
    where
        I: TryInto<IrisLevel>,
        I::Error: Into<crate::Error>,
    {
        let level = level.try_into().map_err(Into::into)?;
        self.send_and_wait(&IrisCommand::Direct(level))
    }
}

// Additional semantic API methods for enhanced ergonomics
#[cfg(feature = "async")]
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
    T: crate::transport::AsyncTransport,
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
    pub async fn set_white_balance_kelvin(
        &self,
        kelvin: crate::units::Kelvin,
    ) -> Result<(), crate::Error> {
        let temperature = ColorTemperature::try_from(kelvin)?;
        match self
            .send_command(&ColorTemperatureCommand::Direct(temperature))
            .await?
        {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
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
    pub async fn set_shutter_fraction(
        &self,
        fraction: crate::units::Fraction,
    ) -> Result<(), crate::Error> {
        let speed = ShutterSpeed::try_from(fraction)?;
        match self.send_command(&ShutterCommand::Direct(speed)).await? {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }

    /// Set zoom using percentage.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Percentage;
    ///
    /// // Set to 50% zoom
    /// camera.set_zoom_percentage(Percentage(50.0)).await?;
    ///
    /// // Set to full zoom
    /// camera.set_zoom_percentage(Percentage(100.0)).await?;
    /// ```
    pub async fn set_zoom_percentage(
        &self,
        percentage: crate::units::Percentage<f32>,
    ) -> Result<(), crate::Error> {
        let position = ZoomPosition::try_from(percentage)?;
        match self.send_command(&ZoomCommand::Direct(position)).await? {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }

    /// Set zoom using magnification factor.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Magnification;
    ///
    /// // Set to 10x magnification
    /// camera.set_zoom_magnification(Magnification(10.0)).await?;
    ///
    /// // Set to 1x (no zoom)
    /// camera.set_zoom_magnification(Magnification(1.0)).await?;
    /// ```
    pub async fn set_zoom_magnification(
        &self,
        magnification: crate::units::Magnification<f32>,
    ) -> Result<(), crate::Error> {
        let position = ZoomPosition::try_from(magnification)?;
        match self.send_command(&ZoomCommand::Direct(position)).await? {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }

    /// Set focus using percentage (0% = infinity, 100% = near).
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Percentage;
    ///
    /// // Set to infinity focus
    /// camera.set_focus_percentage(Percentage(0.0)).await?;
    ///
    /// // Set to mid-range focus
    /// camera.set_focus_percentage(Percentage(50.0)).await?;
    /// ```
    pub async fn set_focus_percentage(
        &self,
        percentage: crate::units::Percentage<f32>,
    ) -> Result<(), crate::Error> {
        let position = FocusPosition::try_from(percentage)?;
        match self.send_command(&FocusCommand::Direct(position)).await? {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
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
    pub async fn set_position_radians(
        &self,
        pan: crate::units::Radians<f32>,
        tilt: crate::units::Radians<f32>,
    ) -> Result<(), crate::Error> {
        let pan_degrees: Degrees<f32> = pan.into();
        let tilt_degrees: Degrees<f32> = tilt.into();
        match self
            .send_command(&PanTiltCommand::absolute_position_degrees::<P>(
                pan_degrees,
                tilt_degrees,
            )?)
            .await?
        {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }

    /// Set iris using percentage (0% = closed, 100% = fully open).
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Percentage;
    ///
    /// // Set to 50% open
    /// camera.set_iris_percentage(Percentage(50.0)).await?;
    ///
    /// // Fully open iris
    /// camera.set_iris_percentage(Percentage(100.0)).await?;
    /// ```
    pub async fn set_iris_percentage(
        &self,
        percentage: crate::units::Percentage<f32>,
    ) -> Result<(), crate::Error> {
        let level = IrisLevel::try_from(percentage)?;
        match self.send_command(&IrisCommand::Direct(level)).await? {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
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
    pub async fn move_percentage(
        &self,
        direction: PanTiltDirection,
        pan_speed: crate::units::Percentage<f32>,
        tilt_speed: crate::units::Percentage<f32>,
    ) -> Result<(), crate::Error> {
        let pan = crate::types::PanSpeed::try_from(pan_speed)?;
        let tilt = crate::types::TiltSpeed::try_from(tilt_speed)?;
        match self
            .send_command(&PanTiltCommand::Move {
                direction,
                pan_speed: pan,
                tilt_speed: tilt,
            })
            .await?
        {
            crate::Response::Completion => Ok(()),
            crate::Response::Ack => Ok(()),
            response => Err(crate::Error::InvalidResponse {
                expected: "Completion".to_string(),
                actual: format!("{:?}", response).into_bytes(),
            }),
        }
    }
}

#[cfg(not(feature = "async"))]
impl<P, T> Camera<P, T>
where
    P: CameraProfile,
    T: crate::transport::blocking::Transport,
{
    /// Set white balance using color temperature in Kelvin.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Kelvin;
    ///
    /// // Set to daylight (5600K)
    /// camera.set_white_balance_kelvin(Kelvin(5600))?;
    ///
    /// // Set to tungsten (3200K)
    /// camera.set_white_balance_kelvin(Kelvin(3200))?;
    /// ```
    pub fn set_white_balance_kelvin(
        &mut self,
        kelvin: crate::units::Kelvin,
    ) -> Result<(), crate::Error> {
        let temperature = ColorTemperature::try_from(kelvin)?;
        self.send_and_wait(&ColorTemperatureCommand::Direct(temperature))
    }

    /// Set shutter speed using fraction notation.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Fraction;
    ///
    /// // Set to 1/60s
    /// camera.set_shutter_fraction(Fraction::new(1, 60))?;
    ///
    /// // Set to 1/1000s
    /// camera.set_shutter_fraction(Fraction::new(1, 1000))?;
    /// ```
    pub fn set_shutter_fraction(
        &mut self,
        fraction: crate::units::Fraction,
    ) -> Result<(), crate::Error> {
        let speed = ShutterSpeed::try_from(fraction)?;
        self.send_and_wait(&ShutterCommand::Direct(speed))
    }

    /// Set zoom using percentage.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Percentage;
    ///
    /// // Set to 50% zoom
    /// camera.set_zoom_percentage(Percentage(50.0))?;
    ///
    /// // Set to full zoom
    /// camera.set_zoom_percentage(Percentage(100.0))?;
    /// ```
    pub fn set_zoom_percentage(
        &mut self,
        percentage: crate::units::Percentage<f32>,
    ) -> Result<(), crate::Error> {
        let position = ZoomPosition::try_from(percentage)?;
        self.send_and_wait(&ZoomCommand::Direct(position))
    }

    /// Set zoom using magnification factor.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Magnification;
    ///
    /// // Set to 10x magnification
    /// camera.set_zoom_magnification(Magnification(10.0))?;
    ///
    /// // Set to 1x (no zoom)
    /// camera.set_zoom_magnification(Magnification(1.0))?;
    /// ```
    pub fn set_zoom_magnification(
        &mut self,
        magnification: crate::units::Magnification<f32>,
    ) -> Result<(), crate::Error> {
        let position = ZoomPosition::try_from(magnification)?;
        self.send_and_wait(&ZoomCommand::Direct(position))
    }

    /// Set focus using percentage (0% = infinity, 100% = near).
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Percentage;
    ///
    /// // Set to infinity focus
    /// camera.set_focus_percentage(Percentage(0.0))?;
    ///
    /// // Set to mid-range focus
    /// camera.set_focus_percentage(Percentage(50.0))?;
    /// ```
    pub fn set_focus_percentage(
        &mut self,
        percentage: crate::units::Percentage<f32>,
    ) -> Result<(), crate::Error> {
        let position = FocusPosition::try_from(percentage)?;
        self.send_and_wait(&FocusCommand::Direct(position))
    }

    /// Set pan/tilt position using radians.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Radians;
    /// use std::f32::consts::PI;
    ///
    /// // Turn 90 degrees right and 45 degrees up
    /// camera.set_position_radians(Radians(PI / 2.0), Radians(PI / 4.0))?;
    /// ```
    pub fn set_position_radians(
        &mut self,
        pan: crate::units::Radians<f32>,
        tilt: crate::units::Radians<f32>,
    ) -> Result<(), crate::Error> {
        let pan_degrees: Degrees<f32> = pan.into();
        let tilt_degrees: Degrees<f32> = tilt.into();
        self.send_and_wait(&PanTiltCommand::absolute_position_degrees::<P>(
            pan_degrees,
            tilt_degrees,
        )?)
    }

    /// Set iris using percentage (0% = closed, 100% = fully open).
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Percentage;
    ///
    /// // Set to 50% open
    /// camera.set_iris_percentage(Percentage(50.0))?;
    ///
    /// // Fully open iris
    /// camera.set_iris_percentage(Percentage(100.0))?;
    /// ```
    pub fn set_iris_percentage(
        &mut self,
        percentage: crate::units::Percentage<f32>,
    ) -> Result<(), crate::Error> {
        let level = IrisLevel::try_from(percentage)?;
        self.send_and_wait(&IrisCommand::Direct(level))
    }

    /// Move camera at percentage of maximum speed.
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::Percentage;
    /// use grafton_visca::command::pan_tilt::PanTiltDirection;
    ///
    /// // Move right at 50% speed
    /// camera.move_percentage(PanTiltDirection::Right, Percentage(50.0), Percentage(0.0))?;
    ///
    /// // Move diagonally at 75% speed
    /// camera.move_percentage(PanTiltDirection::UpRight, Percentage(75.0), Percentage(75.0))?;
    /// ```
    pub fn move_percentage(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: crate::units::Percentage<f32>,
        tilt_speed: crate::units::Percentage<f32>,
    ) -> Result<(), crate::Error> {
        let pan = crate::types::PanSpeed::try_from(pan_speed)?;
        let tilt = crate::types::TiltSpeed::try_from(tilt_speed)?;
        self.send_and_wait(&PanTiltCommand::Move {
            direction,
            pan_speed: pan,
            tilt_speed: tilt,
        })
    }
}
