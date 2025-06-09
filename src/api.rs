//! High-level, ergonomic API for VISCA camera control.
//!
//! This module provides a user-friendly interface that wraps the lower-level
//! VISCA commands with intuitive methods and type-safe builders.

use crate::{
    command::{
        exposure::{ExposureCommand, ExposureMode, IrisCommand},
        focus::{FocusCommand, FocusSpeed},
        gain::{AntiFlickerCommand, AntiFlickerMode, GainCommand},
        image::{NoiseReduction2DCommand, NoiseReduction3DCommand},
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        preset::{PresetAction, PresetCommand, PresetNumber},
        zoom::ZoomSpeed,
    },
    error::Error as ViscaError,
    types::{GainValue, NoiseReduction2DLevel, NoiseReduction3DLevel},
    Transport,
};

/// Speed level for camera movements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    /// Slowest speed (good for precise adjustments)
    Slowest,
    /// Slow speed
    Slow,
    /// Medium speed (default for most operations)
    Medium,
    /// Fast speed
    Fast,
    /// Fastest speed (may cause jerky movements)
    Fastest,
}

impl Speed {
    /// Convert to pan speed value.
    ///
    /// # Errors
    /// Returns `ViscaError` if the speed value is invalid.
    pub fn to_pan_speed(self) -> Result<PanSpeed, ViscaError> {
        match self {
            Self::Slowest => PanSpeed::new(0x01),
            Self::Slow => PanSpeed::new(0x08),
            Self::Medium => PanSpeed::new(0x10),
            Self::Fast | Self::Fastest => PanSpeed::new(0x18), // Max pan speed
        }
    }

    /// Convert to tilt speed value.
    ///
    /// # Errors
    /// Returns `ViscaError` if the speed value is invalid.
    pub fn to_tilt_speed(self) -> Result<TiltSpeed, ViscaError> {
        match self {
            Self::Slowest => TiltSpeed::new(0x01),
            Self::Slow => TiltSpeed::new(0x06),
            Self::Medium => TiltSpeed::new(0x0C),
            Self::Fast => TiltSpeed::new(0x12),
            Self::Fastest => TiltSpeed::new(0x14), // Max tilt speed
        }
    }

    /// Convert to zoom speed value.
    ///
    /// # Errors
    /// Returns `ViscaError` if the speed value is invalid.
    pub fn to_zoom_speed(self) -> Result<ZoomSpeed, ViscaError> {
        match self {
            Self::Slowest => ZoomSpeed::new(0x00),
            Self::Slow => ZoomSpeed::new(0x02),
            Self::Medium => ZoomSpeed::new(0x04),
            Self::Fast => ZoomSpeed::new(0x06),
            Self::Fastest => ZoomSpeed::new(0x07), // Max zoom speed
        }
    }

    /// Convert to focus speed value.
    ///
    /// # Errors
    /// Returns `ViscaError` if the speed value is invalid.
    pub fn to_focus_speed(self) -> Result<FocusSpeed, ViscaError> {
        match self {
            Self::Slowest => FocusSpeed::new(0x00),
            Self::Slow => FocusSpeed::new(0x02),
            Self::Medium => FocusSpeed::new(0x04),
            Self::Fast => FocusSpeed::new(0x06),
            Self::Fastest => FocusSpeed::new(0x07), // Max focus speed
        }
    }
}

/// Gain level presets for common scenarios.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GainLevel {
    /// Minimum gain (0 dB) - best image quality, needs good lighting
    Min,
    /// Low gain (6 dB)
    Low,
    /// Medium gain (12 dB)
    Medium,
    /// High gain (18 dB)
    High,
    /// Maximum gain (24 dB) - brightest but noisiest
    Max,
    /// Custom gain value (0-7)
    Custom(u8),
}

impl GainLevel {
    /// Convert to gain value.
    ///
    /// # Errors
    /// Returns `ViscaError` if the gain value is invalid.
    pub fn to_gain_value(self) -> Result<GainValue, ViscaError> {
        match self {
            Self::Min => GainValue::new(0x00),
            Self::Low => GainValue::new(0x02),
            Self::Medium => GainValue::new(0x04),
            Self::High => GainValue::new(0x06),
            Self::Max => GainValue::new(0x07),
            Self::Custom(val) => GainValue::new(val),
        }
    }
}

/// Noise reduction strength levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoiseReductionStrength {
    /// Disable noise reduction
    Off,
    /// Minimal noise reduction (preserves detail)
    Minimal,
    /// Light noise reduction
    Light,
    /// Medium noise reduction (balanced)
    Medium,
    /// Strong noise reduction
    Strong,
    /// Maximum noise reduction (may lose detail)
    Maximum,
}

