//! Pan/Tilt control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera pan (horizontal) and tilt (vertical)
//! movement, including directional movement, absolute positioning, and relative positioning.
//!
//! # Speed Limits
//! - Pan speed: 0x00 to 0x32 (0-50 decimal), subject to profile and mode.
//! - Tilt speed: 0x00 to 0x32 (0-50 decimal) syntactically; individual
//!   profiles commonly limit standard two-speed movement to 0x14.
//!
//! # VISCA Compliance
//! All commands in this module are part of the baseline VISCA specification and should be
//! supported by all VISCA-compliant cameras.
//! # Example
//! ```rust
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
    capabilities::{CoordinateSystem, PanTiltWireCodec},
    command::{
        bytes::{constants::pan_tilt, FrameWriter},
        encode::WireEncode,
        response::{Nibbles, Payload},
    },
    error::Error,
    types::{PanSpeed, TiltSpeed},
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
            Self::UpRight => 0x01,
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
            pan: 0x0123,
            tilt: 0x0456,
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
            pan: 0x0789,
            tilt: 0x0456,
        },
        &[
            0x81, 0x01, 0x06, 0x07, 0x00, 0x01, 0x00, 0x07, 0x08, 0x09, 0x00, 0x04, 0x05, 0x06,
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
            0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F, 0x0F,
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
            pan: 144,
            tilt: -72,
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
            pan: 144,
            tilt: -72,
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

    fn profiled_bytes(command: PanTiltProfiled) -> Vec<u8> {
        let mut bytes = [0_u8; 17];
        let written = command
            .write_into(crate::CameraId::CAMERA_1, &mut bytes)
            .expect("profiled pan/tilt test command should encode");
        bytes[..written].to_vec()
    }

    #[test]
    fn sony_brc300_profiled_frames_preserve_five_nibble_pan_endpoints() {
        let speed = PanSpeed::new(0x09).expect("valid BRC-300 speed");
        let tilt_speed = TiltSpeed::new(0x09).expect("matching paired API speed");
        assert_eq!(
            profiled_bytes(PanTiltProfiled::AbsolutePosition {
                framing: PanTiltFraming::select(
                    PanTiltWireCodec::SonyBrc300,
                    CoordinateSystem::SignedCentered
                ),
                pan: 0x02490,
                tilt: -0x0C30,
                speeds: PanTiltFraming::select(
                    PanTiltWireCodec::SonyBrc300,
                    CoordinateSystem::SignedCentered
                )
                .move_speeds(speed, tilt_speed)
                .expect("matching speeds"),
            }),
            [
                0x81, 0x01, 0x06, 0x02, 0x09, 0x00, 0x00, 0x02, 0x04, 0x09, 0x00, 0x0F, 0x03, 0x0D,
                0x00, 0xFF,
            ]
        );

        // Manual p. 22's positive endpoint is `08A58`/`493D`.
        assert_eq!(
            profiled_bytes(PanTiltProfiled::LimitSet {
                framing: PanTiltFraming::select(
                    PanTiltWireCodec::SonyBrc300,
                    CoordinateSystem::SignedCentered
                ),
                corner: PanTiltLimitCorner::UpRight,
                pan: 0x08A58,
                tilt: 0x493D,
            }),
            [
                0x81, 0x01, 0x06, 0x07, 0x00, 0x01, 0x00, 0x08, 0x0A, 0x05, 0x08, 0x04, 0x09, 0x03,
                0x0D, 0xFF,
            ]
        );

        // `F75A8` and `E796` are the documented signed negative endpoints.
        assert_eq!(
            profiled_bytes(PanTiltProfiled::LimitSet {
                framing: PanTiltFraming::select(
                    PanTiltWireCodec::SonyBrc300,
                    CoordinateSystem::SignedCentered
                ),
                corner: PanTiltLimitCorner::DownLeft,
                pan: -0x08A58,
                tilt: -0x186A,
            }),
            [
                0x81, 0x01, 0x06, 0x07, 0x00, 0x00, 0x0F, 0x07, 0x05, 0x0A, 0x08, 0x0E, 0x07, 0x09,
                0x06, 0xFF,
            ]
        );

        assert_eq!(
            profiled_bytes(PanTiltProfiled::LimitClear {
                codec: PanTiltWireCodec::SonyBrc300,
                corner: PanTiltLimitCorner::UpRight,
            }),
            [
                0x81, 0x01, 0x06, 0x07, 0x01, 0x01, 0x07, 0x0F, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F,
                0x0F, 0xFF,
            ]
        );
    }
    #[test]
    fn fr7_pan_tilt_five_word_frames_and_replies_match_r7() {
        let framing =
            PanTiltFraming::new(PanTiltWireCodec::SonyFr7, CoordinateSystem::SignedCentered)
                .expect("FR7 grammar");
        let speeds = framing
            .move_speeds(
                PanSpeed::new(9).expect("speed"),
                TiltSpeed::new(5).expect("speed"),
            )
            .expect("paired speeds");
        let bytes = profiled_bytes(PanTiltProfiled::AbsolutePosition {
            framing,
            pan: 0x09CA7,
            tilt: 0x0B3B0,
            speeds,
        });
        assert_eq!(
            bytes,
            [0x81, 1, 6, 2, 9, 5, 0, 9, 12, 10, 7, 0, 11, 3, 11, 0, 255]
        );
        assert_eq!(
            framing
                .decode_position(Payload::new(&bytes[6..16]))
                .expect("R7 reply"),
            (40103, 46000)
        );
        let negative = profiled_bytes(PanTiltProfiled::RelativePosition {
            framing,
            pan: -0x09CA7,
            tilt: -7077,
            speeds,
        });
        assert_eq!(&negative[6..16], [15, 6, 3, 5, 9, 15, 14, 4, 5, 11]);
        let clear = profiled_bytes(PanTiltProfiled::LimitClear {
            codec: PanTiltWireCodec::SonyFr7,
            corner: PanTiltLimitCorner::UpRight,
        });
        assert_eq!(&clear[6..16], [7, 15, 15, 15, 15, 7, 15, 15, 15, 15]);
        assert!(framing
            .decode_position(Payload::new(&bytes[6..15]))
            .is_err());
    }

    #[test]
    fn pan_tilt_word_width_boundaries_round_trip_and_reject_overflow() {
        for codec in [
            PanTiltWireCodec::StandardVisca,
            PanTiltWireCodec::SonyBrc300,
            PanTiltWireCodec::SonyFr7,
        ] {
            let framing = PanTiltFraming::new(codec, CoordinateSystem::SignedCentered)
                .expect("signed framing");
            for (pan, tilt) in [
                (-framing.pan.maximum() - 1, -framing.tilt.maximum() - 1),
                (framing.pan.maximum(), framing.tilt.maximum()),
            ] {
                let mut buffer = [0; 12];
                let count = framing
                    .write_offset(
                        FrameWriter::new(crate::CameraId::CAMERA_1, &mut buffer),
                        pan,
                        tilt,
                    )
                    .expect("bounded words")
                    .finish()
                    .expect("frame capacity");
                assert_eq!(
                    framing
                        .decode_position(Payload::new(&buffer[1..count - 1]))
                        .expect("decode"),
                    (pan, tilt)
                );
            }
            let mut buffer = [0; 12];
            assert!(framing
                .write_offset(
                    FrameWriter::new(crate::CameraId::CAMERA_1, &mut buffer),
                    framing.pan.maximum() + 1,
                    0
                )
                .is_err());
        }
    }

    #[test]
    fn brc300_pan_tilt_speed_shape_refuses_a_discarded_tilt_field() {
        let framing = PanTiltFraming::new(
            PanTiltWireCodec::SonyBrc300,
            CoordinateSystem::SignedCentered,
        )
        .expect("BRC framing");
        assert!(framing
            .move_speeds(
                PanSpeed::new(9).expect("pan"),
                TiltSpeed::new(1).expect("tilt")
            )
            .is_err());
        assert!(matches!(
            framing.move_speeds(
                PanSpeed::new(9).expect("pan"),
                TiltSpeed::new(9).expect("tilt")
            ),
            Ok(MoveSpeeds::Single(_))
        ));
    }
}

