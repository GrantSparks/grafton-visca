//! Test data builders for creating complex command objects easily.
//!
//! These builders help create test data with sensible defaults while
//! avoiding repetitive unwrap() calls.
#![allow(dead_code)] // These utilities are for future test use

#[cfg(feature = "blocking-client")]
use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        preset::{PresetCommand, PresetNumber},
        zoom::{ZoomCommand, ZoomSpeed},
    },
    BrightnessLevel, ContrastLevel, GainLimit, GainValue, IrisLevel, SharpnessLevel, ShutterSpeed,
};

/// Builder for creating `PanTiltCommand` instances in tests.
#[cfg(feature = "blocking-client")]
pub struct TestPanTiltBuilder {
    pan: i16,
    tilt: i16,
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

#[cfg(feature = "blocking-client")]
impl TestPanTiltBuilder {
    /// Create a new builder with default values.
    pub fn new() -> Self {
        Self {
            pan: 0,
            tilt: 0,
            pan_speed: PanSpeed::new(10).expect("Default pan speed should be valid"),
            tilt_speed: TiltSpeed::new(10).expect("Default tilt speed should be valid"),
        }
    }

    /// Set the pan and tilt position.
    pub fn with_position(mut self, pan: i16, tilt: i16) -> Self {
        self.pan = pan;
        self.tilt = tilt;
        self
    }

    /// Set the pan speed.
    pub fn with_pan_speed(mut self, speed: u8) -> Self {
        self.pan_speed =
            PanSpeed::new(speed).unwrap_or_else(|_| panic!("Pan speed {} should be valid", speed));
        self
    }

    /// Set the tilt speed.
    pub fn with_tilt_speed(mut self, speed: u8) -> Self {
        self.tilt_speed = TiltSpeed::new(speed)
            .unwrap_or_else(|_| panic!("Tilt speed {} should be valid", speed));
        self
    }

    /// Set both speeds at once.
    pub fn with_speeds(mut self, pan_speed: u8, tilt_speed: u8) -> Self {
        self.pan_speed = PanSpeed::new(pan_speed)
            .unwrap_or_else(|_| panic!("Pan speed {} should be valid", pan_speed));
        self.tilt_speed = TiltSpeed::new(tilt_speed)
            .unwrap_or_else(|_| panic!("Tilt speed {} should be valid", tilt_speed));
        self
    }

    /// Build an absolute position command.
    pub fn build_absolute(self) -> PanTiltCommand {
        PanTiltCommand::AbsolutePosition {
            pan: self.pan,
            tilt: self.tilt,
            pan_speed: self.pan_speed,
            tilt_speed: self.tilt_speed,
        }
    }

    /// Build a relative position command.
    pub fn build_relative(self) -> PanTiltCommand {
        PanTiltCommand::RelativePosition {
            pan: self.pan,
            tilt: self.tilt,
            pan_speed: self.pan_speed,
            tilt_speed: self.tilt_speed,
        }
    }

    /// Build a move command with a direction.
    pub fn build_move(self, direction: PanTiltDirection) -> PanTiltCommand {
        PanTiltCommand::Move {
            direction,
            pan_speed: self.pan_speed,
            tilt_speed: self.tilt_speed,
        }
    }

    /// Build a home command.
    pub fn build_home() -> PanTiltCommand {
        PanTiltCommand::Home
    }

    /// Build a stop command using Move with Stop direction.
    pub fn build_stop() -> PanTiltCommand {
        PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0).expect("Speed 0 should be valid"),
            tilt_speed: TiltSpeed::new(0).expect("Speed 0 should be valid"),
        }
    }
}

#[cfg(feature = "blocking-client")]
impl Default for TestPanTiltBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating preset commands in tests.
#[cfg(feature = "blocking-client")]
pub struct TestPresetBuilder {
    number: PresetNumber,
}

#[cfg(feature = "blocking-client")]
impl TestPresetBuilder {
    /// Create a new preset builder with preset 0.
    pub fn new() -> Self {
        Self {
            number: PresetNumber::new(0).expect("Preset 0 should be valid"),
        }
    }

    /// Set the preset number.
    pub fn with_number(mut self, number: u8) -> Self {
        self.number = PresetNumber::new(number)
            .unwrap_or_else(|_| panic!("Preset {} should be valid", number));
        self
    }

    /// Build a recall preset command.
    pub fn build_recall(self) -> PresetCommand {
        PresetCommand {
            action: grafton_visca::command::preset::PresetAction::Recall,
            preset_number: self.number,
        }
    }

    /// Build a set preset command.
    pub fn build_set(self) -> PresetCommand {
        PresetCommand {
            action: grafton_visca::command::preset::PresetAction::Set,
            preset_number: self.number,
        }
    }

