//! Pan/Tilt control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera pan (horizontal) and tilt (vertical)
//! movement, including directional movement, absolute positioning, and relative positioning.
//!
//! # Speed Limits
//! - Pan speed: 0x00 to 0x18 (0-24 decimal)
//! - Tilt speed: 0x00 to 0x14 (0-20 decimal)
//!
//! # VISCA Compliance
//! All commands in this module are part of the baseline VISCA specification and should be
//! supported by all VISCA-compliant cameras.
//! # Example
//! ```ignore
//! # #[cfg(not(feature = "mode-async"))]
//! # {
//! # use grafton_visca::command::pan_tilt::{PanTilt, PanTiltDirection, PanSpeed, TiltSpeed};
//! # use grafton_visca::Client;
//! # let client = Client::connect_udp("192.168.0.110:5678").unwrap();
//! // Move camera diagonally up-right
//! let command = PanTilt::Move {
//!     direction: PanTiltDirection::UpRight,
//!     pan_speed: PanSpeed::new(0x10).unwrap(),
//!     tilt_speed: TiltSpeed::new(0x10).unwrap(),
//! };
//! client.send(&command).unwrap();
//! # }
//! ```

use crate::{
    command::{bytes::ConstCommandBuilder, encode::ViscaCommand, InquiryKind},
    error::Error,
    timeout::CommandCategory,
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed},
};

/// Corner position for pan/tilt limit setting.
///
/// Specifies which corner of the movement range to set as a limit.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PanTiltLimitCorner {
    /// Lower-left corner (minimum pan, minimum tilt).
    DownLeft,
    /// Lower-right corner (maximum pan, minimum tilt).
    DownRight,
    /// Upper-left corner (minimum pan, maximum tilt).
    UpLeft,
    /// Upper-right corner (maximum pan, maximum tilt).
    UpRight,
}

impl PanTiltLimitCorner {
    /// Converts the corner to its VISCA byte representation.
    #[must_use]
    pub const fn to_byte(self) -> u8 {
        match self {
            Self::DownLeft => 0x00,
            Self::DownRight => 0x01,
            Self::UpLeft => 0x02,
            Self::UpRight => 0x03,
        }
    }
}

/// Direction for pan/tilt movement commands.
///
/// Represents the 8 directional movements plus stop.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PanTiltDirection {
    /// Move camera upward (tilt up) while maintaining pan position.
    Up,
    /// Move camera downward (tilt down) while maintaining pan position.
    Down,
    /// Move camera leftward (pan left) while maintaining tilt position.
    Left,
    /// Move camera rightward (pan right) while maintaining tilt position.
    Right,
    /// Move camera diagonally up and to the left.
    UpLeft,
    /// Move camera diagonally up and to the right.
    UpRight,
    /// Move camera diagonally down and to the left.
    DownLeft,
    /// Move camera diagonally down and to the right.
    DownRight,
    /// Stop all pan/tilt movement.
    Stop,
}