impl NoiseReductionStrength {
    /// Convert to 2D noise reduction level.
    #[must_use]
    pub fn to_2d_level(self) -> Option<NoiseReduction2DLevel> {
        match self {
            Self::Off => None,
            Self::Minimal => Some(NoiseReduction2DLevel::new(1).ok()?),
            Self::Light => Some(NoiseReduction2DLevel::new(2).ok()?),
            Self::Medium => Some(NoiseReduction2DLevel::new(3).ok()?),
            Self::Strong => Some(NoiseReduction2DLevel::new(4).ok()?),
            Self::Maximum => Some(NoiseReduction2DLevel::new(5).ok()?),
        }
    }

    /// Convert to 3D noise reduction level.
    #[must_use]
    pub fn to_3d_level(self) -> Option<NoiseReduction3DLevel> {
        match self {
            Self::Off => None,
            Self::Minimal => Some(NoiseReduction3DLevel::new(1).ok()?),
            Self::Light => Some(NoiseReduction3DLevel::new(3).ok()?),
            Self::Medium => Some(NoiseReduction3DLevel::new(5).ok()?),
            Self::Strong => Some(NoiseReduction3DLevel::new(7).ok()?),
            Self::Maximum => Some(NoiseReduction3DLevel::new(8).ok()?),
        }
    }
}

/// Common iris F-stop values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrisValue {
    /// F1.8 (maximum aperture)
    F1_8,
    /// F2.0
    F2_0,
    /// F2.4
    F2_4,
    /// F2.8
    F2_8,
    /// F3.4
    F3_4,
    /// F4.0
    F4_0,
    /// F4.8
    F4_8,
    /// F5.6
    F5_6,
    /// F6.8
    F6_8,
    /// F8.0
    F8_0,
    /// F9.6
    F9_6,
    /// F11 (minimum aperture)
    F11,
    /// Fully closed
    Close,
}

impl IrisValue {
    /// Convert to VISCA iris position value.
    #[must_use]
    pub const fn to_position(self) -> u8 {
        match self {
            Self::Close => 0x00,
            Self::F11 => 0x05,
            Self::F9_6 => 0x06,
            Self::F8_0 => 0x07,
            Self::F6_8 => 0x08,
            Self::F5_6 => 0x09,
            Self::F4_8 => 0x0A,
            Self::F4_0 => 0x0B,
            Self::F3_4 => 0x0C,
            Self::F2_8 => 0x0D,
            Self::F2_4 => 0x0E,
            Self::F2_0 => 0x0F,
            Self::F1_8 => 0x10,
        }
    }
}

/// Builder for pan/tilt movements.
#[derive(Debug)]
pub struct PanTiltBuilder<'a, T: Transport> {
    device: &'a mut T,
    direction: Option<PanTiltDirection>,
    pan_speed: Option<PanSpeed>,
    tilt_speed: Option<TiltSpeed>,
}

impl<'a, T: Transport> PanTiltBuilder<'a, T> {
    /// Create a new pan/tilt builder.
    pub fn new(device: &'a mut T) -> Self {
        Self {
            device,
            direction: None,
            pan_speed: None,
            tilt_speed: None,
        }
    }

    /// Set movement direction.
    #[must_use]
    pub const fn direction(mut self, direction: PanTiltDirection) -> Self {
        self.direction = Some(direction);
        self
    }

    /// Move up.
    #[must_use]
    pub const fn up(self) -> Self {
        self.direction(PanTiltDirection::Up)
    }

    /// Move down.
    #[must_use]
    pub const fn down(self) -> Self {
        self.direction(PanTiltDirection::Down)
    }

    /// Move left.
    #[must_use]
    pub const fn left(self) -> Self {
        self.direction(PanTiltDirection::Left)
    }

    /// Move right.
    #[must_use]
    pub const fn right(self) -> Self {
        self.direction(PanTiltDirection::Right)
    }

    /// Move up-left.
    #[must_use]
    pub const fn up_left(self) -> Self {
        self.direction(PanTiltDirection::UpLeft)
    }

    /// Move up-right.
    #[must_use]
    pub const fn up_right(self) -> Self {
        self.direction(PanTiltDirection::UpRight)
    }

    /// Move down-left.
    #[must_use]
    pub const fn down_left(self) -> Self {
        self.direction(PanTiltDirection::DownLeft)
    }

