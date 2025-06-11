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
//! ```no_run
//! # #[cfg(feature = "blocking-client")]
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
        pan: i16,
        /// Absolute tilt position to move to.
        tilt: i16,
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
        pan: i16,
        /// Relative tilt movement amount.
        tilt: i16,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
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
                    (*pan_speed).into(),
                    (*tilt_speed).into(),
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
                let pan_bytes = position_to_bytes(*pan);
                let tilt_bytes = position_to_bytes(*tilt);

                Ok(vec![
                    0x81,
                    0x01,
                    0x06,
                    0x02,
                    (*pan_speed).into(),
                    (*tilt_speed).into(),
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
                let pan_bytes = position_to_bytes(*pan);
                let tilt_bytes = position_to_bytes(*tilt);

                Ok(vec![
                    0x81,
                    0x01,
                    0x06,
                    0x03,
                    (*pan_speed).into(),
                    (*tilt_speed).into(),
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

                if *pan < pan_min || *pan > pan_max {
                    return Err(Error::ModelValidation {
                        model,
                        command: "PanTiltAbsolutePosition".to_string(),
                        reason: format!(
                            "Pan position {pan} out of range [{pan_min}, {pan_max}] for {model:?}"
                        ),
                    });
                }

                if *tilt < tilt_min || *tilt > tilt_max {
                    return Err(Error::ModelValidation {
                        model,
                        command: "PanTiltAbsolutePosition".to_string(),
                        reason: format!(
                            "Tilt position {tilt} out of range [{tilt_min}, {tilt_max}] for {model:?}"
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
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
            Err(Error::InvalidParameter(_))
        ));

        // From trait
        assert!(PanSpeed::try_from(0x10).is_ok());
        assert!(PanSpeed::try_from(0x20).is_err());

        // Into trait
        let speed = PanSpeed::new(0x10).expect("Valid pan speed");
        let value: u8 = speed.into();
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
            Err(Error::InvalidParameter(_))
        ));

        // From trait
        assert!(TiltSpeed::try_from(0x10).is_ok());
        assert!(TiltSpeed::try_from(0x15).is_err());

        // Into trait
        let speed = TiltSpeed::new(0x10).expect("Valid tilt speed");
        let value: u8 = speed.into();
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
            home.to_bytes().expect("Valid command"),
            vec![0x81, 0x01, 0x06, 0x04, 0xFF]
        );

        // Test Reset command
        let reset = PanTiltCommand::Reset;
        assert_eq!(
            reset.to_bytes().expect("Valid command"),
            vec![0x81, 0x01, 0x06, 0x05, 0xFF]
        );

        // Test Move command
        let move_cmd = PanTiltCommand::Move {
            direction: PanTiltDirection::UpRight,
            pan_speed: PanSpeed::new(0x10).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0x10).expect("Valid tilt speed"),
        };
        assert_eq!(
            move_cmd.to_bytes().expect("Valid command"),
            vec![0x81, 0x01, 0x06, 0x01, 0x10, 0x10, 0x02, 0x01, 0xFF]
        );

        // Test AbsolutePosition command
        let abs_pos = PanTiltCommand::AbsolutePosition {
            pan: 0x1234,
            tilt: 0x5678,
            pan_speed: PanSpeed::new(0x10).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0x10).expect("Valid tilt speed"),
        };
        assert_eq!(
            abs_pos.to_bytes().expect("Valid command"),
            vec![
                0x81, 0x01, 0x06, 0x02, 0x10, 0x10, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0xFF
            ]
        );

        // Test RelativePosition command
        let rel_pos = PanTiltCommand::RelativePosition {
            pan: -0x100,
            tilt: 0x200,
            pan_speed: PanSpeed::new(0x10).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0x10).expect("Valid tilt speed"),
        };
        assert_eq!(
            rel_pos.to_bytes().expect("Valid command"),
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
            pan: 0x1000,
            tilt: 0x2000,
        };
        assert_eq!(
            set_limit.to_bytes().expect("Valid command"),
            vec![
                0x81, 0x01, 0x06, 0x07, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
                0xFF
            ]
        );

        // Test Clear command
        let clear_limit = PanTiltLimitCommand::Clear {
            corner: LimitCorner::UpRight,
        };
        assert_eq!(
            clear_limit.to_bytes().expect("Valid command"),
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
            pan: 1000,
            tilt: 500,
            pan_speed: PanSpeed::new(0x10).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0x10).expect("Valid tilt speed"),
        };
        assert!(cmd_valid
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());

        // Test pan out of range
        let cmd_invalid_pan = PanTiltCommand::AbsolutePosition {
            pan: 3000, // Beyond PAN_MAX (2448)
            tilt: 500,
            pan_speed: PanSpeed::new(0x10).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0x10).expect("Valid tilt speed"),
        };
        let result = cmd_invalid_pan.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(result.is_err());
        match result {
            Err(Error::ModelValidation { command, .. }) => {
                assert_eq!(command, "PanTiltAbsolutePosition");
            }
            _ => unreachable!("Expected ModelValidation error"),
        }

        // Test tilt out of range
        let cmd_invalid_tilt = PanTiltCommand::AbsolutePosition {
            pan: 1000,
            tilt: 2000, // Beyond TILT_MAX (1296)
            pan_speed: PanSpeed::new(0x10).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0x10).expect("Valid tilt speed"),
        };
        let result = cmd_invalid_tilt.validate_for_model(CameraModel::PTZOpticsG2);
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
            pan_speed: PanSpeed::new(0x10).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0x10).expect("Valid tilt speed"),
        };
        assert!(Command::validate_for_model(&move_cmd, CameraModel::PTZOpticsG2).is_ok());

        let rel_cmd = PanTiltCommand::RelativePosition {
            pan: 100,
            tilt: -100,
            pan_speed: PanSpeed::new(0x10).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0x10).expect("Valid tilt speed"),
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
            pan_speed: PanSpeed::new(0).expect("Valid pan speed"),
            tilt_speed: TiltSpeed::new(0).expect("Valid tilt speed"),
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

crate::visca_bounded_param! {
    /// Pan (horizontal) movement speed.
    ///
    /// Valid range: 0x00 to 0x18 (0-24 decimal).
    /// Higher values result in faster movement.
    PanSpeed: u8 {
        min: 0x00,
        max: 0x18,
        error_msg: "Pan speed must be in the range 0x00..=0x18"
    }
}

impl PanSpeed {
    /// Zero speed value (stop).
    pub const ZERO: Self = Self(0);

    /// Default medium speed value.
    pub const DEFAULT_MEDIUM: Self = Self(0x10);
}

crate::visca_bounded_param! {
    /// Tilt (vertical) movement speed.
    ///
    /// Valid range: 0x00 to 0x14 (0-20 decimal).
    /// Higher values result in faster movement.
    TiltSpeed: u8 {
        min: 0x00,
        max: 0x14,
        error_msg: "Tilt speed must be in the range 0x00..=0x14"
    }
}

impl TiltSpeed {
    /// Zero speed value (stop).
    pub const ZERO: Self = Self(0);

    /// Default medium speed value.
    pub const DEFAULT_MEDIUM: Self = Self(0x10);
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
        pan: i16,
        /// Tilt position for the limit.
        tilt: i16,
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
                let pan_bytes = position_to_bytes(*pan);
                let tilt_bytes = position_to_bytes(*tilt);

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