impl PanTiltDirection {
    /// Converts the direction to its VISCA byte representation.
    ///
    /// Returns a tuple of (`pan_direction`, `tilt_direction`) bytes.
    #[must_use]
    pub const fn to_bytes(self) -> (u8, u8) {
        match self {
            Self::Up => (0x03, 0x01),
            Self::Down => (0x03, 0x02),
            Self::Left => (0x01, 0x03),
            Self::Right => (0x02, 0x03),
            Self::UpLeft => (0x01, 0x01),
            Self::UpRight => (0x02, 0x01),
            Self::DownLeft => (0x01, 0x02),
            Self::DownRight => (0x02, 0x02),
            Self::Stop => (0x03, 0x03),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        PanTilt,
        test_pan_tilt_home,
        PanTilt::Home,
        &[0x81, 0x01, 0x06, 0x04, VISCA_TERMINATOR]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_reset,
        PanTilt::Reset,
        &[0x81, 0x01, 0x06, 0x05, VISCA_TERMINATOR]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_limit_set_down_left,
        PanTilt::LimitSet {
            corner: PanTiltLimitCorner::DownLeft,
            pan: PanPosition::new(0x0123).expect("valid test pan position"),
            tilt: TiltPosition::new(0x0456).expect("valid test tilt position"),
        },
        &[
            0x81, 0x01, 0x06, 0x07, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x00, 0x04, 0x05, 0x06,
            0xFF
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_limit_clear_down_left,
        PanTilt::LimitClear {
            corner: PanTiltLimitCorner::DownLeft,
        },
        &[
            0x81, 0x01, 0x06, 0x07, 0x01, 0x00, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
            0xFF
        ]
    );
}

/// Pan/Tilt movement commands.
///
/// Provides various ways to control camera pan and tilt:
/// - `Home` - Return to home position
/// - `Reset` - Reset pan/tilt mechanism
/// - `Move` - Directional movement with speed control
/// - `AbsolutePosition` - Move to exact coordinates
/// - `RelativePosition` - Move relative to current position
/// - `AbsolutePositionRaw` - Move to exact coordinates with pre-converted camera units
/// - `RelativePositionRaw` - Move relative with pre-converted camera units
/// - `LimitSet` - Set pan/tilt movement boundaries
/// - `LimitSetRaw` - Set pan/tilt boundaries with pre-converted camera units
/// - `LimitClear` - Clear all pan/tilt movement boundaries
#[derive(Debug, Copy, Clone)]
pub enum PanTilt {
    /// Return camera to home position.
    Home,
    /// Reset pan/tilt mechanism.
    Reset,
    /// Move camera in specified direction with given speeds.
    Move {
        /// Direction of movement (8 directions + stop).
        direction: PanTiltDirection,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
    /// Move camera to an absolute pan/tilt position.
    ///
    /// The pan and tilt values specify exact coordinates to move to.
    AbsolutePosition {
        /// Absolute pan position to move to.
        pan: PanPosition,
        /// Absolute tilt position to move to.
        tilt: TiltPosition,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
    /// Move camera relative to its current position.
    ///
    /// The pan and tilt values specify the offset from the current position.
    RelativePosition {
        /// Relative pan movement amount.
        pan: PanPosition,
        /// Relative tilt movement amount.
        tilt: TiltPosition,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
    /// Move camera to an absolute pan/tilt position using raw camera units.
    ///
    /// The pan and tilt values are pre-converted to camera coordinate system.
    /// This is used internally when the profile-aware conversion has already been applied.
    AbsolutePositionRaw {
        /// Absolute pan position in camera units.
        pan_u16: u16,
        /// Absolute tilt position in camera units.
        tilt_u16: u16,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
    /// Move camera relative to its current position using raw camera units.
    ///
    /// The pan and tilt values are pre-converted to camera coordinate system.
    /// This is used internally when the profile-aware conversion has already been applied.
    RelativePositionRaw {
        /// Relative pan movement in camera units.
        pan_u16: u16,
        /// Relative tilt movement in camera units.
        tilt_u16: u16,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x14).
        tilt_speed: TiltSpeed,
    },
    /// Set pan/tilt movement boundaries.
    ///
    /// Sets a specified position as a corner limit for pan/tilt movement.
    /// This is the full VISCA implementation with corner and position parameters.
    LimitSet {
        /// Which corner of the movement range to set.
        corner: PanTiltLimitCorner,
        /// Pan position for the limit.
        pan: PanPosition,
        /// Tilt position for the limit.
        tilt: TiltPosition,
    },
    /// Set pan/tilt movement boundaries using raw camera units.
    ///
    /// Sets a specified position as a corner limit for pan/tilt movement.
    /// The pan and tilt values are pre-converted to camera coordinate system.
    LimitSetRaw {
        /// Which corner of the movement range to set.
        corner: PanTiltLimitCorner,
        /// Pan position for the limit in camera units.
        pan_u16: u16,
        /// Tilt position for the limit in camera units.
        tilt_u16: u16,
    },
    /// Clear pan/tilt movement boundaries.
    ///
    /// Clears a specified corner limit or all limits.
    LimitClear {
        /// Which corner to clear (same encoding as LimitSet).
        corner: PanTiltLimitCorner,
    },
}

impl PanTilt {
    // NOTE: The absolute_position_degrees method was removed because it referenced
    // an out-of-scope generic parameter. Use the camera facade methods instead.
}

impl ViscaCommand for PanTilt {
    type Response = ();
    const MAX_SIZE: usize = 15;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Movement;

    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::bytes::constants::pan_tilt;

        match self {
            Self::Home => {
                let mut builder = ConstCommandBuilder::<6>::from_prefix(pan_tilt::HOME);
                builder = builder.with_camera_id(camera_id);
                builder.terminate().build_into(buffer)
            }
            Self::Reset => {
                let mut builder = ConstCommandBuilder::<6>::from_prefix(pan_tilt::RESET);
                builder = builder.with_camera_id(camera_id);
                builder.terminate().build_into(buffer)
            }
            Self::Move {
                direction,
                pan_speed,
                tilt_speed,
            } => {
                // Move command: 81 01 06 01 VV WW XX YY FF
                // Where VV = pan speed, WW = tilt speed, XX YY = direction
                let mut builder = ConstCommandBuilder::<9>::from_prefix(pan_tilt::MOVE_PREFIX);
                builder.with_camera_id_mut(camera_id);

                let (pan_dir, tilt_dir) = direction.to_bytes();
                builder.push_mut(pan_speed.value());
                builder.push_mut(tilt_speed.value());
                builder.push_mut(pan_dir);
                builder.push_mut(tilt_dir);
                builder.terminate().build_into(buffer)
            }
            Self::AbsolutePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                // Absolute position: 81 01 06 02 VV WW PP PP PP PP TT TT TT TT FF
                let mut builder = ConstCommandBuilder::<15>::from_prefix(pan_tilt::ABSOLUTE_PREFIX);
                builder.with_camera_id_mut(camera_id);

                builder.push_mut(pan_speed.value());
                builder.push_mut(tilt_speed.value());
                builder.push_visca_u16_mut(pan.value() as u16);
                builder.push_visca_u16_mut(tilt.value() as u16);
                builder.terminate().build_into(buffer)
            }
            Self::RelativePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => {
                // Relative position: 81 01 06 03 VV WW PP PP PP PP TT TT TT TT FF
                let mut builder = ConstCommandBuilder::<15>::from_prefix(pan_tilt::RELATIVE_PREFIX);
                builder.with_camera_id_mut(camera_id);

                builder.push_mut(pan_speed.value());
                builder.push_mut(tilt_speed.value());
                builder.push_visca_u16_mut(pan.value() as u16);
                builder.push_visca_u16_mut(tilt.value() as u16);
                builder.terminate().build_into(buffer)
            }
            Self::AbsolutePositionRaw {
                pan_u16,
                tilt_u16,
                pan_speed,
                tilt_speed,
            } => {
                // Absolute position: 81 01 06 02 VV WW PP PP PP PP TT TT TT TT FF
                let mut builder = ConstCommandBuilder::<15>::from_prefix(pan_tilt::ABSOLUTE_PREFIX);
                builder.with_camera_id_mut(camera_id);

                builder.push_mut(pan_speed.value());
                builder.push_mut(tilt_speed.value());
                builder.push_visca_u16_mut(*pan_u16);
                builder.push_visca_u16_mut(*tilt_u16);
                builder.terminate().build_into(buffer)
            }
            Self::RelativePositionRaw {
                pan_u16,
                tilt_u16,
                pan_speed,
                tilt_speed,
            } => {
                // Relative position: 81 01 06 03 VV WW PP PP PP PP TT TT TT TT FF
                let mut builder = ConstCommandBuilder::<15>::from_prefix(pan_tilt::RELATIVE_PREFIX);
                builder.with_camera_id_mut(camera_id);

                builder.push_mut(pan_speed.value());
                builder.push_mut(tilt_speed.value());
                builder.push_visca_u16_mut(*pan_u16);
                builder.push_visca_u16_mut(*tilt_u16);
                builder.terminate().build_into(buffer)
            }
            Self::LimitSet { corner, pan, tilt } => {
                // PT Limit Set: 81 01 06 07 00 0W PPPP TTTT FF
                // Where W = corner (0-3), PPPP = pan position, TTTT = tilt position
                let builder = ConstCommandBuilder::<15>::from_prefix(pan_tilt::LIMIT_SET_PREFIX)
                    .with_camera_id(camera_id)
                    .push(corner.to_byte())
                    .push_visca_u16(pan.value() as u16)
                    .push_visca_u16(tilt.value() as u16)
                    .terminate();

                builder.build_into(buffer)
            }
            Self::LimitSetRaw {
                corner,
                pan_u16,
                tilt_u16,
            } => {
                // PT Limit Set: 81 01 06 07 00 0W PPPP TTTT FF
                // Where W = corner (0-3), PPPP = pan position, TTTT = tilt position
                let builder = ConstCommandBuilder::<15>::from_prefix(pan_tilt::LIMIT_SET_PREFIX)
                    .with_camera_id(camera_id)
                    .push(corner.to_byte())
                    .push_visca_u16(*pan_u16)
                    .push_visca_u16(*tilt_u16)
                    .terminate();

                builder.build_into(buffer)
            }
            Self::LimitClear { corner } => {
                // PT Limit Clear: 81 01 06 07 01 0W 07 0F 0F 0F 07 0F 0F 0F FF
                // Where W = corner (0-3), the rest are fixed values per PtzOptics spec
                let builder = ConstCommandBuilder::<15>::from_prefix(pan_tilt::LIMIT_CLEAR_PREFIX)
                    .with_camera_id(camera_id)
                    .push(corner.to_byte())
                    .push(0x07)
                    .push(0x0F)
                    .push(0x0F)
                    .push(0x0F)
                    .push(0x07)
                    .push(0x0F)
                    .push(0x0F)
                    .push(0x0F)
                    .terminate();

                builder.build_into(buffer)
            }
        }
    }

    fn response_kind(&self) -> Option<InquiryKind> {
        None
    }
}
