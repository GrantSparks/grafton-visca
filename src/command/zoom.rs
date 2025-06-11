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
//! # #[cfg(feature = "blocking-client")]
//! # {
//! # use grafton_visca::command::{ZoomCommand, zoom::ZoomSpeed};
//! # use grafton_visca::Client;
//! # let client = Client::connect_udp("192.168.1.100:5678").unwrap();
//! // Zoom in at standard speed
//! client.send(&ZoomCommand::ZoomInStandard).unwrap();
//!
//! // Zoom out at variable speed
//! client.send(&ZoomCommand::ZoomOutVariable(ZoomSpeed::new(5).unwrap())).unwrap();
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

/// Zoom control commands.
///
/// Provides various ways to control camera zoom:
/// - `Stop` - Stop zoom movement
/// - `ZoomInStandard` - Zoom in at standard speed
/// - `ZoomOutStandard` - Zoom out at standard speed
/// - `ZoomInVariable` - Zoom in at specified speed (0-7)
/// - `ZoomOutVariable` - Zoom out at specified speed (0-7)
/// - `Direct` - Set zoom to specific position
#[derive(Debug, Copy, Clone)]
pub enum ZoomCommand {
    /// Stop zoom movement.
    Stop,
    /// Zoom in at standard speed.
    ZoomInStandard,
    /// Zoom out at standard speed.
    ZoomOutStandard,
    /// Zoom in at variable speed.
    ZoomInVariable(ZoomSpeed),
    /// Zoom out at variable speed.
    ZoomOutVariable(ZoomSpeed),
    /// Set zoom to direct position (0x0000 to 0xFFFF).
    Direct(u16),
}

