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
//! use grafton_visca::{CameraId, Request};
//! use grafton_visca::{command::PanTiltDirection, request::builtin::PanTiltDrive};
//! use grafton_visca::types::{PanSpeed, TiltSpeed};
//!
//! // Move the camera diagonally up-right using a typed operation request.
//! let request = PanTiltDrive::new(
//!     PanTiltDirection::UpRight,
//!     PanSpeed::new(0x10).unwrap(),
//!     TiltSpeed::new(0x10).unwrap(),
//! ).unwrap();
//! let mut bytes = [0_u8; PanTiltDrive::MAX_SIZE];
//! let written = request.write_into(CameraId::CAMERA_1, &mut bytes).unwrap();
//! assert_eq!(bytes[0], 0x81);
//! assert_eq!(bytes[written - 1], 0xFF);
//! ```

use crate::{
    command::{bytes::ConstCommandBuilder, encode::WireEncode},
    error::Error,
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed},
};

/// Corner position for pan/tilt limit setting.
///
/// Pan/tilt limits define a rectangular bounding box for allowed movement.
/// The bounding box is specified by two diagonal corners: upper-right and
/// lower-left. These two corners fully define the movement rectangle.
///
/// Other corners (upper-left and lower-right) are implicitly derived from
/// the two diagonal corners and are not directly settable via VISCA commands
/// in this implementation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum PanTiltLimitCorner {
    /// Lower-left corner (minimum pan, minimum tilt).
    DownLeft,
    /// Upper-right corner (maximum pan, maximum tilt).
    UpRight,
}

impl PanTiltLimitCorner {
    /// Converts the corner to its VISCA byte representation.
    #[must_use]
    pub const fn to_byte(self) -> u8 {
        match self {
            Self::DownLeft => 0x00,
            Self::UpRight => 0x03,
        }
    }
}

