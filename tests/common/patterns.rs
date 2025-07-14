//! Common VISCA command and response patterns for testing.
//!
//! This module provides predefined byte sequences for common VISCA commands
//! and responses, making tests more readable and reducing duplication.

/// Power command patterns
pub mod power {
    /// Power on command
    pub const ON: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];

    /// Power standby command
    pub const STANDBY: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x03, 0xFF];

    /// Power inquiry command
    pub const INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x00, 0xFF];
}

/// Zoom command patterns
pub mod zoom {
    /// Zoom stop
    pub const STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF];

    /// Zoom tele (in) standard speed
    pub const TELE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];

    /// Zoom wide (out) standard speed
    pub const WIDE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xFF];

    /// Zoom position inquiry
    pub const POSITION_INQ: &[u8] = &[0x81, 0x09, 0x04, 0x47, 0xFF];
}

/// Pan/Tilt command patterns
pub mod pan_tilt {
    /// Pan/tilt home position
    pub const HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xFF];

    /// Pan/tilt reset
    pub const RESET: &[u8] = &[0x81, 0x01, 0x06, 0x05, 0xFF];

    /// Pan/tilt stop
    pub const STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x00, 0x00, 0x03, 0x03, 0xFF];

    /// Pan/tilt up
    pub const UP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x18, 0x14, 0x03, 0x01, 0xFF];

    /// Pan/tilt down
    pub const DOWN: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x18, 0x18, 0x03, 0x02, 0xFF];

    /// Pan/tilt left
    pub const LEFT: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x18, 0x18, 0x01, 0x03, 0xFF];

    /// Pan/tilt right
    pub const RIGHT: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x18, 0x18, 0x02, 0x03, 0xFF];

    /// Pan/tilt position inquiry
    pub const POSITION_INQ: &[u8] = &[0x81, 0x09, 0x06, 0x12, 0xFF];
}

/// Focus command patterns
pub mod focus {
    /// Focus far
    pub const FAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x02, 0xFF];

    /// Focus near
    pub const NEAR: &[u8] = &[0x81, 0x01, 0x04, 0x08, 0x03, 0xFF];

    /// Focus auto
    pub const AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x38, 0x02, 0xFF];

    /// Focus manual
    pub const MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x38, 0x03, 0xFF];

    /// Focus position inquiry
    pub const POSITION_INQ: &[u8] = &[0x81, 0x09, 0x04, 0x48, 0xFF];
}

/// Preset command patterns
pub mod preset {
    /// Preset set for position 0
    pub const SET_0: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x00, 0xFF];

    /// Preset recall for position 0
    pub const RECALL_0: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x00, 0xFF];

    /// Preset set for position 1
    pub const SET_1: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x01, 0x01, 0xFF];

    /// Preset recall for position 1
    pub const RECALL_1: &[u8] = &[0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, 0xFF];
}

/// Common response patterns
pub mod responses {
    /// ACK for socket 0
    pub const ACK_0: &[u8] = &[0x90, 0x40, 0xFF];

    /// ACK for socket 1
    pub const ACK_1: &[u8] = &[0x90, 0x41, 0xFF];

    /// ACK for socket 2 (some cameras support this)
    pub const ACK_2: &[u8] = &[0x90, 0x42, 0xFF];

    /// Completion for socket 0
    pub const COMPLETE_0: &[u8] = &[0x90, 0x50, 0xFF];

    /// Completion for socket 1
    pub const COMPLETE_1: &[u8] = &[0x90, 0x51, 0xFF];

    /// Completion for socket 2
    pub const COMPLETE_2: &[u8] = &[0x90, 0x52, 0xFF];

    /// Syntax error
    pub const SYNTAX_ERROR: &[u8] = &[0x90, 0x60, 0x02, 0xFF];

    /// Command buffer full
    pub const BUFFER_FULL: &[u8] = &[0x90, 0x60, 0x03, 0xFF];

    /// Command canceled
    pub const CANCELED: &[u8] = &[0x90, 0x60, 0x04, 0xFF];

    /// No socket available
    pub const NO_SOCKET: &[u8] = &[0x90, 0x60, 0x05, 0xFF];

    /// Command not executable
    pub const NOT_EXECUTABLE: &[u8] = &[0x90, 0x60, 0x41, 0xFF];
}

/// Exposure command patterns
pub mod exposure {
    /// Auto exposure mode
    pub const AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x39, 0x00, 0xFF];

    /// Manual exposure mode
    pub const MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x39, 0x03, 0xFF];

    /// Shutter priority mode
    pub const SHUTTER_PRIORITY: &[u8] = &[0x81, 0x01, 0x04, 0x39, 0x0A, 0xFF];

    /// Iris priority mode
    pub const IRIS_PRIORITY: &[u8] = &[0x81, 0x01, 0x04, 0x39, 0x0B, 0xFF];

    /// Exposure mode inquiry
    pub const MODE_INQ: &[u8] = &[0x81, 0x09, 0x04, 0x39, 0xFF];
}

/// White balance command patterns
pub mod white_balance {
    /// Auto white balance
    pub const AUTO: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x00, 0xFF];

    /// Indoor white balance
    pub const INDOOR: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x01, 0xFF];

    /// Outdoor white balance
    pub const OUTDOOR: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x02, 0xFF];

    /// One push white balance
    pub const ONE_PUSH: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x03, 0xFF];

    /// Manual white balance
    pub const MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x35, 0x05, 0xFF];

    /// White balance mode inquiry
    pub const MODE_INQ: &[u8] = &[0x81, 0x09, 0x04, 0x35, 0xFF];
}

