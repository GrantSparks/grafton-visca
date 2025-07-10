//! Zoom control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera zoom functionality,
//! including standard speed, variable speed, and direct position control.
//!
//! # Variable Speed Range
//! Variable zoom speed ranges from 0 (slowest) to 7 (fastest).
//!
//! # Example
//! ```ignore
//! # #[cfg(not(feature = "async"))]
//! # {
//! # use grafton_visca::command::{ZoomCommand, zoom::ZoomSpeed};
//! # use grafton_visca::Client;
//! # let client = Client::connect_udp("192.168.1.100:5678").unwrap();
//! // Zoom in at standard speed
//! client.send(&ZoomCommand::TeleStandard).unwrap();
//!
//! // Zoom out at variable speed
//! client.send(&ZoomCommand::WideVariable(ZoomSpeed::new(5).unwrap())).unwrap();
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
    types::{SpeedLevel, ZoomPosition},
};

crate::visca_bounded_param! {
    /// Variable zoom speed.
    ///
    /// Valid range: 0 to 7 where 0 is the slowest and 7 is the fastest.
    ZoomSpeed: u8 {
        min: 0,
        max: 7,
        error_msg: "Zoom speed must be in the range 0..=7"
    }
}

impl From<SpeedLevel> for ZoomSpeed {
    fn from(level: SpeedLevel) -> Self {
        Self(level.to_zoom_speed())
    }
}

/// Zoom control commands.
///
/// Provides various ways to control camera zoom:
/// - `Stop` - Stop zoom movement
/// - `TeleStandard` - Zoom in (telephoto) at standard speed
/// - `WideStandard` - Zoom out (wide) at standard speed
/// - `TeleVariable` - Zoom in at specified speed (0-7)
/// - `WideVariable` - Zoom out at specified speed (0-7)
/// - `Position` - Set zoom to specific position
#[derive(Debug, Copy, Clone)]
pub enum ZoomCommand {
    /// Stop zoom movement.
    Stop,
    /// Zoom in at standard speed (telephoto).
    TeleStandard,
    /// Zoom out at standard speed (wide).
    WideStandard,
    /// Zoom in at variable speed.
    TeleVariable(ZoomSpeed),
    /// Zoom out at variable speed.
    WideVariable(ZoomSpeed),
    /// Set zoom to specific position.
    Position(ZoomPosition),
}

impl ZoomCommand {
    // Legacy method - removed in new API
    // pub fn direct<P: crate::camera::CameraProfile>(position: u16) -> Result<Self, Error> {
    //     ...
    //     Ok(Self::Position(ZoomPosition::new(position)?))
    // }
}

impl Command for ZoomCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            // Stop command
            Self::Stop => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]),

            // Zoom in standard
            Self::TeleStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),

            // Zoom out standard
            Self::WideStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]),

            // Zoom in variable
            Self::TeleVariable(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x20 | speed.value(), 0xFF])
            }

            // Zoom out variable
            Self::WideVariable(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x30 | speed.value(), 0xFF])
            }

            // Direct zoom to a specific position
            Self::Position(position) => {
                let nibbles = position_to_nibbles(position.value());

                Ok(vec![
                    0x81, 0x01, 0x04, 0x47, nibbles[0], nibbles[1], nibbles[2], nibbles[3], 0xFF,
                ])
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        match self {
            Self::TeleStandard => Some(ResponseType::ZoomIn),
            Self::WideStandard => Some(ResponseType::ZoomOut),
            _ => None,
        }
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match self {
            Self::Position(position) => {
                let (min, max) = model.zoom_range();
                if position.value() < min || position.value() > max {
                    return Err(Error::ModelValidation {
                        model,
                        command: "ZoomPosition".to_string(),
                        reason: format!(
                            "Position 0x{:04X} out of range [0x{min:04X}, 0x{max:04X}] for {model:?}", position.value()
                        ),
                    });
                }
                Ok(())
            }
            // Other zoom commands are generally supported by all models
            _ => Ok(()),
        }
    }
}

