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

/// Digital zoom control state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DigitalZoom {
    /// Enable digital zoom.
    On = 0x02,
    /// Disable digital zoom.
    Off = 0x03,
}

/// Command to control digital zoom.
///
/// This command enables or disables digital zoom capability.
/// When enabled, zoom can continue past the optical zoom limit using digital processing.
#[derive(Debug, Copy, Clone)]
pub struct DigitalZoomCommand {
    /// The desired digital zoom state.
    pub zoom: DigitalZoom,
}

impl Command for DigitalZoomCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![0x81, 0x01, 0x04, 0x06, self.zoom as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