/// Pan/Tilt movement commands.
///
/// Provides various ways to control camera pan and tilt:
/// - `Home` - Return to home position
/// - `Reset` - Reset pan/tilt mechanism
/// - `Move` - Directional movement with speed control
/// - `AbsolutePosition` - Move to raw coordinates
/// - `RelativePosition` - Move by raw offsets
/// - `LimitSet` - Set a pan/tilt movement boundary
/// - `LimitClear` - Clear a pan/tilt movement boundary
///
/// This is the profile-less standard VISCA frame. Its position fields are the
/// raw 16-bit words the frame carries (two's complement; an unsigned-centered
/// camera's word `w` is `w as i16`), not angles: units per degree and the
/// usable range belong to the camera profile. Profile-aware requests such as
/// [`PanTiltAbsolute`](crate::request::builtin::PanTiltAbsolute) take degrees,
/// convert them with the profile's scale, and validate the profile's range.
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
        /// Tilt movement speed (0x00-0x18 syntax; profile capability applies).
        tilt_speed: TiltSpeed,
    },
    /// Move camera to an absolute pan/tilt position in raw units.
    AbsolutePosition {
        /// Raw pan word.
        pan: i16,
        /// Raw tilt word.
        tilt: i16,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x18 syntax; profile capability applies).
        tilt_speed: TiltSpeed,
    },
    /// Move camera by a raw pan/tilt offset from its current position.
    RelativePosition {
        /// Raw pan offset.
        pan: i16,
        /// Raw tilt offset.
        tilt: i16,
        /// Pan movement speed (0x00-0x18).
        pan_speed: PanSpeed,
        /// Tilt movement speed (0x00-0x18 syntax; profile capability applies).
        tilt_speed: TiltSpeed,
    },
    /// Set one corner of the pan/tilt movement boundary in raw units.
    LimitSet {
        /// Which corner of the movement range to set.
        corner: PanTiltLimitCorner,
        /// Raw pan word for the limit.
        pan: i16,
        /// Raw tilt word for the limit.
        tilt: i16,
    },
    /// Clear pan/tilt movement boundaries.
    ///
    /// Clears a specified corner limit or all limits.
    LimitClear {
        /// Which corner to clear (same encoding as LimitSet).
        corner: PanTiltLimitCorner,
    },
}

