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
//! use grafton_visca::{CameraId, Request};
//! use grafton_visca::request::builtin::ZoomDrive;
//!
//! // Typed requests write directly into caller-owned storage.
//! let request = ZoomDrive::Tele;
//! let mut bytes = [0_u8; ZoomDrive::MAX_SIZE];
//! let written = request.write_into(CameraId::CAMERA_1, &mut bytes).unwrap();
//! assert_eq!(bytes[0..written], [0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]);
//! ```

use crate::{
    command::{bytes::ConstCommandBuilder, encode::WireEncode},
    error::Error,
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
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

impl WireEncode for Zoom {
    const MAX_SIZE: usize = 10;

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
                    .push_visca_u16(position.value())
                    .terminate();
                builder.build_into(buffer)
            }
        }
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

    /// Returns whether digital zoom is enabled.
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }
}

impl WireEncode for DigitalZoom {
    const MAX_SIZE: usize = 6;

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
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::camera_id::CameraId;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::command::encode::WireEncode;

    /// Helper to encode a zoom command and return the bytes.
    fn encode_zoom(cmd: &Zoom) -> Vec<u8> {
        let mut buf = [0u8; 16];
        let len = cmd.write_into(CameraId::CAMERA_1, &mut buf).unwrap();
        buf[..len].to_vec()
    }

    #[test]
    fn test_zoom_position_encoding_min() {
        let pos = ZoomPosition::new(0x0000).unwrap();
        let bytes = encode_zoom(&Zoom::Position(pos));
        // 81 01 04 47 00 00 00 00 FF
        assert_eq!(
            bytes,
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                0x00,
                0x00,
                0x00,
                0x00,
                VISCA_TERMINATOR
            ]
        );
    }

    #[test]
    fn test_zoom_position_encoding_optical_g2_max() {
        // PtzOpticsG2 optical max = 0x4000
        let pos = ZoomPosition::new(0x4000).unwrap();
        let bytes = encode_zoom(&Zoom::Position(pos));
        // 0x4000 = 0100 0000 0000 0000 → nibbles: 4, 0, 0, 0
        assert_eq!(
            bytes,
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                0x04,
                0x00,
                0x00,
                0x00,
                VISCA_TERMINATOR
            ]
        );
    }

    #[test]
    fn test_zoom_position_encoding_digital_range_value() {
        // Some profiles with validated digital zoom ranges use values above 0x4000.
        let pos = ZoomPosition::new(0x7000).unwrap();
        let bytes = encode_zoom(&Zoom::Position(pos));
        // 0x7000 = nibbles: 7, 0, 0, 0
        assert_eq!(
            bytes,
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                0x07,
                0x00,
                0x00,
                0x00,
                VISCA_TERMINATOR
            ]
        );
    }

    #[test]
    fn test_zoom_position_encoding_high_profile_range_value() {
        // Preserve all nibbles for profile-specific high endpoints such as Axis 0x7AC0.
        let pos = ZoomPosition::new(0x7AC0).unwrap();
        let bytes = encode_zoom(&Zoom::Position(pos));
        // 0x7AC0 = nibbles: 7, A, C, 0
        assert_eq!(
            bytes,
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                0x07,
                0x0A,
                0x0C,
                0x00,
                VISCA_TERMINATOR
            ]
        );
    }

    #[test]
    fn test_zoom_position_encoding_digital_max() {
        // Maximum digital zoom = 0x7FFF
        let pos = ZoomPosition::new(0x7FFF).unwrap();
        let bytes = encode_zoom(&Zoom::Position(pos));
        // 0x7FFF = nibbles: 7, F, F, F
        assert_eq!(
            bytes,
            &[
                0x81,
                0x01,
                0x04,
                0x47,
                0x07,
                0x0F,
                0x0F,
                0x0F,
                VISCA_TERMINATOR
            ]
        );
    }

    /// Regression test: previously push_visca_u14 masked to 0x3FFF,
    /// truncating values >= 0x4000.
    #[test]
    fn test_zoom_position_above_0x3fff_not_truncated() {
        // 0x4001 should encode as 04 00 00 01, NOT 00 00 00 01
        let pos = ZoomPosition::new(0x4001).unwrap();
        let bytes = encode_zoom(&Zoom::Position(pos));
        assert_eq!(
            bytes[4], 0x04,
            "High nibble must be preserved (was truncated by u14 mask)"
        );
        assert_eq!(bytes[5], 0x00);
        assert_eq!(bytes[6], 0x00);
        assert_eq!(bytes[7], 0x01);
    }
}
