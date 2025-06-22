//! Pan/Tilt control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera pan (horizontal) and tilt (vertical)
//! movement, including directional movement, absolute positioning, and relative positioning.
//!
//! # Speed Limits
//! - Pan speed: 0x00 to 0x18 (0-24 decimal)
//! - Tilt speed: 0x00 to 0x14 (0-20 decimal)
//!
//! # Example
//! ```ignore
//! # #[cfg(not(feature = "async"))]
//! # {
//! # use grafton_visca::command::pan_tilt::{PanTiltCommand, PanTiltDirection, PanSpeed, TiltSpeed};
//! # use grafton_visca::Client;
//! # let client = Client::connect_udp("192.168.1.100:5678").unwrap();
//! // Move camera diagonally up-right
//! let command = PanTiltCommand::Move {
//!     direction: PanTiltDirection::UpRight,
//!     pan_speed: PanSpeed::new(0x10).unwrap(),
//!     tilt_speed: TiltSpeed::new(0x10).unwrap(),
//! };
//! client.send(&command).unwrap();
//! # }
//! ```

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{Command, ResponseType},
    constants::{CameraConstants, CameraModel},
    error::Error,
    timeout::CommandCategory,
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed},
    Normalized,
};

/// Direction for pan/tilt movement commands.
///
/// Represents the 8 directional movements plus stop.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PanTiltDirection {
    /// Move camera upward (tilt up) while maintaining pan position.
    Up,
    /// Move camera downward (tilt down) while maintaining pan position.
    Down,
    /// Move camera leftward (pan left) while maintaining tilt position.
    Left,
    /// Move camera rightward (pan right) while maintaining tilt position.
    Right,
    /// Move camera diagonally up and to the left.
    UpLeft,
    /// Move camera diagonally up and to the right.
    UpRight,
    /// Move camera diagonally down and to the left.
    DownLeft,
    /// Move camera diagonally down and to the right.
    DownRight,
    /// Stop all pan/tilt movement.
    Stop,
}

impl PanTiltDirection {
    /// Converts the direction to its VISCA byte representation.
    ///
    /// Returns a tuple of (`pan_direction`, `tilt_direction`) bytes.
    #[must_use]
    pub const fn to_bytes(self) -> (u8, u8) {
        match self {
            Self::Up => (0x03, 0x01),
            Self::Down => (0x03, 0x02),
            Self::Left => (0x01, 0x03),
            Self::Right => (0x02, 0x03),
            Self::UpLeft => (0x01, 0x01),
            Self::UpRight => (0x02, 0x01),
            Self::DownLeft => (0x01, 0x02),
            Self::DownRight => (0x02, 0x02),
            Self::Stop => (0x03, 0x03),
        }
    }
}