/// Converts a 16-bit position value into an array of 4 nibbles.
///
/// This is a common pattern in VISCA commands for encoding position data.
const fn position_to_nibbles(position: u16) -> [u8; 4] {
    [
        ((position >> 12) & 0x0F) as u8,
        ((position >> 8) & 0x0F) as u8,
        ((position >> 4) & 0x0F) as u8,
        (position & 0x0F) as u8,
    ]
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_speed_new() {
        // Valid speeds
        for speed in 0..=7 {
            let zoom_speed = ZoomSpeed::new(speed)
                .unwrap_or_else(|e| panic!("Failed to create ZoomSpeed {speed}: {e:?}"));
            assert_eq!(zoom_speed.value(), speed);
        }

        // Invalid speed
        assert!(matches!(ZoomSpeed::new(8), Err(Error::InvalidParameter(_))));
        assert!(matches!(
            ZoomSpeed::new(255),
            Err(Error::InvalidParameter(_))
        ));
    }

    #[test]
    fn test_zoom_speed_try_from() {
        // Valid conversion
        let speed = ZoomSpeed::try_from(5)
            .unwrap_or_else(|e| panic!("ZoomSpeed::try_from(5) should be valid: {e:?}"));
        assert_eq!(speed.value(), 5);

        // Invalid conversion
        assert!(ZoomSpeed::try_from(8).is_err());
    }

    #[test]
    fn test_zoom_speed_into_u8() {
        let speed =
            ZoomSpeed::new(3).unwrap_or_else(|e| panic!("Failed to create ZoomSpeed 3: {e:?}"));
        let value: u8 = speed.value();
        assert_eq!(value, 3);
    }

    #[test]
    fn test_zoom_command_stop() {
        let cmd = ZoomCommand::Stop;
        let bytes = cmd
            .to_bytes()
            .unwrap_or_else(|e| panic!("Failed to convert Stop command to bytes: {e:?}"));
        assert_eq!(
            bytes,
            vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF],
            "Stop command bytes mismatch"
        );
    }

    #[test]
    fn test_zoom_command_zoom_in_standard() {
        let cmd = ZoomCommand::TeleStandard;
        let bytes = cmd
            .to_bytes()
            .unwrap_or_else(|e| panic!("Failed to convert TeleStandard command to bytes: {e:?}"));
        assert_eq!(
            bytes,
            vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF],
            "TeleStandard command bytes mismatch"
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::ZoomIn));
    }

    #[test]
    fn test_zoom_command_zoom_out_standard() {
        let cmd = ZoomCommand::WideStandard;
        let bytes = cmd
            .to_bytes()
            .unwrap_or_else(|e| panic!("Failed to convert WideStandard command to bytes: {e:?}"));
        assert_eq!(
            bytes,
            vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF],
            "WideStandard command bytes mismatch"
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::ZoomOut));
    }

    #[test]
    fn test_zoom_command_zoom_in_variable() {
        let speed =
            ZoomSpeed::new(5).unwrap_or_else(|e| panic!("Failed to create ZoomSpeed 5: {e:?}"));
        let cmd = ZoomCommand::TeleVariable(speed);
        let bytes = cmd
            .to_bytes()
            .unwrap_or_else(|e| panic!("Failed to convert TeleVariable command to bytes: {e:?}"));
        assert_eq!(
            bytes,
            vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF],
            "TeleVariable command bytes mismatch"
        );
    }

    #[test]
    fn test_zoom_command_zoom_out_variable() {
        let speed =
            ZoomSpeed::new(7).unwrap_or_else(|e| panic!("Failed to create ZoomSpeed 7: {e:?}"));
        let cmd = ZoomCommand::WideVariable(speed);
        let bytes = cmd
            .to_bytes()
            .unwrap_or_else(|e| panic!("Failed to convert WideVariable command to bytes: {e:?}"));
        assert_eq!(
            bytes,
            vec![0x81, 0x01, 0x04, 0x07, 0x37, 0xFF],
            "WideVariable command bytes mismatch"
        );
    }

    #[test]
    fn test_zoom_command_position() {
        let cmd = ZoomCommand::Position(
            ZoomPosition::new(0x1234).unwrap_or_else(|e| panic!("Valid zoom position: {e:?}")),
        );
        let bytes = cmd
            .to_bytes()
            .unwrap_or_else(|e| panic!("Failed to convert Position command to bytes: {e:?}"));
        assert_eq!(
            bytes,
            vec![0x81, 0x01, 0x04, 0x47, 0x01, 0x02, 0x03, 0x04, 0xFF],
            "Position command bytes mismatch"
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
    fn test_zoom_validation_g2_camera() {
        // Test validation for PTZOptics G2 (20X zoom)
        let cmd_valid = ZoomCommand::Position(
            ZoomPosition::new(0x7000).unwrap_or_else(|e| panic!("Valid zoom position: {e:?}")),
        ); // Max for 20X
        assert!(cmd_valid
            .validate_for_model(CameraModel::PTZOpticsG2)
            .is_ok());

        // Test that ZoomPosition itself enforces the maximum
        let result = ZoomPosition::new(0x7AC0); // 30X position
        assert!(result.is_err());
    }

    #[test]
    fn test_zoom_validation_30x_camera() {
        // Test validation for PTZOptics 30X
        // Note: ZoomPosition is limited to 0x7000, so we can't test 30X-specific values
        let cmd_valid = ZoomCommand::Position(
            ZoomPosition::new(0x7000).unwrap_or_else(|e| panic!("Valid zoom position: {e:?}")),
        ); // Max allowed by ZoomPosition
        assert!(cmd_valid
            .validate_for_model(CameraModel::PTZOptics30X)
            .is_ok());
    }

    #[test]
    fn test_zoom_validation_other_commands() {
        // Test that other zoom commands pass validation
        assert!(Command::validate_for_model(&ZoomCommand::Stop, CameraModel::PTZOpticsG2).is_ok());
        assert!(
            Command::validate_for_model(&ZoomCommand::TeleStandard, CameraModel::PTZOpticsG2)
                .is_ok()
        );
        assert!(
            Command::validate_for_model(&ZoomCommand::WideStandard, CameraModel::PTZOpticsG2)
                .is_ok()
        );
        assert!(Command::validate_for_model(
            &ZoomCommand::TeleVariable(
                ZoomSpeed::new(5).unwrap_or_else(|e| panic!("Valid ZoomSpeed 5: {e:?}"))
            ),
            CameraModel::PTZOpticsG2
        )
        .is_ok());
        assert!(Command::validate_for_model(
            &ZoomCommand::WideVariable(
                ZoomSpeed::new(3).unwrap_or_else(|e| panic!("Valid ZoomSpeed 3: {e:?}"))
            ),
            CameraModel::PTZOpticsG2
        )
        .is_ok());
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
            ZoomCommand::Position(
                ZoomPosition::new(0).unwrap_or_else(|e| panic!("Valid zoom position: {e:?}"))
            )
            .command_category(),
            CommandCategory::Movement
        );
    }
}
