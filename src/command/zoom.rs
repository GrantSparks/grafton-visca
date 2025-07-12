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
//! # use grafton_visca::command::{Zoom, zoom::ZoomSpeed};
//! # use grafton_visca::Client;
//! # let client = Client::connect_udp("192.168.1.100:5678").unwrap();
//! // Zoom in at standard speed
//! client.send(&Zoom::TeleStd).unwrap();
//!
//! // Zoom out at variable speed
//! client.send(&Zoom::WideVariable(ZoomSpeed::new(5).unwrap())).unwrap();
//! # }
//! ```

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{SpeedLevel, ZoomPosition}};

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
/// - `TeleStd` - Zoom in (telephoto) at standard speed
/// - `WideStd` - Zoom out (wide) at standard speed
/// - `TeleVariable` - Zoom in at specified speed (0-7)
/// - `WideVariable` - Zoom out at specified speed (0-7)
/// - `Position` - Set zoom to specific position
#[derive(Debug, Copy, Clone)]
pub enum Zoom {
    /// Stop zoom movement.
    Stop,
    /// Zoom in at standard speed (telephoto).
    TeleStd,
    /// Zoom out at standard speed (wide).
    WideStd,
    /// Zoom in at variable speed.
    TeleVariable(ZoomSpeed),
    /// Zoom out at variable speed.
    WideVariable(ZoomSpeed),
    /// Set zoom to specific position.
    Position(ZoomPosition)}

impl Zoom {
    // Legacy method - removed in new API
    // pub fn direct<P: crate::camera::CameraProfile>(position: u16) -> Result<Self, Error> {
    //     ...
    //     Ok(Self::Position(ZoomPosition::new(position)?))
    // }
}

impl EncodeVisca for Zoom {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0x07;
        buffer[4] = 0x00;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        match self {
            Self::TeleStd => Some(ResponseType::ZoomIn),
            Self::WideStd => Some(ResponseType::ZoomOut),
            _ => None}
    }
    
    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Movement
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
    Off = 0x03}

/// Command to control digital zoom.
///
/// This command enables or disables digital zoom capability.
/// When enabled, zoom can continue past the optical zoom limit using digital processing.
#[derive(Debug, Copy, Clone)]
pub(crate) struct DigitalZoomCommand {
    /// The desired digital zoom state.
    pub zoom: DigitalZoom}

impl EncodeVisca for DigitalZoomCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len()});
        }

        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0x06;
        buffer[4] = self.zoom as u8;
        buffer[5] = 0xFF;
        
        Ok(Self::MAX_SIZE)
    }
    
    fn response_type(&self) -> Option<ResponseType> {
        None
    }
    
    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}