/// Sole position/limit encoder used by both public commands and typed requests.
///
/// The public [`PanTilt`] command remains the baseline VISCA representation.
/// It lowers through this crate-private discriminator with the standard
/// framing; typed requests supply the framing their profile's validated
/// conversion selects, so each wire grammar has one implementation.
#[derive(Debug, Copy, Clone)]
pub(crate) enum PanTiltProfiled {
    AbsolutePosition {
        framing: PanTiltFraming,
        pan: i32,
        tilt: i32,
        speeds: MoveSpeeds,
    },
    RelativePosition {
        framing: PanTiltFraming,
        pan: i32,
        tilt: i32,
        speeds: MoveSpeeds,
    },
    LimitSet {
        framing: PanTiltFraming,
        corner: PanTiltLimitCorner,
        pan: i32,
        tilt: i32,
    },
    LimitClear {
        codec: PanTiltWireCodec,
        corner: PanTiltLimitCorner,
    },
}

/// The signed word width of one position axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PositionWord {
    Four,
    Five,
}

impl PositionWord {
    const fn len(self) -> usize {
        match self {
            Self::Four => 4,
            Self::Five => 5,
        }
    }

    const fn maximum(self) -> i32 {
        match self {
            Self::Four => 0x7FFF,
            Self::Five => 0x7_FFFF,
        }
    }

    fn write(self, frame: FrameWriter<'_>, value: i32) -> Result<FrameWriter<'_>, Error> {
        if !(-self.maximum() - 1..=self.maximum()).contains(&value) {
            return Err(Error::InvalidRequest(
                format!(
                    "pan/tilt coordinate {value} exceeds signed {}-bit framing",
                    self.len() * 4
                )
                .into(),
            ));
        }
        Ok(match self {
            Self::Four => frame.nibbles::<4>(value as u16),
            Self::Five => frame.nibbles::<5>(value as u32),
        })
    }

    fn decode(self, bytes: &[u8]) -> Result<i32, Error> {
        match self {
            Self::Four => Nibbles::<4>::try_from(Payload::new(bytes))?
                .i16_quad(0)
                .map(i32::from),
            Self::Five => Nibbles::<5>::try_from(Payload::new(bytes))?.i20_penta(0),
        }
        .ok_or(Error::InvalidResponseFormat)
    }
}

/// A position grammar carries exactly the speed fields it writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoveSpeeds {
    Paired { pan: PanSpeed, tilt: TiltSpeed },
    Single(PanSpeed),
}

impl MoveSpeeds {
    pub(crate) const fn values(self) -> (u8, u8) {
        match self {
            Self::Paired { pan, tilt } => (pan.value(), tilt.value()),
            Self::Single(pan) => (pan.value(), pan.value()),
        }
    }