/// Direction for pan/tilt movement commands.
///
/// Represents the 8 directional movements plus stop.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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

    use crate::{command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test};

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

    visca_test!(
        PanTilt,
        test_pan_tilt_limit_set_up_right,
        PanTilt::LimitSet {
            corner: PanTiltLimitCorner::UpRight,
            pan: PanPosition::new(0x0789).expect("valid test pan position"),
            // Use a valid TiltPosition value (range: -432 to 1296, i.e. 0xFE50 to 0x0510)
            tilt: TiltPosition::new(0x0456).expect("valid test tilt position"),
        },
        &[
            0x81, 0x01, 0x06, 0x07, 0x00, 0x03, 0x00, 0x07, 0x08, 0x09, 0x00, 0x04, 0x05, 0x06,
            0xFF
        ]
    );

    visca_test!(
        PanTilt,
        test_pan_tilt_limit_clear_up_right,
        PanTilt::LimitClear {
            corner: PanTiltLimitCorner::UpRight,
        },
        &[
            0x81, 0x01, 0x06, 0x07, 0x01, 0x03, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
            0xFF
        ]
    );

    // Golden frames for the movement encodings (issue #633).
    //
    // These vectors are hand-derived from the VISCA specification, not
    // recomputed from `PanTiltDirection::to_bytes` or from the position
    // pushes, so a swapped direction-table row or a transposed pan/tilt field
    // fails here even though every differential test stays green. The pan and
    // tilt speed fields are deliberately different (`0x18` vs `0x14`), and the
    // positional fixtures use dissimilar pan and tilt values, so a
    // transposition is never self-cancelling.

    /// Builds a directional move at maximum pan (`0x18`) and tilt (`0x14`) speed.
    fn golden_move(direction: PanTiltDirection) -> PanTilt {
        PanTilt::Move {
            direction,
            pan_speed: PanSpeed::new(0x18).expect("maximum pan speed"),
            tilt_speed: TiltSpeed::new(0x14).expect("maximum tilt speed"),
        }
    }

    visca_test!(
        PanTilt,
        test_pan_tilt_move_up_golden_frame,
        golden_move(PanTiltDirection::Up),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x03,
            0x01,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_move_down_golden_frame,
        golden_move(PanTiltDirection::Down),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x03,
            0x02,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_move_left_golden_frame,
        golden_move(PanTiltDirection::Left),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x01,
            0x03,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_move_right_golden_frame,
        golden_move(PanTiltDirection::Right),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x02,
            0x03,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_move_up_left_golden_frame,
        golden_move(PanTiltDirection::UpLeft),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x01,
            0x01,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_move_up_right_golden_frame,
        golden_move(PanTiltDirection::UpRight),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x02,
            0x01,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_move_down_left_golden_frame,
        golden_move(PanTiltDirection::DownLeft),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x01,
            0x02,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_move_down_right_golden_frame,
        golden_move(PanTiltDirection::DownRight),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x02,
            0x02,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_move_stop_golden_frame,
        golden_move(PanTiltDirection::Stop),
        &[
            0x81,
            0x01,
            0x06,
            0x01,
            0x18,
            0x14,
            0x03,
            0x03,
            VISCA_TERMINATOR
        ]
    );

    // Pan `+144` is `0x0090` (nibbles `00 00 09 00`); tilt `-72` is the
    // two's-complement `0xFFB8` (nibbles `0F 0F 0B 08`).
    visca_test!(
        PanTilt,
        test_pan_tilt_absolute_position_golden_frame,
        PanTilt::AbsolutePosition {
            pan: PanPosition::new(144).expect("valid test pan position"),
            tilt: TiltPosition::new(-72).expect("valid test tilt position"),
            pan_speed: PanSpeed::new(0x0A).expect("valid pan speed"),
            tilt_speed: TiltSpeed::new(0x05).expect("valid tilt speed"),
        },
        &[
            0x81,
            0x01,
            0x06,
            0x02,
            0x0A,
            0x05,
            0x00,
            0x00,
            0x09,
            0x00,
            0x0F,
            0x0F,
            0x0B,
            0x08,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_absolute_position_raw_golden_frame,
        PanTilt::AbsolutePositionRaw {
            pan_u16: 0x0090,
            tilt_u16: 0xFFB8,
            pan_speed: PanSpeed::new(0x0A).expect("valid pan speed"),
            tilt_speed: TiltSpeed::new(0x05).expect("valid tilt speed"),
        },
        &[
            0x81,
            0x01,
            0x06,
            0x02,
            0x0A,
            0x05,
            0x00,
            0x00,
            0x09,
            0x00,
            0x0F,
            0x0F,
            0x0B,
            0x08,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_relative_position_golden_frame,
        PanTilt::RelativePosition {
            pan: PanPosition::new(144).expect("valid test pan position"),
            tilt: TiltPosition::new(-72).expect("valid test tilt position"),
            pan_speed: PanSpeed::new(0x0A).expect("valid pan speed"),
            tilt_speed: TiltSpeed::new(0x05).expect("valid tilt speed"),
        },
        &[
            0x81,
            0x01,
            0x06,
            0x03,
            0x0A,
            0x05,
            0x00,
            0x00,
            0x09,
            0x00,
            0x0F,
            0x0F,
            0x0B,
            0x08,
            VISCA_TERMINATOR
        ]
    );
    visca_test!(
        PanTilt,
        test_pan_tilt_relative_position_raw_golden_frame,
        PanTilt::RelativePositionRaw {
            pan_u16: 0x0090,
            tilt_u16: 0xFFB8,
            pan_speed: PanSpeed::new(0x0A).expect("valid pan speed"),
            tilt_speed: TiltSpeed::new(0x05).expect("valid tilt speed"),
        },
        &[
            0x81,
            0x01,
            0x06,
            0x03,
            0x0A,
            0x05,
            0x00,
            0x00,
            0x09,
            0x00,
            0x0F,
            0x0F,
            0x0B,
            0x08,
            VISCA_TERMINATOR
        ]
    );

    /// Each direction must own a distinct `(pan, tilt)` byte pair.
    ///
    /// The golden frames above pin every direction to the right bytes; this
    /// rules out a table that collapses two directions onto one encoding.
    #[test]
    fn every_direction_owns_a_distinct_byte_pair() {
        let directions = [
            PanTiltDirection::Up,
            PanTiltDirection::Down,
            PanTiltDirection::Left,
            PanTiltDirection::Right,
            PanTiltDirection::UpLeft,
            PanTiltDirection::UpRight,
            PanTiltDirection::DownLeft,
            PanTiltDirection::DownRight,
            PanTiltDirection::Stop,
        ];
        for (index, direction) in directions.iter().enumerate() {
            for other in directions.iter().skip(index + 1) {
                assert_ne!(
                    direction.to_bytes(),
                    other.to_bytes(),
                    "{direction:?} and {other:?} share one direction byte pair"
                );
            }
        }
    }
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
    /// The pan and tilt values are pre-converted to the camera coordinate system.
    /// Prefer [`AbsolutePosition`](Self::AbsolutePosition) unless you are building
    /// a profile-aware conversion layer.
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
    /// The pan and tilt values are pre-converted to the camera coordinate system.
    /// Prefer [`RelativePosition`](Self::RelativePosition) unless you are building
    /// a profile-aware conversion layer.
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

impl WireEncode for PanTilt {
    const MAX_SIZE: usize = 15;

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
}
