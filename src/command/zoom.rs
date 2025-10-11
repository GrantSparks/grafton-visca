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
//! # #[cfg(not(feature = "mode-async"))]
//! # {
//! # use grafton_visca::command::{Zoom, zoom::ZoomSpeed};
//! # use grafton_visca::Client;
//! # let client = Client::connect_udp("192.168.0.110:5678").unwrap();
//! // Zoom in at standard speed
//! client.send(&Zoom::TeleStd).unwrap();
//!
//! // Zoom out at variable speed
//! client.send(&Zoom::WideVariable(ZoomSpeed::new(5).unwrap())).unwrap();
//! # }
//! ```

use crate::{
    command::{bytes::ConstCommandBuilder, encode::ViscaCommand, InquiryKind},
    error::Error,
    timeout::CommandCategory,
    types::{ZoomPosition, ZoomSpeed},
};

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
    Position(ZoomPosition),
}

impl ViscaCommand for Zoom {
    type Response = ();
    const MAX_SIZE: usize = 10;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants::zoom;

        match self {
            Self::Stop => {
                let builder = ConstCommandBuilder::<6>::from_prefix(zoom::STOP)
                    .with_camera_id(camera_id)
                    .terminate();
                builder.build_into(buffer)
            }
            Self::TeleStd => {
                let builder = ConstCommandBuilder::<6>::from_prefix(zoom::TELE_STD)
                    .with_camera_id(camera_id)
                    .terminate();
                builder.build_into(buffer)
            }
            Self::WideStd => {
                let builder = ConstCommandBuilder::<6>::from_prefix(zoom::WIDE_STD)
                    .with_camera_id(camera_id)
                    .terminate();
                builder.build_into(buffer)
            }
            Self::TeleVariable(speed) => {
                // Tele variable: 81 01 04 07 2p FF where p is speed
                let builder = ConstCommandBuilder::<6>::from_prefix(zoom::VARIABLE_PREFIX)
                    .with_camera_id(camera_id)
                    .push(0x20 | (speed.value() & 0x0F))
                    .terminate();
                builder.build_into(buffer)
            }
            Self::WideVariable(speed) => {
                // Wide variable: 81 01 04 07 3p FF where p is speed
                let builder = ConstCommandBuilder::<6>::from_prefix(zoom::VARIABLE_PREFIX)
                    .with_camera_id(camera_id)
                    .push(0x30 | (speed.value() & 0x0F))
                    .terminate();
                builder.build_into(buffer)
            }
            Self::Position(position) => {
                // Direct position: 81 01 04 47 0p 0q 0r 0s FF
                let builder = ConstCommandBuilder::<9>::from_prefix(zoom::POSITION_PREFIX)
                    .with_camera_id(camera_id)
                    .push_visca_u14(position.value())
                    .terminate();
                builder.build_into(buffer)
            }
        }
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        // Zoom movement commands are action commands, not inquiries.
        // They receive ACK + Completion responses like pan-tilt movements.
        // Only dedicated inquiry commands should return specific response types.
        None
    }
}

/// Command to control digital zoom.
///
/// This command enables or disables digital zoom capability.
/// When enabled, zoom can continue past the optical zoom limit using digital processing.
#[derive(Debug, Copy, Clone)]
pub struct DigitalZoom {
    enabled: bool,
}

impl DigitalZoom {
    /// Create a new digital zoom command.
    pub const fn new(enabled: bool) -> Self {
        Self { enabled }
    }
}

impl ViscaCommand for DigitalZoom {
    type Response = ();
    const MAX_SIZE: usize = 6;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants::zoom::DIGITAL_ZOOM_PREFIX;

        let builder = ConstCommandBuilder::<6>::from_prefix(DIGITAL_ZOOM_PREFIX)
            .with_camera_id(camera_id)
            .push(if self.enabled { 0x02 } else { 0x03 })
            .terminate();
        builder.build_into(buffer)
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }
}
