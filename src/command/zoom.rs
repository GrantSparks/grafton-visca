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
    capabilities::{CameraFeature, CommandFeatures},
    command::{
        const_encoding::{constants, CommandBuilder, DEFAULT_ADDRESS},
        encode_visca::EncodeVisca,
        ResponseType,
    },
    error::Error,
    timeout::CommandCategory,
    types::{SpeedLevel, ZoomPosition},
    visca_command,
};

crate::visca_bounded_param! {
    /// Variable zoom speed.
    ///
    /// Valid range: 0 to 7 where 0 is the slowest and 7 is the fastest.
    ZoomSpeed: u8 {
        min: 0,
        max: 7
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
    Position(ZoomPosition),
}

impl Zoom {
    // Legacy method - removed in new API
    // pub fn direct<P: crate::camera::CameraProfile>(position: u16) -> Result<Self, Error> {
    //     ...
    //     Ok(Self::Position(ZoomPosition::new(position)?))
    // }
}

impl EncodeVisca for Zoom {
    type Response = ();
    const MAX_SIZE: usize = 10;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::constants::zoom;

        match self {
            Self::Stop => {
                let mut builder = CommandBuilder::<6>::from_prefix(zoom::STOP);
                builder.with_camera_id(camera_id);
                builder.copy_to(buffer)
            }
            Self::TeleStd => {
                let mut builder = CommandBuilder::<6>::from_prefix(zoom::TELE_STD);
                builder.with_camera_id(camera_id);
                builder.copy_to(buffer)
            }
            Self::WideStd => {
                let mut builder = CommandBuilder::<6>::from_prefix(zoom::WIDE_STD);
                builder.with_camera_id(camera_id);
                builder.copy_to(buffer)
            }
            Self::TeleVariable(speed) => {
                // Tele variable: 81 01 04 07 2p FF where p is speed
                let mut builder =
                    CommandBuilder::<6>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x07]);
                builder.with_camera_id(camera_id);
                builder.push(0x20 | (speed.0 & 0x0F)).finalize();
                builder.copy_to(buffer)
            }
            Self::WideVariable(speed) => {
                // Wide variable: 81 01 04 07 3p FF where p is speed
                let mut builder =
                    CommandBuilder::<6>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x07]);
                builder.with_camera_id(camera_id);
                builder.push(0x30 | (speed.0 & 0x0F)).finalize();
                builder.copy_to(buffer)
            }
            Self::Position(position) => {
                // Direct position: 81 01 04 47 0p 0q 0r 0s FF
                let mut builder =
                    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x47]);
                builder.with_camera_id(camera_id);
                builder.push_visca_u14(position.value()).finalize();
                builder.copy_to(buffer)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        // Zoom movement commands are action commands, not inquiries.
        // They receive ACK + Completion responses like pan-tilt movements.
        // Only dedicated inquiry commands should return specific response types.
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Movement
    }
}

impl CommandFeatures for Zoom {
    fn required_features(&self) -> &[CameraFeature] {
        // All Zoom commands require the Zoom feature
        &[CameraFeature::Zoom]
    }
}

/// Digital zoom control state.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DigitalZoom {
    /// Enable digital zoom.
    On = 0x02,
    /// Disable digital zoom.
    Off = 0x03,
}

visca_command! {
    /// Command to control digital zoom.
    ///
    /// This command enables or disables digital zoom capability.
    /// When enabled, zoom can continue past the optical zoom limit using digital processing.
    category = "Quick",
    enum DigitalZoomCommand {
        /// Enable digital zoom.
        On => {
            let cmd = CommandBuilder::<6>::new()
                .append(constants::zoom::DIGITAL_ZOOM_PREFIX)
                .push(0x02)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Disable digital zoom.
        Off => {
            let cmd = CommandBuilder::<6>::new()
                .append(constants::zoom::DIGITAL_ZOOM_PREFIX)
                .push(0x03)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

impl DigitalZoomCommand {
    /// Create a new digital zoom command.
    pub fn new(zoom: DigitalZoom) -> Self {
        match zoom {
            DigitalZoom::On => Self::On,
            DigitalZoom::Off => Self::Off,
        }
    }
}

impl CommandFeatures for DigitalZoomCommand {
    fn required_features(&self) -> &[CameraFeature] {
        // Digital zoom is part of the Zoom feature
        &[CameraFeature::Zoom]
    }
}