/// System command patterns
pub mod system {
    /// System information inquiry
    pub const INFO_INQ: &[u8] = &[0x81, 0x09, 0x00, 0x02, 0xFF];

    /// Interface clear (reset)
    pub const IF_CLEAR: &[u8] = &[0x88, 0x01, 0x00, 0x01, 0xFF];

    /// Command cancel for socket 1
    pub const CANCEL_1: &[u8] = &[0x81, 0x21, 0xFF];

    /// Command cancel for socket 2
    pub const CANCEL_2: &[u8] = &[0x81, 0x22, 0xFF];
}

/// Inquiry command patterns - all inquiry commands in one place
pub mod inquiry {
    /// Power state inquiry
    pub const POWER: &[u8] = &[0x81, 0x09, 0x04, 0x00, 0xFF];

    /// Zoom position inquiry
    pub const ZOOM_POSITION: &[u8] = &[0x81, 0x09, 0x04, 0x47, 0xFF];

    /// Pan/tilt position inquiry
    pub const PAN_TILT_POSITION: &[u8] = &[0x81, 0x09, 0x06, 0x12, 0xFF];

    /// Focus position inquiry
    pub const FOCUS_POSITION: &[u8] = &[0x81, 0x09, 0x04, 0x48, 0xFF];

    /// Focus near limit inquiry
    pub const FOCUS_NEAR_LIMIT: &[u8] = &[0x81, 0x09, 0x04, 0x28, 0xFF];

    /// Exposure mode inquiry
    pub const EXPOSURE_MODE: &[u8] = &[0x81, 0x09, 0x04, 0x39, 0xFF];

    /// White balance mode inquiry
    pub const WHITE_BALANCE_MODE: &[u8] = &[0x81, 0x09, 0x04, 0x35, 0xFF];

    /// Anti-flicker mode inquiry
    pub const ANTI_FLICKER: &[u8] = &[0x81, 0x09, 0x04, 0x23, 0xFF];

    /// Focus zone inquiry
    pub const FOCUS_ZONE: &[u8] = &[0x81, 0x09, 0x04, 0x3C, 0xFF];

    /// Auto focus sensitivity inquiry
    pub const AUTO_FOCUS_SENSITIVITY: &[u8] = &[0x81, 0x09, 0x04, 0x58, 0xFF];

    /// Sharpness mode inquiry
    pub const SHARPNESS_MODE: &[u8] = &[0x81, 0x09, 0x04, 0x42, 0xFF];

    /// Sharpness value inquiry
    pub const SHARPNESS: &[u8] = &[0x81, 0x09, 0x04, 0x42, 0xFF];

    /// Gain limit inquiry
    pub const GAIN_LIMIT: &[u8] = &[0x81, 0x09, 0x04, 0x2C, 0xFF];

    /// Red gain inquiry
    pub const RED_GAIN: &[u8] = &[0x81, 0x09, 0x04, 0x43, 0xFF];

    /// Blue gain inquiry
    pub const BLUE_GAIN: &[u8] = &[0x81, 0x09, 0x04, 0x44, 0xFF];

    /// Exposure compensation mode inquiry
    pub const EXPOSURE_COMPENSATION_MODE: &[u8] = &[0x81, 0x09, 0x04, 0x3E, 0xFF];

    /// Exposure compensation value inquiry
    pub const EXPOSURE_COMPENSATION: &[u8] = &[0x81, 0x09, 0x04, 0x4E, 0xFF];

    /// Black/white mode inquiry
    pub const BLACK_WHITE: &[u8] = &[0x81, 0x09, 0x04, 0x3F, 0xFF];

    /// Image flip inquiry
    pub const IMAGE_FLIP: &[u8] = &[0x81, 0x09, 0x04, 0x61, 0xFF];

    /// Backlight compensation inquiry
    pub const BACKLIGHT: &[u8] = &[0x81, 0x09, 0x04, 0x33, 0xFF];

    /// Luminance inquiry
    pub const LUMINANCE: &[u8] = &[0x81, 0x09, 0x04, 0x4D, 0xFF];

    /// Contrast inquiry
    pub const CONTRAST: &[u8] = &[0x81, 0x09, 0x04, 0x4C, 0xFF];

    /// Shutter speed inquiry
    pub const SHUTTER: &[u8] = &[0x81, 0x09, 0x04, 0x4A, 0xFF];

    /// Iris position inquiry
    pub const IRIS: &[u8] = &[0x81, 0x09, 0x04, 0x4B, 0xFF];
}

/// Helper function to create power on response
pub fn power_on_response() -> Vec<u8> {
    vec![0x90, 0x50, 0x02, 0xFF]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_patterns() {
        // Verify power commands have correct structure
        assert_eq!(power::ON[0] & 0xF0, 0x80); // Command header
        assert_eq!(power::ON[power::ON.len() - 1], 0xFF); // Terminator

        assert_eq!(power::INQUIRY[1], 0x09); // Inquiry type
    }

    #[test]
    fn test_response_patterns() {
        // Verify response patterns
        assert_eq!(responses::ACK_0[0], 0x90); // Response header
        assert_eq!(responses::ACK_0[1] & 0xF0, 0x40); // ACK type

        assert_eq!(responses::COMPLETE_1[1] & 0xF0, 0x50); // Completion type
        assert_eq!(responses::COMPLETE_1[1] & 0x0F, 0x01); // Socket 1

        assert_eq!(responses::SYNTAX_ERROR[1], 0x60); // Error type
        assert_eq!(responses::SYNTAX_ERROR[2], 0x02); // Error code
    }
}