/// Pan/Tilt movement commands.
///
/// Provides various ways to control camera pan and tilt:
/// - `Home` - Return to home position
/// - `Reset` - Reset pan/tilt mechanism
/// - `Move` - Directional movement with speed control
/// - `AbsolutePosition` - Move to exact coordinates
/// - `RelativePosition` - Move relative to current position
#[derive(Debug, Copy, Clone)]
pub enum PanTiltCommand {
    /// Return camera to home position.
    Home,
    /// Reset pan/tilt mechanism.
    Reset,
    /// Move camera in specified direction with given speeds.
    Move {
        /// Direction of movement (8 directions + stop).
        direction: PanTiltDirection,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
    /// Move camera to an absolute pan/tilt position.
    ///
    /// The pan and tilt values specify exact coordinates to move to.
    AbsolutePosition {
        /// Absolute pan position to move to.
        pan: PanPosition,
        /// Absolute tilt position to move to.
        tilt: TiltPosition,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
    /// Move camera relative to its current position.
    ///
    /// The pan and tilt values specify the offset from the current position.
    RelativePosition {
        /// Relative pan movement amount.
        pan: PanPosition,
        /// Relative tilt movement amount.
        tilt: TiltPosition,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
}

impl PanTiltCommand {
    /// Create an absolute position command with validation.
    pub fn absolute_position<P: crate::camera::CameraProfile>(
        pan: i16,
        tilt: i16,
    ) -> Result<Self, Error> {
        // Range validation
        if !P::PAN_RANGE.contains(&pan) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: pan as i32,
                min: *P::PAN_RANGE.start() as i32,
                max: *P::PAN_RANGE.end() as i32,
            });
        }

        if !P::TILT_RANGE.contains(&tilt) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: tilt as i32,
                min: *P::TILT_RANGE.start() as i32,
                max: *P::TILT_RANGE.end() as i32,
            });
        }

        Ok(Self::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?,
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: PanPosition::new(pan)?,
            tilt: TiltPosition::new(tilt)?,
        })
    }

    /// Create an absolute position command from normalized coordinates.
    pub fn absolute_position_normalized<P: crate::camera::CameraProfile>(
        pan: Normalized<f32>,
        tilt: Normalized<f32>,
    ) -> Result<Self, Error> {
        // Clamp normalized values to -1.0 to 1.0
        let pan_norm = pan.0.clamp(-1.0, 1.0);
        let tilt_norm = tilt.0.clamp(-1.0, 1.0);

        // Convert normalized to VISCA units
        let pan_range = P::PAN_RANGE.end() - P::PAN_RANGE.start();
        let pan_units = (pan_norm * pan_range as f32 / 2.0) as i16;

        let tilt_range = P::TILT_RANGE.end() - P::TILT_RANGE.start();
        let tilt_units = (tilt_norm * tilt_range as f32 / 2.0) as i16;

        Ok(Self::AbsolutePosition {
            pan_speed: PanSpeed::new(P::MAX_PAN_SPEED / 2)?,
            tilt_speed: TiltSpeed::new(P::MAX_TILT_SPEED / 2)?,
            pan: PanPosition::new(pan_units)?,
            tilt: TiltPosition::new(tilt_units)?,
        })
    }

    /// Create an absolute position command from degree coordinates.
    pub fn absolute_position_degrees<P: crate::camera::CameraProfile>(
        pan: crate::camera::units::Degrees<f32>,
        tilt: crate::camera::units::Degrees<f32>,
    ) -> Result<Self, Error> {
        // Create a default profile instance for conversion
        let profile = P::default();
        let pan_units = profile.pan_degrees_to_units(pan.0);
        let tilt_units = profile.tilt_degrees_to_units(tilt.0);

        Self::absolute_position::<P>(pan_units, tilt_units)
    }

    /// Create a continuous movement command with speed validation.
    pub fn continuous_move<P: crate::camera::CameraProfile>(
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<Self, Error> {
        // Ensure speeds are within valid range - 0 is always valid
        let safe_pan_speed = pan_speed.min(P::MAX_PAN_SPEED);
        let safe_tilt_speed = tilt_speed.min(P::MAX_TILT_SPEED);

        Ok(Self::Move {
            direction,
            pan_speed: PanSpeed::new(safe_pan_speed).map_err(|_| {
                Error::InvalidParameter(format!("Invalid pan speed: {}", safe_pan_speed))
            })?,
            tilt_speed: TiltSpeed::new(safe_tilt_speed).map_err(|_| {
                Error::InvalidParameter(format!("Invalid tilt speed: {}", safe_tilt_speed))
            })?,
        })
    }

    /// Create a stop command.
    pub fn stop() -> Result<Self, Error> {
        Ok(Self::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0)
                .map_err(|_| Error::InvalidParameter("Invalid pan speed: 0".to_string()))?,
            tilt_speed: TiltSpeed::new(0)
                .map_err(|_| Error::InvalidParameter("Invalid tilt speed: 0".to_string()))?,
        })
    }
}