impl Command for ZoomCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            // Stop command
            Self::Stop => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]),

            // Zoom in standard
            Self::ZoomInStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),

            // Zoom out standard
            Self::ZoomOutStandard => Ok(vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]),

            // Zoom in variable
            Self::ZoomInVariable(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x20 | speed.value(), 0xFF])
            }

            // Zoom out variable
            Self::ZoomOutVariable(speed) => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x30 | speed.value(), 0xFF])
            }

            // Direct zoom to a specific position
            Self::Direct(position) => {
                let nibbles = position_to_nibbles(*position);

                Ok(vec![
                    0x81, 0x01, 0x04, 0x47, nibbles[0], nibbles[1], nibbles[2], nibbles[3], 0xFF,
                ])
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        match self {
            Self::ZoomInStandard => Some(ResponseType::ZoomInStandard),
            Self::ZoomOutStandard => Some(ResponseType::ZoomOutStandard),
            _ => None,
        }
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Movement
    }

    fn validate_for_model(&self, model: CameraModel) -> Result<(), Error> {
        match self {
            Self::Direct(position) => {
                let (min, max) = model.zoom_range();
                if *position < min || *position > max {
                    return Err(Error::ModelValidation {
                        model,
                        command: "ZoomDirect".to_string(),
                        reason: format!(
                            "Position 0x{:04X} out of range [0x{:04X}, 0x{:04X}] for {:?}",
                            position, min, max, model
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_zoom_speed_new() {
        // Valid speeds
        for speed in 0..=7 {
            let zoom_speed = ZoomSpeed::new(speed).expect("Valid zoom speed");
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
        let speed = ZoomSpeed::try_from(5).expect("Valid zoom speed");
        assert_eq!(speed.value(), 5);

        // Invalid conversion
        assert!(ZoomSpeed::try_from(8).is_err());
    }

    #[test]
    fn test_zoom_speed_into_u8() {
        let speed = ZoomSpeed::new(3).expect("Valid zoom speed");
        let value: u8 = speed.into();
        assert_eq!(value, 3);
    }

    #[test]
    fn test_zoom_command_stop() {
        let cmd = ZoomCommand::Stop;
        assert_eq!(
            cmd.to_bytes().expect("Valid command"),
            vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_zoom_command_zoom_in_standard() {
        let cmd = ZoomCommand::ZoomInStandard;
        assert_eq!(
            cmd.to_bytes().expect("Valid command"),
            vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::ZoomInStandard));
    }

    #[test]
    fn test_zoom_command_zoom_out_standard() {
        let cmd = ZoomCommand::ZoomOutStandard;
        assert_eq!(
            cmd.to_bytes().expect("Valid command"),
            vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]
        );
        assert_eq!(cmd.response_type(), Some(ResponseType::ZoomOutStandard));
    }

    #[test]
    fn test_zoom_command_zoom_in_variable() {
        let speed = ZoomSpeed::new(5).expect("Valid zoom speed");
        let cmd = ZoomCommand::ZoomInVariable(speed);
        assert_eq!(
            cmd.to_bytes().expect("Valid command"),
            vec![0x81, 0x01, 0x04, 0x07, 0x25, 0xFF]
        );
    }

    #[test]
    fn test_zoom_command_zoom_out_variable() {
        let speed = ZoomSpeed::new(7).expect("Valid zoom speed");
        let cmd = ZoomCommand::ZoomOutVariable(speed);
        assert_eq!(
            cmd.to_bytes().expect("Valid command"),
            vec![0x81, 0x01, 0x04, 0x07, 0x37, 0xFF]
        );
    }

    #[test]
    fn test_zoom_command_direct() {
        let cmd = ZoomCommand::Direct(0x1234);
        assert_eq!(
            cmd.to_bytes().expect("Valid command"),
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
    fn test_zoom_validation_g2_camera() {
        // Test validation for PTZOptics G2 (20X zoom)
        let cmd_valid = ZoomCommand::Direct(0x7000); // Max for 20X
        assert!(cmd_valid.validate_for_model(CameraModel::PTZOpticsG2).is_ok());
        
        let cmd_invalid = ZoomCommand::Direct(0x7AC0); // 30X position
        let result = cmd_invalid.validate_for_model(CameraModel::PTZOpticsG2);
        assert!(result.is_err());
        match result {
            Err(Error::ModelValidation { model, command, reason }) => {
                assert_eq!(model, CameraModel::PTZOpticsG2);
                assert_eq!(command, "ZoomDirect");
                assert!(reason.contains("0x7AC0"));
                assert!(reason.contains("0x7000"));
            }
            _ => panic!("Expected ModelValidation error"),
        }
    }
    
    #[test]
    fn test_zoom_validation_30x_camera() {
        // Test validation for PTZOptics 30X
        let cmd_valid = ZoomCommand::Direct(0x7AC0); // Max for 30X
        assert!(cmd_valid.validate_for_model(CameraModel::PTZOptics30X).is_ok());
        
        let cmd_edge = ZoomCommand::Direct(0x7FFF); // Beyond 30X but within digital range
        let result = cmd_edge.validate_for_model(CameraModel::PTZOptics30X);
        assert!(result.is_err());
    }
    
    #[test]
    fn test_zoom_validation_other_commands() {
        // Test that other zoom commands pass validation
        assert!(Command::validate_for_model(&ZoomCommand::Stop, CameraModel::PTZOpticsG2).is_ok());
        assert!(Command::validate_for_model(&ZoomCommand::ZoomInStandard, CameraModel::PTZOpticsG2).is_ok());
        assert!(Command::validate_for_model(&ZoomCommand::ZoomOutStandard, CameraModel::PTZOpticsG2).is_ok());
        assert!(Command::validate_for_model(&ZoomCommand::ZoomInVariable(ZoomSpeed::new(5).unwrap()), CameraModel::PTZOpticsG2).is_ok());
        assert!(Command::validate_for_model(&ZoomCommand::ZoomOutVariable(ZoomSpeed::new(3).unwrap()), CameraModel::PTZOpticsG2).is_ok());
    }

    #[test]
    fn test_command_category() {
        assert_eq!(
            ZoomCommand::Stop.command_category(),
            CommandCategory::Movement
        );
        assert_eq!(
            ZoomCommand::ZoomInStandard.command_category(),
            CommandCategory::Movement
        );
        assert_eq!(
            ZoomCommand::Direct(0).command_category(),
            CommandCategory::Movement
        );
    }
}