    /// Move down-right.
    #[must_use]
    pub const fn down_right(self) -> Self {
        self.direction(PanTiltDirection::DownRight)
    }

    /// Set movement speed.
    ///
    /// # Errors
    /// Returns `ViscaError` if the speed values are invalid.
    pub fn speed(mut self, speed: Speed) -> Result<Self, ViscaError> {
        self.pan_speed = Some(speed.to_pan_speed()?);
        self.tilt_speed = Some(speed.to_tilt_speed()?);
        Ok(self)
    }

    /// Set pan speed specifically.
    ///
    /// # Errors
    /// Returns `ViscaError` if the pan speed value is invalid.
    pub fn pan_speed(mut self, speed: Speed) -> Result<Self, ViscaError> {
        self.pan_speed = Some(speed.to_pan_speed()?);
        Ok(self)
    }

    /// Set tilt speed specifically.
    ///
    /// # Errors
    /// Returns `ViscaError` if the tilt speed value is invalid.
    pub fn tilt_speed(mut self, speed: Speed) -> Result<Self, ViscaError> {
        self.tilt_speed = Some(speed.to_tilt_speed()?);
        Ok(self)
    }

    /// Execute the movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed or if no direction was specified.
    ///
    /// # Panics
    /// Panics if default speed values (0x10) are invalid, which should never happen.
    pub fn execute(self) -> Result<(), ViscaError> {
        let direction = self.direction.ok_or_else(|| {
            ViscaError::InvalidParameter("Direction must be specified".to_string())
        })?;

        let pan_speed = self.pan_speed.unwrap_or(PanSpeed::DEFAULT_MEDIUM);
        let tilt_speed = self.tilt_speed.unwrap_or(TiltSpeed::DEFAULT_MEDIUM);

        self.device.execute_command(&PanTiltCommand::Move {
            direction,
            pan_speed,
            tilt_speed,
        })?;

        Ok(())
    }

    /// Stop all movement.
    ///
    /// # Errors
    /// Returns `ViscaError` if the stop command cannot be executed.
    pub fn stop(self) -> Result<(), ViscaError> {
        self.device.execute_command(&PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::ZERO,
            tilt_speed: TiltSpeed::ZERO,
        })?;
        Ok(())
    }

    /// Move to home position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the home command cannot be executed.
    pub fn home(self) -> Result<(), ViscaError> {
        self.device.execute_command(&PanTiltCommand::Home)?;
        Ok(())
    }
}

/// High-level camera control extension trait.
pub trait CameraControl: Transport {
    /// Create a pan/tilt movement builder.
    fn pan_tilt(&mut self) -> PanTiltBuilder<Self>
    where
        Self: Sized,
    {
        PanTiltBuilder::new(self)
    }

