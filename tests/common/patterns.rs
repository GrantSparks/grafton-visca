//! Common VISCA command and response patterns for testing.
//!
//! This module provides predefined byte sequences for common VISCA commands
//! and responses, making tests more readable and reducing duplication.

/// VISCA command terminator byte.
const VISCA_TERMINATOR: u8 = 0xFF;

/// Power command patterns
pub mod power {
    use super::VISCA_TERMINATOR;

    /// Power on command
    pub const ON: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR];

    /// Power standby command
    pub const STANDBY: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR];
}

/// Zoom command patterns
pub mod zoom {
    use super::VISCA_TERMINATOR;

    /// Zoom stop
    pub const STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, VISCA_TERMINATOR];

    /// Zoom tele (in) standard speed
    pub const TELE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR];

    /// Zoom wide (out) standard speed
    pub const WIDE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, VISCA_TERMINATOR];
}

/// Pan/Tilt command patterns
pub mod pan_tilt {
    use super::VISCA_TERMINATOR;

    /// Pan/tilt home position
    pub const HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, VISCA_TERMINATOR];

    /// Pan/tilt reset
    pub const RESET: &[u8] = &[0x81, 0x01, 0x06, 0x05, VISCA_TERMINATOR];

    /// Pan/tilt stop
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

    /// Pan/tilt up
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

    /// Pan/tilt down
    pub const DOWN: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x18,
        0x03,
        0x02,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt left
    pub const LEFT: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x18,
        0x01,
        0x03,
        VISCA_TERMINATOR,
    ];

    /// Pan/tilt right
    pub const RIGHT: &[u8] = &[
        0x81,
        0x01,
        0x06,
        0x01,
        0x18,
        0x18,
        0x02,
        0x03,
        VISCA_TERMINATOR,
    ];
}

/// Focus command patterns
pub mod focus {
    use super::VISCA_TERMINATOR;

    /// Focus far
    pub const FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, VISCA_TERMINATOR];

    /// Focus near
    pub const NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, VISCA_TERMINATOR];

    /// Focus auto
    pub const AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x38, 0x02, VISCA_TERMINATOR];

    /// Focus manual
    pub const MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x38, 0x03, VISCA_TERMINATOR];
}

/// Preset command patterns
pub mod preset {
    use super::VISCA_TERMINATOR;

    /// Preset set for position 0
    pub const SET_0: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x00, VISCA_TERMINATOR];

    /// Preset recall for position 0
    pub const RECALL_0: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x00, VISCA_TERMINATOR];

    /// Preset set for position 1
    pub const SET_1: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x01, VISCA_TERMINATOR];

    /// Preset recall for position 1
    pub const RECALL_1: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, VISCA_TERMINATOR];
}

/// Common response patterns
pub mod responses {
    use super::VISCA_TERMINATOR;

    /// ACK for socket 0
    pub const ACK_0: &[u8] = &[0x90, 0x40, VISCA_TERMINATOR];

    /// ACK for socket 1
    pub const ACK_1: &[u8] = &[0x90, 0x41, VISCA_TERMINATOR];

    /// ACK for socket 2 (some cameras support this)
    pub const ACK_2: &[u8] = &[0x90, 0x42, VISCA_TERMINATOR];

    /// Completion for socket 0
    pub const COMPLETE_0: &[u8] = &[0x90, 0x50, VISCA_TERMINATOR];

    /// Completion for socket 1
    pub const COMPLETE_1: &[u8] = &[0x90, 0x51, VISCA_TERMINATOR];

    /// Completion for socket 2
    pub const COMPLETE_2: &[u8] = &[0x90, 0x52, VISCA_TERMINATOR];

    /// Syntax error
    pub const SYNTAX_ERROR: &[u8] = &[0x90, 0x60, 0x02, VISCA_TERMINATOR];

    /// Command buffer full
    pub const BUFFER_FULL: &[u8] = &[0x90, 0x60, 0x03, VISCA_TERMINATOR];

    /// Command canceled
    pub const CANCELED: &[u8] = &[0x90, 0x60, 0x04, VISCA_TERMINATOR];

    /// No socket available
    pub const NO_SOCKET: &[u8] = &[0x90, 0x60, 0x05, VISCA_TERMINATOR];

    /// Command not executable
    pub const NOT_EXECUTABLE: &[u8] = &[0x90, 0x60, 0x41, VISCA_TERMINATOR];
}

/// Exposure command patterns
pub mod exposure {
    use super::VISCA_TERMINATOR;

    /// Auto exposure mode
    pub const AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x39, 0x00, VISCA_TERMINATOR];

    /// Manual exposure mode
    pub const MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x39, 0x03, VISCA_TERMINATOR];

    /// Shutter priority mode
    pub const SHUTTER_PRIORITY: &[u8] = &[0x81, 0x01, 0x04, 0x39, 0x0A, VISCA_TERMINATOR];

    /// Iris priority mode
    pub const IRIS_PRIORITY: &[u8] = &[0x81, 0x01, 0x04, 0x39, 0x0B, VISCA_TERMINATOR];
}

/// White balance command patterns
pub mod white_balance {
    use super::VISCA_TERMINATOR;

    /// Auto white balance
    pub const AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x00, VISCA_TERMINATOR];

    /// Indoor white balance
    pub const INDOOR: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x01, VISCA_TERMINATOR];

    /// Outdoor white balance
    pub const OUTDOOR: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x02, VISCA_TERMINATOR];

    /// One push white balance
    pub const ONE_PUSH: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x03, VISCA_TERMINATOR];

    /// Manual white balance
    pub const MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x05, VISCA_TERMINATOR];
}

/// System command patterns
pub mod system {
    use super::VISCA_TERMINATOR;

    /// Interface clear (reset)
    pub const IF_CLEAR: &[u8] = &[0x88, 0x01, 0x00, 0x01, VISCA_TERMINATOR];

    /// Command cancel for socket 1
    pub const CANCEL_1: &[u8] = &[0x81, 0x21, VISCA_TERMINATOR];

    /// Command cancel for socket 2
    pub const CANCEL_2: &[u8] = &[0x81, 0x22, VISCA_TERMINATOR];
}

/// Helper function to create power on response
pub fn power_on_response() -> Vec<u8> {
    vec![0x90, 0x50, 0x02, VISCA_TERMINATOR]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_patterns() {
        // Verify power commands have correct structure
        assert_eq!(power::ON[0] & 0xF0, 0x80); // Command header
        assert_eq!(power::ON[power::ON.len() - 1], 0xFF); // Terminator
    }

    #[test]
    fn test_response_patterns() {
        // Verify response patterns
        assert_eq!(responses::ACK_0[0], 0x90); // ViscaResponse header
        assert_eq!(responses::ACK_0[1] & 0xF0, 0x40); // ACK type

        assert_eq!(responses::COMPLETE_1[1] & 0xF0, 0x50); // Completion type
        assert_eq!(responses::COMPLETE_1[1] & 0x0F, 0x01); // Socket 1

        assert_eq!(responses::SYNTAX_ERROR[1], 0x60); // Error type
        assert_eq!(responses::SYNTAX_ERROR[2], 0x02); // Error code
    }
}
