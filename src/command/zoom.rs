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
//! # use grafton_visca::command::ZoomCommand;
//! # use grafton_visca::{UdpTransport, ViscaTransport};
//! # let mut transport = UdpTransport::new("192.168.1.100:5678").unwrap();
//! // Zoom in at standard speed
//! transport.send_command(&ZoomCommand::TeleStandard).unwrap();
//!
//! // Zoom out at variable speed
//! transport.send_command(&ZoomCommand::WideVariable(5)).unwrap();
//! ```

use super::ViscaResponseType;
use crate::command::ViscaCommand;
use crate::error::ViscaError;
use crate::timeout::CommandCategory;

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
    /// Zoom in at variable speed (0=slowest, 7=fastest).
    TeleVariable(u8),
    /// Zoom out at variable speed (0=slowest, 7=fastest).
    WideVariable(u8),
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

            // Tele variable zoom with valid speed (0..=7)
            ZoomCommand::TeleVariable(speed) if *speed <= 7 => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x20 | speed, 0xFF])
            }

            // Wide variable zoom with valid speed (0..=7)
            ZoomCommand::WideVariable(speed) if *speed <= 7 => {
                Ok(vec![0x81, 0x01, 0x04, 0x07, 0x30 | speed, 0xFF])
            }

            // Handle invalid speed values for variable zoom commands
            ZoomCommand::TeleVariable(_) | ZoomCommand::WideVariable(_) => Err(
                ViscaError::InvalidParameter("Zoom speed must be in the range 0..=7".into()),
            ),

            // Direct zoom to a specific position
            ZoomCommand::Direct(position) => {
                // Extract individual nibbles from the position
                let p = ((*position >> 12) & 0x0F) as u8;
                let q = ((*position >> 8) & 0x0F) as u8;
                let r = ((*position >> 4) & 0x0F) as u8;
                let s = (*position & 0x0F) as u8;

                Ok(vec![0x81, 0x01, 0x04, 0x47, p, q, r, s, 0xFF])
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