    /// Recall a saved preset position.
    ///
    /// # Errors
    /// Returns `ViscaError` if the preset number is invalid or command cannot be executed.
    fn recall_preset(&mut self, preset_number: u8) -> Result<(), ViscaError> {
        let preset_num = PresetNumber::new(preset_number)?;
        self.execute_command(&PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset_num,
        })?;
        Ok(())
    }

    /// Set focus mode to auto.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn focus_auto(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&FocusCommand::Auto)?;
        Ok(())
    }

    /// Set focus mode to manual.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn focus_manual(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&FocusCommand::Manual)?;
        Ok(())
    }

    /// Focus near with speed control.
    ///
    /// # Errors
    /// Returns `ViscaError` if the speed is invalid or command cannot be executed.
    fn focus_near(&mut self, speed: Speed) -> Result<(), ViscaError> {
        let focus_speed = speed.to_focus_speed()?;
        self.execute_command(&FocusCommand::NearVariable(focus_speed))?;
        Ok(())
    }

    /// Focus far with speed control.
    ///
    /// # Errors
    /// Returns `ViscaError` if the speed is invalid or command cannot be executed.
    fn focus_far(&mut self, speed: Speed) -> Result<(), ViscaError> {
        let focus_speed = speed.to_focus_speed()?;
        self.execute_command(&FocusCommand::FarVariable(focus_speed))?;
        Ok(())
    }

    /// Set gain level using intuitive presets.
    ///
    /// # Errors
    /// Returns `ViscaError` if the gain level is invalid or command cannot be executed.
    fn set_gain(&mut self, level: GainLevel) -> Result<(), ViscaError> {
        let gain_value = level.to_gain_value()?;
        self.execute_command(&GainCommand::Direct(gain_value))?;
        Ok(())
    }

    /// Set exposure mode with validation.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn set_exposure_mode(&mut self, mode: ExposureMode) -> Result<(), ViscaError> {
        self.execute_command(&ExposureCommand { mode })?;
        Ok(())
    }

    /// Set iris using F-stop values.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn set_iris(&mut self, iris: IrisValue) -> Result<(), ViscaError> {
        use crate::types::IrisLevel;
        self.execute_command(&IrisCommand::Direct(IrisLevel::new(iris.to_position())?))?;
        Ok(())
    }

    /// Set 2D noise reduction with intuitive strength levels.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn set_noise_reduction_2d(
        &mut self,
        strength: NoiseReductionStrength,
    ) -> Result<(), ViscaError> {
        let command = strength
            .to_2d_level()
            .map_or(NoiseReduction2DCommand::Off, NoiseReduction2DCommand::Level);
        self.execute_command(&command)?;
        Ok(())
    }

    /// Set 3D noise reduction with intuitive strength levels.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn set_noise_reduction_3d(
        &mut self,
        strength: NoiseReductionStrength,
    ) -> Result<(), ViscaError> {
        let command = strength
            .to_3d_level()
            .map_or(NoiseReduction3DCommand::Off, NoiseReduction3DCommand::Level);
        self.execute_command(&command)?;
        Ok(())
    }

    /// Set anti-flicker mode for your region.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn set_anti_flicker_50hz(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&AntiFlickerCommand {
            mode: AntiFlickerMode::Hz50,
        })?;
        Ok(())
    }

    /// Set anti-flicker mode for your region.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn set_anti_flicker_60hz(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&AntiFlickerCommand {
            mode: AntiFlickerMode::Hz60,
        })?;
        Ok(())
    }

    /// Disable anti-flicker.
    ///
    /// # Errors
    /// Returns `ViscaError` if the command cannot be executed.
    fn disable_anti_flicker(&mut self) -> Result<(), ViscaError> {
        self.execute_command(&AntiFlickerCommand {
            mode: AntiFlickerMode::Off,
        })?;
        Ok(())
    }
}

// Implement for all types that implement Transport
impl<T: Transport> CameraControl for T {}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_conversions() {
        // Test pan speed conversion
        assert!(Speed::Slowest.to_pan_speed().is_ok());
        assert!(Speed::Medium.to_pan_speed().is_ok());
        assert!(Speed::Fastest.to_pan_speed().is_ok());

        // Test that speeds are ordered correctly
        let slowest = Speed::Slowest.to_pan_speed().unwrap();
        let medium = Speed::Medium.to_pan_speed().unwrap();
        let fastest = Speed::Fastest.to_pan_speed().unwrap();

        assert!(slowest.value() < medium.value());
        assert!(medium.value() < fastest.value());
    }

    #[test]
    fn test_gain_level_conversions() {
        assert_eq!(
            GainLevel::Min
                .to_gain_value()
                .expect("Min gain should be valid")
                .value(),
            0x00
        );
        assert_eq!(
            GainLevel::Max
                .to_gain_value()
                .expect("Max gain should be valid")
                .value(),
            0x07
        );
        assert_eq!(
            GainLevel::Custom(0x05)
                .to_gain_value()
                .expect("Custom gain should be valid")
                .value(),
            0x05
        );

        // Test invalid custom value
        assert!(GainLevel::Custom(0x08).to_gain_value().is_err());
    }

    #[test]
    fn test_noise_reduction_strength() {
        // Test 2D levels
        assert!(NoiseReductionStrength::Off.to_2d_level().is_none());
        assert_eq!(
            NoiseReductionStrength::Minimal
                .to_2d_level()
                .unwrap()
                .value(),
            1
        );
        assert_eq!(
            NoiseReductionStrength::Maximum
                .to_2d_level()
                .unwrap()
                .value(),
            5
        );

        // Test 3D levels
        assert!(NoiseReductionStrength::Off.to_3d_level().is_none());
        assert_eq!(
            NoiseReductionStrength::Minimal
                .to_3d_level()
                .unwrap()
                .value(),
            1
        );
        assert_eq!(
            NoiseReductionStrength::Maximum
                .to_3d_level()
                .unwrap()
                .value(),
            8
        );
    }

    #[test]
    fn test_iris_values() {
        assert_eq!(IrisValue::Close.to_position(), 0x00);
        assert_eq!(IrisValue::F4_0.to_position(), 0x0B);
        assert_eq!(IrisValue::F1_8.to_position(), 0x10);
    }
}
