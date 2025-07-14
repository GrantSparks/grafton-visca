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

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        use crate::command::const_encoding::constants::zoom;

        match self {
            Self::Stop => {
                let bytes = zoom::STOP;
                if buffer.len() < bytes.len() {
                    return Err(Error::BufferTooSmall {
                        required: bytes.len(),
                        actual: buffer.len(),
                    });
                }
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            Self::TeleStd => {
                let bytes = zoom::TELE_STD;
                if buffer.len() < bytes.len() {
                    return Err(Error::BufferTooSmall {
                        required: bytes.len(),
                        actual: buffer.len(),
                    });
                }
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            Self::WideStd => {
                let bytes = zoom::WIDE_STD;
                if buffer.len() < bytes.len() {
                    return Err(Error::BufferTooSmall {
                        required: bytes.len(),
                        actual: buffer.len(),
                    });
                }
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
            Self::TeleVariable(speed) => {
                // Tele variable: 81 01 04 07 2p FF where p is speed
                let required = 6;
                if buffer.len() < required {
                    return Err(Error::BufferTooSmall {
                        required,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x07;
                buffer[4] = 0x20 | (speed.0 & 0x0F);
                buffer[5] = 0xFF;
                Ok(required)
            }
            Self::WideVariable(speed) => {
                // Wide variable: 81 01 04 07 3p FF where p is speed
                let required = 6;
                if buffer.len() < required {
                    return Err(Error::BufferTooSmall {
                        required,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x07;
                buffer[4] = 0x30 | (speed.0 & 0x0F);
                buffer[5] = 0xFF;
                Ok(required)
            }
            Self::Position(position) => {
                // Direct position: 81 01 04 47 0p 0q 0r 0s FF
                let required = 9;
                if buffer.len() < required {
                    return Err(Error::BufferTooSmall {
                        required,
                        actual: buffer.len(),
                    });
                }
                let nibbles = position_to_nibbles(position.value());
                buffer[0] = 0x81;
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x47;
                buffer[4] = nibbles[0];
                buffer[5] = nibbles[1];
                buffer[6] = nibbles[2];
                buffer[7] = nibbles[3];
                buffer[8] = 0xFF;
                Ok(required)
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

visca_command! {
    /// Command to control digital zoom.
    ///
    /// This command enables or disables digital zoom capability.
    /// When enabled, zoom can continue past the optical zoom limit using digital processing.
    category = "Quick",
    enum DigitalZoomCommand {
        /// Enable digital zoom.
        On => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::zoom::DIGITAL_ZOOM_PREFIX)
                .push(0x02)
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Disable digital zoom.
        Off => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(crate::command::const_encoding::constants::zoom::DIGITAL_ZOOM_PREFIX)
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
