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
//! # use grafton_visca::ViscaClient;
//! # let client = ViscaClient::connect_udp("192.168.1.100:5678").unwrap();
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
use std::convert::TryFrom;

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{ViscaCommand, ViscaResponseType},
    error::ViscaError,
    timeout::CommandCategory,
};

/// Direction for pan/tilt movement commands.
///
/// Represents the 8 directional movements plus stop.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PanTiltDirection {
    Up,
    Down,
    Left,
    Right,
    UpLeft,
    UpRight,
    DownLeft,
    DownRight,
    Stop,
}

impl PanTiltDirection {
    pub const fn to_bytes(self) -> (u8, u8) {
        match self {
            PanTiltDirection::Up => (0x03, 0x01),
            PanTiltDirection::Down => (0x03, 0x02),
            PanTiltDirection::Left => (0x01, 0x03),
            PanTiltDirection::Right => (0x02, 0x03),
            PanTiltDirection::UpLeft => (0x01, 0x01),
            PanTiltDirection::UpRight => (0x02, 0x01),
            PanTiltDirection::DownLeft => (0x01, 0x02),
            PanTiltDirection::DownRight => (0x02, 0x02),
            PanTiltDirection::Stop => (0x03, 0x03),
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
#[derive(Debug)]
pub enum PanTiltCommand {
    /// Return camera to home position.
    Home,
    /// Reset pan/tilt mechanism.
    Reset,
    /// Move camera in specified direction with given speeds.
    Move {
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    },
    AbsolutePosition {
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    },
    RelativePosition {
        pan: i16,
        tilt: i16,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    },
}

impl ViscaCommand for PanTiltCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            PanTiltCommand::Home => Ok(vec![0x81, 0x01, 0x06, 0x04, 0xFF]),
            PanTiltCommand::Reset => Ok(vec![0x81, 0x01, 0x06, 0x05, 0xFF]),
            PanTiltCommand::Move {
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
            PanTiltCommand::AbsolutePosition {
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
            PanTiltCommand::RelativePosition {
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

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }
}

#[cfg(test)]
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
            Err(ViscaError::InvalidParameter(_))
        ));

        // From trait
        assert!(PanSpeed::try_from(0x10).is_ok());
        assert!(PanSpeed::try_from(0x20).is_err());

        // Into trait
        let speed = PanSpeed::new(0x10).unwrap();
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
            Err(ViscaError::InvalidParameter(_))
        ));

        // From trait
        assert!(TiltSpeed::try_from(0x10).is_ok());
        assert!(TiltSpeed::try_from(0x15).is_err());

        // Into trait
        let speed = TiltSpeed::new(0x10).unwrap();
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
        assert_eq!(home.to_bytes().unwrap(), vec![0x81, 0x01, 0x06, 0x04, 0xFF]);

        // Test Reset command
        let reset = PanTiltCommand::Reset;
        assert_eq!(
            reset.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x06, 0x05, 0xFF]
        );

        // Test Move command
        let move_cmd = PanTiltCommand::Move {
            direction: PanTiltDirection::UpRight,
            pan_speed: PanSpeed::new(0x10).unwrap(),
            tilt_speed: TiltSpeed::new(0x10).unwrap(),
        };
        assert_eq!(
            move_cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x06, 0x01, 0x10, 0x10, 0x02, 0x01, 0xFF]
        );

        // Test AbsolutePosition command
        let abs_pos = PanTiltCommand::AbsolutePosition {
            pan: 0x1234,
            tilt: 0x5678,
            pan_speed: PanSpeed::new(0x10).unwrap(),
            tilt_speed: TiltSpeed::new(0x10).unwrap(),
        };
        assert_eq!(
            abs_pos.to_bytes().unwrap(),
            vec![
                0x81, 0x01, 0x06, 0x02, 0x10, 0x10, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0xFF
            ]
        );

        // Test RelativePosition command
        let rel_pos = PanTiltCommand::RelativePosition {
            pan: -0x100,
            tilt: 0x200,
            pan_speed: PanSpeed::new(0x10).unwrap(),
            tilt_speed: TiltSpeed::new(0x10).unwrap(),
        };
        assert_eq!(
            rel_pos.to_bytes().unwrap(),
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
            set_limit.to_bytes().unwrap(),
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
            clear_limit.to_bytes().unwrap(),
            vec![
                0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
                0xFF
            ]
        );
    }

    #[test]
    fn test_response_type() {
        // All pan/tilt commands should return None for response_type
        assert!(PanTiltCommand::Home.response_type().is_none());
        assert!(PanTiltCommand::Reset.response_type().is_none());

        let move_cmd = PanTiltCommand::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::new(0).unwrap(),
            tilt_speed: TiltSpeed::new(0).unwrap(),
        };
        assert!(move_cmd.response_type().is_none());

        let limit_cmd = PanTiltLimitCommand::Clear {
            corner: LimitCorner::DownLeft,
        };
        assert!(limit_cmd.response_type().is_none());
    }
}

