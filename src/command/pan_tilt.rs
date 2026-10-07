//! Pan/Tilt control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera pan (horizontal) and tilt (vertical)
//! movement, including directional movement, absolute positioning, and relative positioning.
//!
//! # Speed Limits
//! - Pan speed: 0x00 to 0x18 (0-24 decimal)
//! - Tilt speed: 0x00 to 0x18 (0-24 decimal) syntactically; individual
//!   profiles commonly limit standard two-speed movement to 0x14.
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
        let mut bytes = [0_u8; 16];
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
                framing: PanTiltFraming::SonyBrc300,
                pan: 0x02490,
                tilt: -0x0C30,
                pan_speed: speed,
                tilt_speed,
            }),
            [
                0x81, 0x01, 0x06, 0x02, 0x09, 0x00, 0x00, 0x02, 0x04, 0x09, 0x00, 0x0F, 0x03, 0x0D,
                0x00, 0xFF,
            ]
        );

        // Manual p. 22's positive endpoint is `08A58`/`493D`.
        assert_eq!(
            profiled_bytes(PanTiltProfiled::LimitSet {
                framing: PanTiltFraming::SonyBrc300,
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
                framing: PanTiltFraming::SonyBrc300,
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
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    },
    RelativePosition {
        framing: PanTiltFraming,
        pan: i32,
        tilt: i32,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
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

/// The position framing selected by a profile's pan/tilt wire codec and
/// coordinate system.
///
/// Position commands, limit commands and the position inquiry all derive
/// their wire grammar from this one value, so the Sony BRC-300 rules (signed
/// coordinates only, a five-nibble signed pan, a four-nibble signed tilt and
/// the one-speed `VV 00` grammar) and the standard four-nibble rules each
/// live in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PanTiltFraming {
    /// Standard VISCA: four-nibble pan and tilt words in the given coordinate
    /// system.
    Standard(CoordinateSystem),
    /// Sony BRC-300: a five-nibble signed pan and a four-nibble signed tilt.
    SonyBrc300,
}

impl PanTiltFraming {
    /// The profile-less framing: standard words read as signed values.
    pub(crate) const STANDARD: Self = Self::Standard(CoordinateSystem::SignedCentered);

    /// Whether `codec` can frame `coordinate_system`. Sony BRC-300 framing is
    /// signed by definition; standard framing carries either system.
    pub(crate) const fn is_consistent(
        codec: PanTiltWireCodec,
        coordinate_system: CoordinateSystem,
    ) -> bool {
        !matches!(
            (codec, coordinate_system),
            (
                PanTiltWireCodec::SonyBrc300,
                CoordinateSystem::UnsignedCentered
            )
        )
    }

    /// The framing for `codec` in `coordinate_system`: the single rule every
    /// profile construction path applies to its coordinate conversion.
    ///
    /// # Errors
    ///
    /// Pairing Sony BRC-300 framing with unsigned-centered coordinates is an
    /// inconsistent profile and returns the profile-field error
    /// [`Error::InvalidRequest`] naming `pan_tilt_coordinates`.
    pub(crate) fn new(
        codec: PanTiltWireCodec,
        coordinate_system: CoordinateSystem,
    ) -> Result<Self, Error> {
        if Self::is_consistent(codec, coordinate_system) {
            Ok(Self::select(codec, coordinate_system))
        } else {
            Err(Error::InvalidRequest(
                "profile field `pan_tilt_coordinates`: Sony BRC-300 pan/tilt framing requires signed-centered coordinates"
                    .into(),
            ))
        }
    }

    /// The framing a profile's coordinate conversion selects. Every
    /// conversion passed [`Self::new`] when its profile was constructed.
    pub(crate) const fn for_conversion(conversion: crate::PanTiltCoordinateConversion) -> Self {
        Self::select(conversion.wire_codec(), conversion.coordinate_system())
    }

    /// The codec-led mapping: the codec picks the grammar and a standard
    /// grammar reads words in `coordinate_system`.
    const fn select(codec: PanTiltWireCodec, coordinate_system: CoordinateSystem) -> Self {
        match codec {
            PanTiltWireCodec::StandardVisca => Self::Standard(coordinate_system),
            PanTiltWireCodec::SonyBrc300 => Self::SonyBrc300,
        }
    }

    /// Writes the speed pair and the position words of an absolute or
    /// relative move.
    fn write_move(
        self,
        frame: FrameWriter<'_>,
        pan: i32,
        tilt: i32,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<FrameWriter<'_>, Error> {
        let frame = match self {
            Self::Standard(_) => frame.byte(pan_speed.value()).byte(tilt_speed.value()),
            // The one-speed `VV 00` grammar: the pan speed drives both axes.
            Self::SonyBrc300 => frame.byte(pan_speed.value()).byte(0x00),
        };
        self.write_position(frame, pan, tilt)
    }

    /// Writes the position words, refusing values the framing cannot carry.
    fn write_position(
        self,
        frame: FrameWriter<'_>,
        pan: i32,
        tilt: i32,
    ) -> Result<FrameWriter<'_>, Error> {
        match self {
            Self::Standard(coordinate_system) => {
                let pan = i16::try_from(pan).map_err(|_| {
                    Error::InvalidRequest(
                        format!(
                            "standard VISCA pan coordinate {pan} exceeds signed 16-bit framing"
                        )
                        .into(),
                    )
                })?;
                let tilt = i16::try_from(tilt).map_err(|_| {
                    Error::InvalidRequest(
                        format!(
                            "standard VISCA tilt coordinate {tilt} exceeds signed 16-bit framing"
                        )
                        .into(),
                    )
                })?;
                let (pan, tilt) = coordinate_system.to_camera_coords(pan, tilt);
                Ok(frame.nibbles::<4>(pan).nibbles::<4>(tilt))
            }
            Self::SonyBrc300 => {
                if !(-0x08_0000..=0x07_FFFF).contains(&pan) {
                    return Err(Error::InvalidRequest(
                        format!("Sony BRC-300 pan coordinate {pan} exceeds signed 20-bit framing")
                            .into(),
                    ));
                }
                let tilt = i16::try_from(tilt).map_err(|_| {
                    Error::InvalidRequest(
                        format!(
                            "Sony BRC-300 tilt coordinate {tilt} exceeds signed 16-bit framing"
                        )
                        .into(),
                    )
                })?;
                // Two's-complement words: the nibble writer keeps the low 20
                // and 16 bits.
                Ok(frame.nibbles::<5>(pan as u32).nibbles::<4>(tilt as u16))
            }
        }
    }

    /// Writes the cleared-limit position: the maximum positive word on each
    /// axis.
    fn write_cleared_limit(self, frame: FrameWriter<'_>) -> FrameWriter<'_> {
        match self {
            Self::Standard(_) => frame.nibbles::<4>(0x7FFF_u16).nibbles::<4>(0x7FFF_u16),
            Self::SonyBrc300 => frame.nibbles::<5>(0x7_FFFF_u32).nibbles::<4>(0x7FFF_u16),
        }
    }

    /// Decodes a position inquiry reply (`0w 0w 0w 0w 0z 0z 0z 0z`, or the
    /// BRC-300's five pan nibbles) to logical `(pan, tilt)`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidResponseLength`] for a payload of the wrong
    /// length and [`Error::InvalidResponseFormat`] for a byte that is not a
    /// nibble, whichever framing is selected.
    pub(crate) fn decode_position(self, payload: Payload<'_>) -> Result<(i32, i32), Error> {
        match self {
            Self::Standard(coordinate_system) => {
                let nibbles = Nibbles::<8>::try_from(payload)?;
                let (pan, tilt) = coordinate_system
                    .convert_from_camera_coords(nibbles.u16_quad(0), nibbles.u16_quad(4));
                Ok((i32::from(pan), i32::from(tilt)))
            }
            Self::SonyBrc300 => {
                let nibbles = Nibbles::<9>::try_from(payload)?;
                Ok((nibbles.i20_penta(0), i32::from(nibbles.i16_quad(5))))
            }
        }
    }
}

impl WireEncode for PanTiltProfiled {
    fn write_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        let frame = FrameWriter::new(camera_id, buffer);
        match *self {
            Self::AbsolutePosition {
                framing,
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => framing.write_move(
                frame.bytes(&pan_tilt::ABSOLUTE),
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            )?,
            Self::RelativePosition {
                framing,
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            } => framing.write_move(
                frame.bytes(&pan_tilt::RELATIVE),
                pan,
                tilt,
                pan_speed,
                tilt_speed,
            )?,
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
                    .write_cleared_limit(frame.bytes(&pan_tilt::LIMIT_CLEAR).byte(corner.to_byte()))
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
                pan_speed,
                tilt_speed,
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
                pan_speed,
                tilt_speed,
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
