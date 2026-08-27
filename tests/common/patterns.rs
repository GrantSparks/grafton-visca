//! Golden VISCA wire frames for the movement command surface.
//!
//! Every constant is an absolute, hand-derived frame taken from the VISCA
//! specification, not a value recomputed with the encoder under test. They are
//! consumed by `tests/issue_633_golden_wire_bytes.rs`, which compares them
//! byte-for-byte against the bytes the public request encoders produce for
//! [`CameraId::CAMERA_1`](grafton_visca::CameraId::CAMERA_1).
//!
//! Keep the pan/tilt speed fields asymmetric (`0x18` pan, `0x14` tilt) so a
//! transposition of the two speed bytes is visible, and keep the pan and tilt
//! nibble groups of the positional frames distinct for the same reason.

/// VISCA command terminator byte.
pub const VISCA_TERMINATOR: u8 = 0xFF;

/// Maximum pan speed accepted by the profiles used in the golden fixtures.
pub const PAN_SPEED_MAX: u8 = 0x18;

/// Maximum tilt speed accepted by the profiles used in the golden fixtures.
pub const TILT_SPEED_MAX: u8 = 0x14;

/// Power command patterns.
pub mod power {
    use super::VISCA_TERMINATOR;

    /// Power on command.
    pub const ON: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

    /// Power standby command.
    pub const STANDBY: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR];
}

/// Pan/tilt command patterns.
///
/// The directional frames use the maximum pan speed (`0x18`) and the maximum
/// tilt speed (`0x14`); the trailing two bytes are the pan and tilt direction
/// bytes in that order.
pub mod pan_tilt {
    use super::VISCA_TERMINATOR;

    /// Pan/tilt home position.
    pub const HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, VISCA_TERMINATOR];

    /// Pan/tilt reset.
    pub const RESET: &[u8] = &[0x81, 0x01, 0x06, 0x05, VISCA_TERMINATOR];

    /// Pan/tilt stop, driven with a distinct pan (`0x0C`) and tilt (`0x0A`)
    /// speed so a transposed speed pair is caught here too.
    pub const STOP: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x0C,
        0x0A,
        0x03,
        0x03,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt up: pan stops (`0x03`), tilt moves up (`0x01`).
    pub const UP: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x14,
        0x03,
        0x01,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt down: pan stops (`0x03`), tilt moves down (`0x02`).
    pub const DOWN: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x14,
        0x03,
        0x02,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt left: pan moves left (`0x01`), tilt stops (`0x03`).
    pub const LEFT: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x14,
        0x01,
        0x03,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt right: pan moves right (`0x02`), tilt stops (`0x03`).
    pub const RIGHT: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x14,
        0x02,
        0x03,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt up-left: pan left (`0x01`), tilt up (`0x01`).
    pub const UP_LEFT: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x14,
        0x01,
        0x01,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt up-right: pan right (`0x02`), tilt up (`0x01`).
    pub const UP_RIGHT: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x14,
        0x02,
        0x01,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt down-left: pan left (`0x01`), tilt down (`0x02`).
    pub const DOWN_LEFT: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x14,
        0x01,
        0x02,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt down-right: pan right (`0x02`), tilt down (`0x02`).
    pub const DOWN_RIGHT: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x14,
        0x02,
        0x02,
        VISCA_TERMINATOR,
    ];

    /// Absolute pan/tilt drive to pan `+144` units and tilt `-72` units at pan
    /// speed `0x0A` and tilt speed `0x05`.
    ///
    /// `+144` is `0x0090`, sent as the nibbles `00 00 09 00`; `-72` is the
    /// two's-complement `0xFFB8`, sent as the nibbles `0F 0F 0B 08`. The two
    /// groups are deliberately dissimilar so a pan/tilt transposition is not
    /// self-cancelling.
    pub const ABSOLUTE_PAN_144_TILT_NEG_72: &[u8] = &[
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
        VISCA_TERMINATOR,
    ];

    /// Relative pan/tilt drive with the same fields as
    /// [`ABSOLUTE_PAN_144_TILT_NEG_72`], on the relative opcode `0x03`.
    pub const RELATIVE_PAN_144_TILT_NEG_72: &[u8] = &[
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
        VISCA_TERMINATOR,
    ];
}

/// Zoom command patterns.
pub mod zoom {
    use super::VISCA_TERMINATOR;

    /// Zoom stop.
    pub const STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, VISCA_TERMINATOR];

    /// Zoom tele (in) standard speed.
    pub const TELE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR];

    /// Zoom wide (out) standard speed.
    pub const WIDE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, VISCA_TERMINATOR];

    /// Zoom tele at variable speed 5.
    pub const TELE_VARIABLE_5: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x25, VISCA_TERMINATOR];

    /// Zoom wide at variable speed 5.
    pub const WIDE_VARIABLE_5: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x35, VISCA_TERMINATOR];

    /// Direct zoom drive to `0x1234`, sent as the nibbles `01 02 03 04`.
    pub const DIRECT_0X1234: &[u8] = &[
        0x81,
        0x01,
        0x04,
        0x47,
        0x01,
        0x02,
        0x03,
        0x04,
        VISCA_TERMINATOR,
    ];
}

/// Focus command patterns.
pub mod focus {
    use super::VISCA_TERMINATOR;

    /// Focus stop.
    pub const STOP: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x00, VISCA_TERMINATOR];

    /// Focus far.
    pub const FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, VISCA_TERMINATOR];

    /// Focus near.
    pub const NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, VISCA_TERMINATOR];

    /// Focus far at variable speed 5.
    pub const FAR_VARIABLE_5: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x25, VISCA_TERMINATOR];

    /// Focus near at variable speed 5.
    pub const NEAR_VARIABLE_5: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x35, VISCA_TERMINATOR];

    /// Focus to infinity.
    pub const INFINITY: &[u8] = &[0x81, 0x01, 0x04, 0x18, 0x02, VISCA_TERMINATOR];

    /// Focus auto.
    pub const AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x38, 0x02, VISCA_TERMINATOR];

    /// Focus manual.
    pub const MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x38, 0x03, VISCA_TERMINATOR];

    /// Direct focus drive to `0x2345`, sent as the nibbles `02 03 04 05`.
    pub const DIRECT_0X2345: &[u8] = &[
        0x81,
        0x01,
        0x04,
        0x48,
        0x02,
        0x03,
        0x04,
        0x05,
        VISCA_TERMINATOR,
    ];
}

/// Preset command patterns.
pub mod preset {
    use super::VISCA_TERMINATOR;

    /// Preset set for slot 1.
    pub const SET_1: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x01, VISCA_TERMINATOR];

    /// Preset recall for slot 1.
    pub const RECALL_1: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, VISCA_TERMINATOR];

    /// Preset reset (clear) for slot 1.
    pub const RESET_1: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x00, 0x01, VISCA_TERMINATOR];
}
