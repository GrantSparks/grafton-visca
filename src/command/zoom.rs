//! Zoom control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera zoom functionality,
//! including standard speed, variable speed, and direct position control.
//!
//! # Variable Speed Range
//! Variable zoom speed ranges from 0 (slowest) to 7 (fastest).
//!
//! # Example
//! ```no_run
//! # use grafton_visca::command::{ZoomCommand, zoom::ZoomSpeed};
//! # use grafton_visca::{UdpTransport, ViscaTransport};
//! # let mut transport = UdpTransport::new("192.168.1.100:5678").unwrap();
//! // Zoom in at standard speed
//! transport.send_command(&ZoomCommand::TeleStandard).unwrap();
//!
//! // Zoom out at variable speed
//! transport.send_command(&ZoomCommand::WideVariable(ZoomSpeed::new(5).unwrap())).unwrap();
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

/// Variable zoom speed.
///
/// Valid range: 0 to 7 where 0 is the slowest and 7 is the fastest.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ZoomSpeed(u8);

impl ZoomSpeed {
    /// Maximum allowed zoom speed.
    pub const MAX: u8 = 7;

    /// Creates a new ZoomSpeed with validation.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if value > 7.
    pub fn new(value: u8) -> Result<Self, ViscaError> {
        if value <= Self::MAX {
            Ok(ZoomSpeed(value))
        } else {
            Err(ViscaError::InvalidParameter(format!(
                "Zoom speed must be in the range 0..={}",
                Self::MAX
            )))
        }
    }

    /// Get the raw value.
    pub fn value(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for ZoomSpeed {
    type Error = ViscaError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        ZoomSpeed::new(value)
    }
}

impl From<ZoomSpeed> for u8 {
    fn from(speed: ZoomSpeed) -> Self {
        speed.0
    }
}

/// Zoom control commands.
///
/// Provides various ways to control camera zoom:
/// - `Stop` - Stop zoom movement
/// - `TeleStandard` - Zoom in at standard speed
/// - `WideStandard` - Zoom out at standard speed
/// - `TeleVariable` - Zoom in at specified speed (0-7)
/// - `WideVariable` - Zoom out at specified speed (0-7)
/// - `Direct` - Set zoom to specific position
#[derive(Debug)]
pub enum ZoomCommand {
    /// Stop zoom movement.
    Stop,
    /// Zoom in (telephoto) at standard speed.
    TeleStandard,
    /// Zoom out (wide) at standard speed.
    WideStandard,
    /// Zoom in at variable speed.
    TeleVariable(ZoomSpeed),
    /// Zoom out at variable speed.
    WideVariable(ZoomSpeed),
    /// Set zoom to direct position (0x0000 to 0xFFFF).
    Direct(u16),
}

impl ViscaCommand for ZoomCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
        match self {
            // Stop command
            ZoomCommand::Stop => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]),

            // Tele standard zoom
            ZoomCommand::TeleStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),

            // Wide standard zoom
            ZoomCommand::WideStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]),

            // Tele variable zoom
            ZoomCommand::TeleVariable(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x20 | speed.value(), 0xFF])
            }

            // Wide variable zoom
            ZoomCommand::WideVariable(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x30 | speed.value(), 0xFF])
            }

            // Direct zoom to a specific position
            ZoomCommand::Direct(position) => {
                let nibbles = position_to_nibbles(*position);

                Ok(vec![
                    0x81, 0x01, 0x04, 0x47, nibbles[0], nibbles[1], nibbles[2], nibbles[3], 0xFF,
                ])
            }
        }
    }

    fn response_type(&self) -> Option<ViscaResponseType> {
        match self {
            ZoomCommand::TeleStandard => Some(ViscaResponseType::ZoomTeleStandard),
            ZoomCommand::WideStandard => Some(ViscaResponseType::ZoomWideStandard),
            _ => None,
        }
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }
}

/// Converts a 16-bit position value into an array of 4 nibbles.
///
/// This is a common pattern in VISCA commands for encoding position data.
fn position_to_nibbles(position: u16) -> [u8; 4] {
    [
        ((position >> 12) & 0x0F) as u8,
        ((position >> 8) & 0x0F) as u8,
        ((position >> 4) & 0x0F) as u8,
        (position & 0x0F) as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_speed_new() {
        // Valid speeds
        for speed in 0..=7 {
            let zoom_speed = ZoomSpeed::new(speed).unwrap();
            assert_eq!(zoom_speed.value(), speed);
        }

        // Invalid speed
        assert!(matches!(
            ZoomSpeed::new(8),
            Err(ViscaError::InvalidParameter(_))
        ));
        assert!(matches!(
            ZoomSpeed::new(255),
            Err(ViscaError::InvalidParameter(_))
        ));
    }

    #[test]
    fn test_zoom_speed_try_from() {
        // Valid conversion
        let speed = ZoomSpeed::try_from(5).unwrap();
        assert_eq!(speed.value(), 5);

        // Invalid conversion
        assert!(ZoomSpeed::try_from(8).is_err());
    }

    #[test]
    fn test_zoom_speed_into_u8() {
        let speed = ZoomSpeed::new(3).unwrap();
        let value: u8 = speed.into();
        assert_eq!(value, 3);
    }

    #[test]
    fn test_zoom_command_stop() {
        let cmd = ZoomCommand::Stop;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_zoom_command_tele_standard() {
        let cmd = ZoomCommand::TeleStandard;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]
        );
        assert_eq!(
            cmd.response_type(),
            Some(ViscaResponseType::ZoomTeleStandard)
        );
    }

    #[test]
    fn test_zoom_command_wide_standard() {
        let cmd = ZoomCommand::WideStandard;
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]
        );
        assert_eq!(
            cmd.response_type(),
            Some(ViscaResponseType::ZoomWideStandard)
        );
    }

    #[test]
    fn test_zoom_command_tele_variable() {
        let speed = ZoomSpeed::new(5).unwrap();
        let cmd = ZoomCommand::TeleVariable(speed);
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF]
        );
    }

    #[test]
    fn test_zoom_command_wide_variable() {
        let speed = ZoomSpeed::new(7).unwrap();
        let cmd = ZoomCommand::WideVariable(speed);
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x07, 0x37, 0xFF]
        );
    }

    #[test]
    fn test_zoom_command_direct() {
        let cmd = ZoomCommand::Direct(0x1234);
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x47, 0x01, 0x02, 0x03, 0x04, 0xFF]
        );
    }

    #[test]
    fn test_position_to_nibbles() {
        assert_eq!(position_to_nibbles(0x0000), [0x00, 0x00, 0x00, 0x00]);
        assert_eq!(position_to_nibbles(0x1234), [0x01, 0x02, 0x03, 0x04]);
        assert_eq!(position_to_nibbles(0xABCD), [0x0A, 0x0B, 0x0C, 0x0D]);
        assert_eq!(position_to_nibbles(0xFFFF), [0x0F, 0x0F, 0x0F, 0x0F]);
    }

    #[test]
    fn test_command_category() {
        assert_eq!(
            ZoomCommand::Stop.command_category(),
            CommandCategory::Movement
        );
        assert_eq!(
            ZoomCommand::TeleStandard.command_category(),
            CommandCategory::Movement
        );
        assert_eq!(
            ZoomCommand::Direct(0).command_category(),
            CommandCategory::Movement
        );
    }
}