impl Command for PanTiltCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            Self::Home => Ok(vec![0x81, 0x01, 0x06, 0x04, 0xFF]),
            Self::Reset => Ok(vec![0x81, 0x01, 0x06, 0x05, 0xFF]),
            Self::Move {
                direction,
                pan_speed,
                tilt_speed,
            } => {
                let (dir_byte1, dir_byte2) = direction.to_bytes();
                Ok(vec![
                    0x81,
                    0x01,
                    0x06,
                    0x01,
                    pan_speed.value(),
                    tilt_speed.value(),
                    dir_byte1,
                    dir_byte2,
                    0xFF,
                ])
            }
            Self::AbsolutePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                let pan_bytes = position_to_bytes(pan.value());
                let tilt_bytes = position_to_bytes(tilt.value());

                Ok(vec![
                    0x81,
                    0x01,
                    0x06,
                    0x02,
                    pan_speed.value(),
                    tilt_speed.value(),
                    pan_bytes[0],
                    pan_bytes[1],
                    pan_bytes[2],
                    pan_bytes[3],
                    tilt_bytes[0],
                    tilt_bytes[1],
                    tilt_bytes[2],
                    tilt_bytes[3],
                    0xFF,
                ])
            }
            Self::RelativePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                let pan_bytes = position_to_bytes(pan.value());
                let tilt_bytes = position_to_bytes(tilt.value());

                Ok(vec![
                    0x81,
                    0x01,
                    0x06,
                    0x03,
                    pan_speed.value(),
                    tilt_speed.value(),
                    pan_bytes[0],
                    pan_bytes[1],
                    pan_bytes[2],
                    pan_bytes[3],
                    tilt_bytes[0],
                    tilt_bytes[1],
                    tilt_bytes[2],
                    tilt_bytes[3],
                    0xFF,
                ])
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match self {
            Self::AbsolutePosition { pan, tilt, .. } => {
                let (pan_min, pan_max) = model.pan_range();
                let (tilt_min, tilt_max) = model.tilt_range();

                if pan.value() < pan_min || pan.value() > pan_max {
                    return Err(Error::ModelValidation {
                        model,
                        command: "PanTiltAbsolutePosition".to_string(),
                        reason: format!(
                            "Pan position {} out of range [{}, {}] for {:?}",
                            pan.value(),
                            pan_min,
                            pan_max,
                            model
                        ),
                    });
                }

                if tilt.value() < tilt_min || tilt.value() > tilt_max {
                    return Err(Error::ModelValidation {
                        model,
                        command: "PanTiltAbsolutePosition".to_string(),
                        reason: format!(
                            "Tilt position {} out of range [{}, {}] for {:?}",
                            tilt.value(),
                            tilt_min,
                            tilt_max,
                            model
                        ),
                    });
                }

                Ok(())
            }
            // Other pan/tilt commands are generally supported by all models
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_pan_tilt_direction_to_bytes() {
        assert_eq!(PanTiltDirection::Up.to_bytes(), (0x03, 0x01));
        assert_eq!(PanTiltDirection::Down.to_bytes(), (0x03, 0x02));
        assert_eq!(PanTiltDirection::Left.to_bytes(), (0x01, 0x03));
        assert_eq!(PanTiltDirection::Right.to_bytes(), (0x02, 0x03));
        assert_eq!(PanTiltDirection::UpLeft.to_bytes(), (0x01, 0x01));
        assert_eq!(PanTiltDirection::UpRight.to_bytes(), (0x02, 0x01));
        assert_eq!(PanTiltDirection::DownLeft.to_bytes(), (0x01, 0x02));
        assert_eq!(PanTiltDirection::DownRight.to_bytes(), (0x02, 0x02));
        assert_eq!(PanTiltDirection::Stop.to_bytes(), (0x03, 0x03));
    }

    #[test]
    fn test_pan_speed_validation() {
        // Valid speeds
        assert!(PanSpeed::new(0x00).is_ok());
        assert!(PanSpeed::new(0x18).is_ok());

        // Invalid speed
        assert!(matches!(
            PanSpeed::new(0x19),
            Err(Error::ParameterOutOfRange { .. })
        ));

        // From trait
        assert!(PanSpeed::try_from(0x10).is_ok());
        assert!(PanSpeed::try_from(0x20).is_err());

        // Value method
        let speed = PanSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}"));
        let value: u8 = speed.value();
        assert_eq!(value, 0x10);
    }

    #[test]
    fn test_tilt_speed_validation() {
        // Valid speeds
        assert!(TiltSpeed::new(0x00).is_ok());
        assert!(TiltSpeed::new(0x14).is_ok());

        // Invalid speed
        assert!(matches!(
            TiltSpeed::new(0x15),
            Err(Error::ParameterOutOfRange { .. })
        ));

        // From trait
        assert!(TiltSpeed::try_from(0x10).is_ok());
        assert!(TiltSpeed::try_from(0x15).is_err());

        // Value method
        let speed = TiltSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}"));
        let value: u8 = speed.value();
        assert_eq!(value, 0x10);
    }

    #[test]
    fn test_position_to_bytes() {
        // Test positive values
        assert_eq!(position_to_bytes(0x1234), [0x01, 0x02, 0x03, 0x04]);

        // Test negative values (two's complement)
        assert_eq!(position_to_bytes(-1), [0x0F, 0x0F, 0x0F, 0x0F]);
        assert_eq!(position_to_bytes(-0x1234), [0x0E, 0x0D, 0x0C, 0x0C]);

        // Test zero
        assert_eq!(position_to_bytes(0), [0x00, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn test_pan_tilt_command_to_bytes() {
        // Test Home command
        let home = PanTiltCommand::Home;
        assert_eq!(
            home.to_bytes()
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![0x81, 0x01, 0x06, 0x04, 0xFF]
        );

        // Test Reset command
        let reset = PanTiltCommand::Reset;
        assert_eq!(
            reset
                .to_bytes()
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![0x81, 0x01, 0x06, 0x05, 0xFF]
        );

        // Test Move command
        let move_cmd = PanTiltCommand::Move {
            direction: PanTiltDirection::UpRight,
            pan_speed: PanSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}")),
            tilt_speed: TiltSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}")),
        };
        assert_eq!(
            move_cmd
                .to_bytes()
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![0x81, 0x01, 0x06, 0x01, 0x10, 0x10, 0x02, 0x01, 0xFF]
        );

        // Test AbsolutePosition command
        let abs_pos = PanTiltCommand::AbsolutePosition {
            pan: PanPosition::new(0x0500).unwrap_or_else(|e| panic!("Valid pan position: {e:?}")),
            tilt: TiltPosition::new(0x0300).unwrap_or_else(|e| panic!("Valid tilt position: {e:?}")),
            pan_speed: PanSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}")),
            tilt_speed: TiltSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}")),
        };
        assert_eq!(
            abs_pos
                .to_bytes()
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![
                0x81, 0x01, 0x06, 0x02, 0x10, 0x10, 0x00, 0x05, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00,
                0xFF
            ]
        );

        // Test RelativePosition command
        let rel_pos = PanTiltCommand::RelativePosition {
            pan: PanPosition::new(-0x100).unwrap_or_else(|e| panic!("Valid pan position: {e:?}")),
            tilt: TiltPosition::new(0x200).unwrap_or_else(|e| panic!("Valid tilt position: {e:?}")),
            pan_speed: PanSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}")),
            tilt_speed: TiltSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}")),
        };
        assert_eq!(
            rel_pos
                .to_bytes()
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![
                0x81, 0x01, 0x06, 0x03, 0x10, 0x10, 0x0F, 0x0F, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
                0xFF
            ]
        );
    }

    #[test]
    fn test_pan_tilt_limit_command_to_bytes() {
        // Test Set command
        let set_limit = PanTiltLimitCommand::Set {
            corner: LimitCorner::DownLeft,
            pan: PanPosition::new(0x0400).unwrap_or_else(|e| panic!("Valid pan position: {e:?}")),
            tilt: TiltPosition::new(0x0200)
                .unwrap_or_else(|e| panic!("Valid tilt position: {e:?}")),
        };
        assert_eq!(
            set_limit
                .to_bytes()
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![
                0x81, 0x01, 0x06, 0x07, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00,
                0xFF
            ]
        );

        // Test Clear command
        let clear_limit = PanTiltLimitCommand::Clear {
            corner: LimitCorner::UpRight,
        };
        assert_eq!(
            clear_limit
                .to_bytes()
                .unwrap_or_else(|e| panic!("Valid command: {e:?}")),
            vec![
                0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
                0xFF
            ]
        );
    }

    #[test]
    fn test_pan_tilt_validation() {
        // Test validation for absolute position command
        let cmd_valid = PanTiltCommand::AbsolutePosition {
            pan: PanPosition::new(1000).unwrap_or_else(|e| panic!("Valid pan position: {e:?}")),
            tilt: TiltPosition::new(500).unwrap_or_else(|e| panic!("Valid tilt position: {e:?}")),
            pan_speed: PanSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}")),
            tilt_speed: TiltSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}")),
        };
        assert!(cmd_valid
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());

        // Test pan out of range - we can't create an invalid PanPosition,
        // so we need to test at a different level
        let result = PanPosition::new(3000); // Beyond PAN_MAX (2448)
        assert!(result.is_err());
        // Test tilt out of range
        let result = TiltPosition::new(2000); // Beyond TILT_MAX (1296)
        assert!(result.is_err());
    }

    #[test]
    fn test_pan_tilt_validation_other_commands() {
        // Test that other pan/tilt commands pass validation
        assert!(
            Command::validate_for_model(&PanTiltCommand::Home, CameraModel::PTZOpticsG2).is_ok()
        );
        assert!(
            Command::validate_for_model(&PanTiltCommand::Reset, CameraModel::PTZOpticsG2).is_ok()
        );

        let move_cmd = PanTiltCommand::Move {
            direction: PanTiltDirection::UpRight,
            pan_speed: PanSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}")),
            tilt_speed: TiltSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}")),
        };
        assert!(Command::validate_for_model(&move_cmd, CameraModel::PTZOpticsG2).is_ok());

        let rel_cmd = PanTiltCommand::RelativePosition {
            pan: PanPosition::new(100).unwrap_or_else(|e| panic!("Valid pan position: {e:?}")),
            tilt: TiltPosition::new(-100).unwrap_or_else(|e| panic!("Valid tilt position: {e:?}")),
            pan_speed: PanSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}")),
            tilt_speed: TiltSpeed::new(0x10).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}")),
        };
        assert!(Command::validate_for_model(&rel_cmd, CameraModel::PTZOpticsG2).is_ok());
    }

    #[test]
    fn test_response_type() {
        // All pan/tilt commands should return None for response_type
        assert!(PanTiltCommand::Home.response_type().is_none());
        assert!(PanTiltCommand::Reset.response_type().is_none());

        let move_cmd = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0).unwrap_or_else(|e| panic!("Valid pan speed: {e:?}")),
            tilt_speed: TiltSpeed::new(0).unwrap_or_else(|e| panic!("Valid tilt speed: {e:?}")),
        };
        assert!(move_cmd.response_type().is_none());

        let limit_cmd = PanTiltLimitCommand::Clear {
            corner: LimitCorner::DownLeft,
        };
        assert!(limit_cmd.response_type().is_none());
    }
}