    /// Build a reset preset command.
    pub fn build_reset(self) -> PresetCommand {
        PresetCommand {
            action: grafton_visca::command::preset::PresetAction::Reset,
            preset_number: self.number,
        }
    }
}

/// Builder for creating zoom commands in tests.
#[cfg(feature = "blocking-client")]
pub struct TestZoomBuilder {
    position: u16,
    speed: Option<ZoomSpeed>,
}

#[cfg(feature = "blocking-client")]
impl TestZoomBuilder {
    /// Create a new zoom builder.
    pub fn new() -> Self {
        Self {
            position: 0,
            speed: None,
        }
    }

    /// Set the zoom position.
    pub fn with_position(mut self, position: u16) -> Self {
        self.position = position;
        self
    }

    /// Set the zoom speed.
    pub fn with_speed(mut self, speed: u8) -> Self {
        self.speed = Some(
            ZoomSpeed::new(speed)
                .unwrap_or_else(|_| panic!("Zoom speed {} should be valid", speed)),
        );
        self
    }

    /// Build a direct zoom position command.
    pub fn build_direct(self) -> ZoomCommand {
        ZoomCommand::Direct(self.position)
    }

    /// Build a zoom in command.
    pub fn build_zoom_in(self) -> ZoomCommand {
        if let Some(speed) = self.speed {
            ZoomCommand::ZoomInVariable(speed)
        } else {
            ZoomCommand::ZoomInStandard
        }
    }

    /// Build a zoom out command.
    pub fn build_zoom_out(self) -> ZoomCommand {
        if let Some(speed) = self.speed {
            ZoomCommand::ZoomOutVariable(speed)
        } else {
            ZoomCommand::ZoomOutStandard
        }
    }

    /// Build a stop command.
    pub fn build_stop() -> ZoomCommand {
        ZoomCommand::Stop
    }
}

/// Helpers for creating test parameter types.
pub struct TestParameters;

impl TestParameters {
    /// Create a valid brightness level for tests.
    pub fn brightness(level: u16) -> BrightnessLevel {
        BrightnessLevel::new(level)
            .unwrap_or_else(|_| panic!("Brightness level {} should be valid", level))
    }

    /// Create a valid contrast level for tests.
    pub fn contrast(level: u8) -> ContrastLevel {
        ContrastLevel::new(level)
            .unwrap_or_else(|_| panic!("Contrast level {} should be valid", level))
    }

    /// Create a valid sharpness level for tests.
    pub fn sharpness(level: u8) -> SharpnessLevel {
        SharpnessLevel::new(level)
            .unwrap_or_else(|_| panic!("Sharpness level {} should be valid", level))
    }

    /// Create a valid gain value for tests.
    pub fn gain(value: u8) -> GainValue {
        GainValue::new(value).unwrap_or_else(|_| panic!("Gain value {} should be valid", value))
    }

    /// Create a valid gain limit for tests.
    pub fn gain_limit(limit: u8) -> GainLimit {
        GainLimit::new(limit).unwrap_or_else(|_| panic!("Gain limit {} should be valid", limit))
    }

    /// Create a valid iris level for tests.
    pub fn iris(level: u8) -> IrisLevel {
        IrisLevel::new(level).unwrap_or_else(|_| panic!("Iris level {} should be valid", level))
    }

    /// Create a valid shutter speed for tests.
    pub fn shutter(speed: u16) -> ShutterSpeed {
        ShutterSpeed::new(speed)
            .unwrap_or_else(|_| panic!("Shutter speed {} should be valid", speed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_pan_tilt_builder() {
        let cmd = TestPanTiltBuilder::new()
            .with_position(100, 200)
            .with_speeds(15, 20)
            .build_absolute();

        match cmd {
            PanTiltCommand::AbsolutePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                assert_eq!(pan, 100);
                assert_eq!(tilt, 200);
                assert_eq!(pan_speed.value(), 15);
                assert_eq!(tilt_speed.value(), 20);
            }
            _ => panic!("Expected AbsolutePosition command"),
        }
    }

    #[cfg(feature = "blocking-client")]
    #[test]
    fn test_preset_builder() {
        let cmd = TestPresetBuilder::new().with_number(5).build_recall();

        match cmd {
            PresetCommand {
                action: grafton_visca::command::preset::PresetAction::Recall,
                preset_number,
            } => {
                assert_eq!(preset_number.value(), 5);
            }
            _ => panic!("Expected Recall command"),
        }
    }

    #[test]
    fn test_parameters() {
        assert_eq!(TestParameters::brightness(10).value(), 10);
        assert_eq!(TestParameters::contrast(14).value(), 14); // Max is 14
        assert_eq!(TestParameters::sharpness(5).value(), 5);
    }
}