    fn write(self, frame: FrameWriter<'_>) -> FrameWriter<'_> {
        match self {
            Self::Paired { pan, tilt } => frame.byte(pan.value()).byte(tilt.value()),
            Self::Single(speed) => frame.byte(speed.value()).byte(0),
        }
    }
}

/// Shared per-axis grammar for position commands, limits and position replies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PanTiltFraming {
    coordinate_system: CoordinateSystem,
    pan: PositionWord,
    tilt: PositionWord,
    single_speed: bool,
}

impl PanTiltFraming {
    pub(crate) const STANDARD: Self = Self::select(
        PanTiltWireCodec::StandardVisca,
        CoordinateSystem::SignedCentered,
    );

    pub(crate) const fn is_consistent(
        codec: PanTiltWireCodec,
        coordinate_system: CoordinateSystem,
    ) -> bool {
        matches!(codec, PanTiltWireCodec::StandardVisca)
            || matches!(coordinate_system, CoordinateSystem::SignedCentered)
    }

    pub(crate) fn new(
        codec: PanTiltWireCodec,
        coordinate_system: CoordinateSystem,
    ) -> Result<Self, Error> {
        if !Self::is_consistent(codec, coordinate_system) {
            return Err(Error::InvalidRequest(
                "profile field `pan_tilt_coordinates`: Sony position framing requires signed-centered coordinates".into(),
            ));
        }
        Ok(Self::select(codec, coordinate_system))
    }

    pub(crate) const fn for_conversion(conversion: crate::PanTiltCoordinateConversion) -> Self {
        Self::select(conversion.wire_codec(), conversion.coordinate_system())
    }

    const fn select(codec: PanTiltWireCodec, coordinate_system: CoordinateSystem) -> Self {
        let (pan, tilt, single_speed) = match codec {
            PanTiltWireCodec::StandardVisca => (PositionWord::Four, PositionWord::Four, false),
            PanTiltWireCodec::SonyBrc300 => (PositionWord::Five, PositionWord::Four, true),
            PanTiltWireCodec::SonyFr7 => (PositionWord::Five, PositionWord::Five, false),
        };
        Self {
            coordinate_system,
            pan,
            tilt,
            single_speed,
        }
    }

    /// Build the only speed shape that the selected grammar can encode.
    pub(crate) fn move_speeds(self, pan: PanSpeed, tilt: TiltSpeed) -> Result<MoveSpeeds, Error> {
        if self.single_speed {
            if pan.value() != tilt.value() {
                return Err(Error::InvalidRequest(
                    "Sony BRC-300 absolute and relative position commands have one speed byte; pan and tilt speeds must match".into(),
                ));
            }
            Ok(MoveSpeeds::Single(pan))
        } else {
            Ok(MoveSpeeds::Paired { pan, tilt })
        }
    }

    fn write_position(
        self,
        frame: FrameWriter<'_>,
        pan: i32,
        tilt: i32,
    ) -> Result<FrameWriter<'_>, Error> {
        // Absolute unsigned-centered words carry the 0x8000 bias; relative
        // offsets always use the distinct signed-word path below.
        if matches!(self.coordinate_system, CoordinateSystem::UnsignedCentered) {
            let pan = i16::try_from(pan)
                .map_err(|_| Error::InvalidRequest("pan exceeds signed 16-bit framing".into()))?;
            let tilt = i16::try_from(tilt)
                .map_err(|_| Error::InvalidRequest("tilt exceeds signed 16-bit framing".into()))?;
            let (pan, tilt) = self.coordinate_system.to_camera_coords(pan, tilt);
            Ok(frame.nibbles::<4>(pan).nibbles::<4>(tilt))
        } else {
            self.write_offset(frame, pan, tilt)
        }
    }

    fn write_offset(
        self,
        frame: FrameWriter<'_>,
        pan: i32,
        tilt: i32,
    ) -> Result<FrameWriter<'_>, Error> {
        self.tilt.write(self.pan.write(frame, pan)?, tilt)
    }

    fn write_cleared_limit(self, frame: FrameWriter<'_>) -> Result<FrameWriter<'_>, Error> {
        self.write_offset(frame, self.pan.maximum(), self.tilt.maximum())
    }

    /// Number of nibble bytes in this grammar's position reply.
    pub(crate) const fn position_nibbles(self) -> usize {
        self.pan.len() + self.tilt.len()
    }

    pub(crate) fn decode_position(self, payload: Payload<'_>) -> Result<(i32, i32), Error> {
        let bytes = payload.as_slice();
        let expected = self.position_nibbles();
        if bytes.len() != expected {
            return Err(Error::invalid_response_length(expected, bytes));
        }
        if matches!(self.coordinate_system, CoordinateSystem::UnsignedCentered) {
            let nibbles = Nibbles::<8>::try_from(payload)?;
            let (pan, tilt) = self.coordinate_system.convert_from_camera_coords(
                nibbles.u16_quad(0).ok_or(Error::InvalidResponseFormat)?,
                nibbles.u16_quad(4).ok_or(Error::InvalidResponseFormat)?,
            );
            Ok((i32::from(pan), i32::from(tilt)))
        } else {
            let (pan, tilt) = bytes.split_at(self.pan.len());
            Ok((self.pan.decode(pan)?, self.tilt.decode(tilt)?))
        }
    }
}

