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
    // units::Normalized, // Used in commented out code
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
    // Legacy methods removed - use the new camera API instead
    /*
    /// Create an absolute position command with validation.
    pub fn absolute_position<P: crate::camera::CameraProfile>(
        pan: i16,
        tilt: i16,
    ) -> Result<Self, Error> {
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
    pub fn absolute_position_normalized(
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
    pub fn absolute_position_degrees(
        pan: crate::units::Degrees<f32>,
        tilt: crate::units::Degrees<f32>,
    ) -> Result<Self, Error> {
        // Create a default profile instance for conversion
        let profile = P::default();
        let pan_units = profile.pan_degrees_to_units(pan.0);
        let tilt_units = profile.tilt_degrees_to_units(tilt.0);

        Self::absolute_position::<P>(pan_units, tilt_units)
    }

    /// Create a continuous movement command with speed validation.
    pub fn continuous_move(
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<Self, Error> {
        // Ensure speeds are within valid range - 0 is always valid
        let safe_pan_speed = pan_speed.min(P::MAX_PAN_SPEED);
        let safe_tilt_speed = tilt_speed.min(P::MAX_TILT_SPEED);

        let (pan_speed, tilt_speed) = crate::validate_all! {
            pan_speed: PanSpeed::new(safe_pan_speed),
            tilt_speed: TiltSpeed::new(safe_tilt_speed),
        }?;

        Ok(Self::Move {
            direction,
            pan_speed,
            tilt_speed,
        })
    }
    */

    /// Create a stop command.
    pub fn stop() -> Result<Self, Error> {
        let (pan_speed, tilt_speed) = crate::validate_all! {
            pan_speed: PanSpeed::new(0),
            tilt_speed: TiltSpeed::new(0),
        }?;

        Ok(Self::Move {
            direction: PanTiltDirection::Stop,
            pan_speed,
            tilt_speed,
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

/// Converts a position value to 4 bytes in VISCA nibble format.
fn position_to_bytes(position: i16) -> [u8; 4] {
    let pos_u16 = position as u16;
    [
        ((pos_u16 >> 12) & 0x0F) as u8,
        ((pos_u16 >> 8) & 0x0F) as u8,
        ((pos_u16 >> 4) & 0x0F) as u8,
        (pos_u16 & 0x0F) as u8,
    ]
}