const fn position_to_bytes(position: i16) -> [u8; 4] {
    let unsigned = position as u16;
    [
        ((unsigned >> 12) & 0x0F) as u8,
        ((unsigned >> 8) & 0x0F) as u8,
        ((unsigned >> 4) & 0x0F) as u8,
        (unsigned & 0x0F) as u8,
    ]
}

/// Pan (horizontal) movement speed.
///
/// Valid range: 0x00 to 0x18 (0-24 decimal).
/// Higher values result in faster movement.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PanSpeed(u8);

impl PanSpeed {
    /// Maximum allowed pan speed (0x18 = 24 decimal).
    pub const MAX: u8 = 0x18;

    /// Creates a new `PanSpeed` with validation.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if value > 0x18.
    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= Self::MAX {
            Ok(PanSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Pan speed must be in the range 0x00..=0x{:02X}",
                Self::MAX
            )))
        }
    }
}

impl TryFrom<u8> for PanSpeed {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        PanSpeed::new(value)
    }
}

impl From<PanSpeed> for u8 {
    fn from(speed: PanSpeed) -> Self {
        speed.0
    }
}

/// Tilt (vertical) movement speed.
///
/// Valid range: 0x00 to 0x14 (0-20 decimal).
/// Higher values result in faster movement.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TiltSpeed(u8);

impl TiltSpeed {
    /// Maximum allowed tilt speed (0x14 = 20 decimal).
    pub const MAX: u8 = 0x14;

    /// Creates a new `TiltSpeed` with validation.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if value > 0x14.
    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= Self::MAX {
            Ok(TiltSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Tilt speed must be in the range 0x00..=0x{:02X}",
                Self::MAX
            )))
        }
    }
}

impl TryFrom<u8> for TiltSpeed {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        TiltSpeed::new(value)
    }
}

impl From<TiltSpeed> for u8 {
    fn from(speed: TiltSpeed) -> Self {
        speed.0
    }
}

/// Corner position for pan/tilt limits.
///
/// Used to define the movement boundaries of the camera.
#[derive(Debug, Copy, Clone)]
pub enum LimitCorner {
    DownLeft = 0,
    UpRight = 1,
}

/// Pan/Tilt limit commands.
///
/// Used to set or clear movement boundaries for the camera.
/// This prevents the camera from moving beyond specified positions.
#[derive(Debug)]
pub enum PanTiltLimitCommand {
    Set {
        corner: LimitCorner,
        pan: i16,
        tilt: i16,
    },
    Clear {
        corner: LimitCorner,
    },
}

impl ViscaCommand for PanTiltLimitCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            PanTiltLimitCommand::Set { corner, pan, tilt } => {
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
            PanTiltLimitCommand::Clear { corner } => Ok(vec![
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

    fn response_type(&self) -> Option<ViscaResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }
}