impl WireEncode for PanTiltProfiled {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        if let Self::LimitClear { codec, .. } = *self {
            let required = codec.maximum_position_frame_bytes();
            if buffer.len() < required {
                return Err(Error::BufferTooSmall {
                    required,
                    actual: buffer.len(),
                });
            }
        }
        let frame = FrameWriter::new(camera_id, buffer);
        match *self {
            Self::AbsolutePosition {
                framing,
                pan,
                tilt,
                speeds,
            } => {
                framing.write_position(speeds.write(frame.bytes(&pan_tilt::ABSOLUTE)), pan, tilt)?
            }
            Self::RelativePosition {
                framing,
                pan,
                tilt,
                speeds,
            } => framing.write_offset(speeds.write(frame.bytes(&pan_tilt::RELATIVE)), pan, tilt)?,
            Self::LimitSet {
                framing,
                corner,
                pan,
                tilt,
            } => framing.write_position(
                frame.bytes(&pan_tilt::LIMIT_SET).byte(corner.to_byte()),
                pan,
                tilt,
            )?,
            Self::LimitClear { codec, corner } => {
                PanTiltFraming::select(codec, CoordinateSystem::SignedCentered)
                    .write_cleared_limit(
                        frame.bytes(&pan_tilt::LIMIT_CLEAR).byte(corner.to_byte()),
                    )?
            }
        }
        .finish()
    }
}

impl WireEncode for PanTilt {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        match *self {
            Self::Home => FrameWriter::new(camera_id, buffer)
                .bytes(&pan_tilt::HOME)
                .finish(),
            Self::Reset => FrameWriter::new(camera_id, buffer)
                .bytes(&pan_tilt::RESET)
                .finish(),
            Self::Move {
                direction,
                pan_speed,
                tilt_speed,
            } => {
                let (pan, tilt) = direction.to_bytes();
                FrameWriter::new(camera_id, buffer)
                    .bytes(&pan_tilt::DRIVE)
                    .byte(pan_speed.value())
                    .byte(tilt_speed.value())
                    .byte(pan)
                    .byte(tilt)
                    .finish()
            }
            Self::AbsolutePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => PanTiltProfiled::AbsolutePosition {
                framing: PanTiltFraming::STANDARD,
                pan: i32::from(pan),
                tilt: i32::from(tilt),
                speeds: MoveSpeeds::Paired {
                    pan: pan_speed,
                    tilt: tilt_speed,
                },
            }
            .write_into(camera_id, buffer),
            Self::RelativePosition {
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => PanTiltProfiled::RelativePosition {
                framing: PanTiltFraming::STANDARD,
                pan: i32::from(pan),
                tilt: i32::from(tilt),
                speeds: MoveSpeeds::Paired {
                    pan: pan_speed,
                    tilt: tilt_speed,
                },
            }
            .write_into(camera_id, buffer),
            Self::LimitSet { corner, pan, tilt } => PanTiltProfiled::LimitSet {
                framing: PanTiltFraming::STANDARD,
                corner,
                pan: i32::from(pan),
                tilt: i32::from(tilt),
            }
            .write_into(camera_id, buffer),
            Self::LimitClear { corner } => PanTiltProfiled::LimitClear {
                codec: PanTiltWireCodec::StandardVisca,
                corner,
            }
            .write_into(camera_id, buffer),
        }
    }
}