const fn position_to_bytes(position: i16) -> [u8; 4] {
    // Convert to unsigned using bitwise representation (preserves bit pattern)
    #[allow(clippy::cast_sign_loss)]
    let unsigned = position as u16;
    [
        ((unsigned >> 12) & 0x0F) as u8,
        ((unsigned >> 8) & 0x0F) as u8,
        ((unsigned >> 4) & 0x0F) as u8,
        (unsigned & 0x0F) as u8,
    ]
}

/// Corner position for pan/tilt limits.
///
/// Used to define the movement boundaries of the camera.
#[derive(Debug, Copy, Clone)]
pub enum LimitCorner {
    /// Lower-left corner boundary position (minimum pan/tilt values).
    DownLeft = 0,
    /// Upper-right corner boundary position (maximum pan/tilt values).
    UpRight = 1,
}

/// Pan/Tilt limit commands.
///
/// Used to set or clear movement boundaries for the camera.
/// This prevents the camera from moving beyond specified positions.
#[derive(Debug, Copy, Clone)]
pub enum PanTiltLimitCommand {
    /// Set a movement limit at the specified corner position.
    ///
    /// This establishes a boundary that the camera cannot move beyond.
    Set {
        /// Which corner to set the limit for (`DownLeft` or `UpRight`).
        corner: LimitCorner,
        /// Pan position for the limit.
        pan: PanPosition,
        /// Tilt position for the limit.
        tilt: TiltPosition,
    },
    /// Clear the movement limit for the specified corner.
    ///
    /// This removes the boundary, allowing full range of movement.
    Clear {
        /// Which corner limit to clear (`DownLeft` or `UpRight`).
        corner: LimitCorner,
    },
}

impl Command for PanTiltLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            Self::Set { corner, pan, tilt } => {
                let pan_bytes = position_to_bytes(pan.value());
                let tilt_bytes = position_to_bytes(tilt.value());

                Ok(vec![
                    0x81,
                    0x01,
                    0x06,
                    0x07,
                    0x00,
                    *corner as u8,
                    pan_bytes[0],
                    pan_bytes[1],
                    pan_bytes[2],
                    pan_bytes[3],
                    tilt_bytes[0],
                    tilt_bytes[1],
                    tilt_bytes[2],
                    tilt_bytes[3],
                    0xFF,
                ])
            }
            Self::Clear { corner } => Ok(vec![
                0x81,
                0x01,
                0x06,
                0x07,
                0x01,
                *corner as u8,
                0x07,
                0x0F,
                0x0F,
                0x0F,
                0x07,
                0x0F,
                0x0F,
                0x0F,
                0xFF,
            ]),
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }
}
