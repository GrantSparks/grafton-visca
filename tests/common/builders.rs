#![allow(missing_docs)]
//! Test data builders for creating complex command objects easily.
//!
//! These builders help create test data with sensible defaults while
//! avoiding repetitive unwrap() calls.
#![allow(dead_code)] // These utilities are for future test use

#[cfg(not(feature = "async"))]
use grafton_visca::{
    command::{
        pan_tilt::{PanTilt, PanTiltDirection},
        preset::PresetNumber,
        zoom::ZoomSpeed,
        Zoom,
    },
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed, ZoomPosition},
};

/// Builder for creating `PanTilt` instances in tests.
#[cfg(not(feature = "async"))]
pub struct TestPanTiltBuilder {
    pan: i16,
    tilt: i16,
    pan_speed: PanSpeed,
    tilt_speed: TiltSpeed,
}

#[cfg(not(feature = "async"))]
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
    pub fn build_absolute(self) -> PanTilt {
        PanTilt::AbsolutePosition {
            pan: PanPosition::new(self.pan).expect("Valid pan position"),
            tilt: TiltPosition::new(self.tilt).expect("Valid tilt position"),
            pan_speed: self.pan_speed,
            tilt_speed: self.tilt_speed,
        }
    }

    /// Build a relative position command.
    pub fn build_relative(self) -> PanTilt {
        PanTilt::RelativePosition {
            pan: PanPosition::new(self.pan).expect("Valid pan position"),
            tilt: TiltPosition::new(self.tilt).expect("Valid tilt position"),
            pan_speed: self.pan_speed,
            tilt_speed: self.tilt_speed,
        }
    }

    /// Build a move command with a direction.
    pub fn build_move(self, direction: PanTiltDirection) -> PanTilt {
        PanTilt::Move {
            direction,
            pan_speed: self.pan_speed,
            tilt_speed: self.tilt_speed,
        }
    }

    /// Build a home command.
    pub fn build_home() -> PanTilt {
        PanTilt::Home
    }

    /// Build a stop command using Move with Stop direction.
    pub fn build_stop() -> PanTilt {
        PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0).expect("Speed 0 should be valid"),
            tilt_speed: TiltSpeed::new(0).expect("Speed 0 should be valid"),
        }
    }
}

#[cfg(not(feature = "async"))]
impl Default for TestPanTiltBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating preset commands in tests.
#[cfg(not(feature = "async"))]
pub struct TestPresetBuilder {
    number: PresetNumber,
}

#[cfg(not(feature = "async"))]
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
}

/// Builder for creating zoom commands in tests.
#[cfg(not(feature = "async"))]
pub struct TestZoomBuilder {
    position: u16,
    speed: Option<ZoomSpeed>,
}

#[cfg(not(feature = "async"))]
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

    /// Build a zoom position command.
    pub fn build_position(self) -> Zoom {
        Zoom::Position(ZoomPosition::new(self.position).expect("Valid zoom position"))
    }

    /// Build a zoom in command.
    pub fn build_zoom_in(self) -> Zoom {
        if let Some(speed) = self.speed {
            Zoom::TeleVariable(speed)
        } else {
            Zoom::TeleStd
        }
    }

    /// Build a zoom out command.
    pub fn build_zoom_out(self) -> Zoom {
        if let Some(speed) = self.speed {
            Zoom::WideVariable(speed)
        } else {
            Zoom::WideStd
        }
    }

    /// Build a stop command.
    pub fn build_stop() -> Zoom {
        Zoom::Stop
    }
}

// Note: TestParameters helpers removed since those types are no longer part of the public API
// Tests that need these types should construct command objects directly

#[cfg(all(test, not(feature = "async")))]
mod tests {
    use super::*;

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_pan_tilt_builder() {
        let cmd = TestPanTiltBuilder::new()
            .with_position(100, 200)
            .with_speeds(15, 20)
            .build_absolute();

        match cmd {
            PanTilt::AbsolutePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                assert_eq!(pan.value(), 100);
                assert_eq!(tilt.value(), 200);
                assert_eq!(pan_speed.value(), 15);
                assert_eq!(tilt_speed.value(), 20);
            }
            _ => panic!("Expected AbsolutePosition command"),
        }
    }

    // TestParameters test removed since those types are no longer public
}
