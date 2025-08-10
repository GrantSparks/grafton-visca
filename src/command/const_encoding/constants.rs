//! Compile-time VISCA command constants.
//!
//! This module provides centralized constants for all VISCA protocol byte sequences.
//! Constants are organized hierarchically by:
//! - Command category (power, zoom, focus, etc.)
//! - Operation type (control commands vs inquiry commands)
//! - Vendor-specific vs standard VISCA commands

use crate::macros::internal::{visca_bytes, visca_prefix};

/// Power command constants.
pub mod power {
    use super::*;

    /// Power on command.
    pub const ON: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x00, 0x02];

    /// Power off/standby command.
    pub const OFF: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x00, 0x03];
}

/// Pan/Tilt command constants.
pub mod pan_tilt {
    use super::*;

    /// Home position command.
    pub const HOME: &[u8] = visca_bytes![0x81, 0x01, 0x06, 0x04];

    /// Reset pan/tilt.
    pub const RESET: &[u8] = visca_bytes![0x81, 0x01, 0x06, 0x05];

    /// Pan/tilt directional movement prefix.
    pub const MOVE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x01];

    /// Absolute position prefix.
    pub const ABSOLUTE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x02];

    /// Relative position prefix.
    pub const RELATIVE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x03];

    /// Limit set command prefix.
    pub const LIMIT_SET_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x07, 0x00];

    /// Limit clear command prefix.
    pub const LIMIT_CLEAR_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x07, 0x01];
}

/// Zoom command constants.
pub mod zoom {
    use super::*;

    /// Stop zoom.
    pub const STOP: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x07, 0x00];

    /// Zoom in (telephoto) standard speed.
    pub const TELE_STD: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x07, 0x02];

    /// Zoom out (wide) standard speed.
    pub const WIDE_STD: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x07, 0x03];

    /// Variable speed zoom prefix (for both tele and wide).
    pub const VARIABLE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x07];

    /// Direct zoom position prefix.
    pub const POSITION_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x47];

    /// Digital zoom control prefix.
    pub const DIGITAL_ZOOM_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x06];
}

/// Preset command constants.
pub mod preset {
    use super::*;

    /// Preset control prefix (reset/set/recall).
    pub const CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3F];
}

/// Focus command constants.
pub mod focus {
    use super::*;

    /// Focus movement control prefix (stop/far/near).
    pub const MOVEMENT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x08];

    /// Focus direct position control prefix.
    pub const POSITION_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x48];

    /// Focus mode control prefix (auto/manual).
    pub const MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x38];

    /// One-push focus trigger control prefix.
    pub const ONE_PUSH_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x18];

    /// Focus lock control prefix.
    pub const LOCK_PREFIX: &[u8] = visca_prefix![0x81, 0x0A, 0x04, 0x68];

    /// Focus zone control prefix.
    pub const ZONE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xAA];

    /// Auto focus sensitivity prefix.
    pub const AF_SENSITIVITY_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x58];

    /// Focus range/near limit prefix.
    pub const NEAR_LIMIT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x28];

    /// Push AF control prefix (Sony FR7).
    pub const PUSH_AF_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00];
}

/// Exposure command constants.
pub mod exposure {
    use super::*;

    /// Exposure mode control prefix.
    pub const MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x39];

    /// Spotlight prefix (Sony models).
    pub const SPOTLIGHT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3A];

    /// Exposure compensation on/off prefix.
    pub const COMPENSATION_ON_OFF_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3E];

    /// Exposure compensation control prefix (reset/up/down).
    pub const COMPENSATION_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x0E];

    /// Exposure compensation direct level prefix.
    pub const COMPENSATION_LEVEL_PREFIX: &[u8] =
        visca_prefix![0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00];

    /// Dynamic range control prefix.
    pub const DYNAMIC_RANGE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00];

    /// Iris control prefix (reset/up/down).
    pub const IRIS_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x0B];

    /// Iris direct value prefix.
    pub const IRIS_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4B, 0x00, 0x00];

    /// Shutter control prefix (reset/up/down).
    pub const SHUTTER_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x0A];

    /// Shutter direct value prefix.
    pub const SHUTTER_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4A, 0x00, 0x00];

    /// Brightness control prefix (reset/up/down).
    pub const BRIGHTNESS_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x0D];

    /// Brightness direct value prefix.
    pub const BRIGHTNESS_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4D, 0x00, 0x00];

    /// Brightness value prefix (alternative).
    pub const BRIGHTNESS_VALUE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x0D, 0x00, 0x00];

    /// Spot AE (auto exposure) control prefix.
    pub const SPOT_AE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x5A];
}

/// Image flip command constants.
pub mod flip {
    use super::*;

    /// Image flip prefix.
    pub const PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x66];

    /// Horizontal flip prefix.
    pub const HFLIP_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x61];

    /// Image freeze prefix.
    pub const FREEZE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x62];
}

/// Image adjustment command constants.
pub mod image {
    use super::*;

    /// Backlight compensation prefix.
    pub const BACKLIGHT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x33];

    /// Black and white mode prefix.
    pub const BLACK_WHITE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x01];

    /// Image flip combined mode prefix.
    pub const FLIP_COMBINED_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x61];

    /// Picture effect mode prefix.
    pub const PICTURE_EFFECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x63];

    /// 2D noise reduction prefix.
    pub const NOISE_REDUCTION_2D_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x53];

    /// 3D noise reduction prefix.
    pub const NOISE_REDUCTION_3D_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x54];

    /// Luminance/brightness adjustment prefix.
    pub const LUMINANCE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00];

    /// Contrast adjustment prefix.
    pub const CONTRAST_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00];

    /// Sharpness mode control prefix (auto/manual).
    pub const SHARPNESS_MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x05];

    /// Sharpness control prefix (reset/up/down).
    pub const SHARPNESS_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x02];

    /// Sharpness direct level prefix.
    pub const SHARPNESS_LEVEL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x42, 0x00, 0x00];
}

/// System command constants.
pub mod system {
    // Constants moved to macro-based implementations in system.rs:
    // - ADDRESS_SET → AddressSetCommand using visca_const_command!
    // - INTERFACE_CLEAR → InterfaceClearCommand using visca_const_command!
    // - COMMAND_CANCEL_PREFIX → CommandCancelCommand with direct encoding
}

/// White balance command constants.
pub mod white_balance {
    use super::*;

    /// White balance mode control prefix.
    pub const MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x35];

    /// Auto white balance sensitivity prefix.
    pub const AWB_SENSITIVITY_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA9];

    /// One push white balance trigger.
    /// One push white balance trigger command.
    /// Note: Currently unused in production code because visca_const_command! macro doesn't support constant references.
    /// The command is defined with hardcoded bytes in color.rs.
    #[cfg(test)]
    pub const ONE_PUSH_TRIGGER: &[u8] = visca_bytes![0x81, 0x01, 0x04, 0x10, 0x05];
}

/// Color adjustment command constants.
pub mod color {
    use super::*;

    /// Color saturation prefix.
    pub const SATURATION_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00];

    /// Color hue prefix.
    pub const HUE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4F, 0x00, 0x00, 0x00];

    /// Red gain direct prefix (for WB fine-tuning).
    pub const RED_GAIN_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x43, 0x00, 0x00];

    /// Blue gain direct prefix (for WB fine-tuning).
    pub const BLUE_GAIN_DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x44, 0x00, 0x00];

    /// Red gain control prefix (reset/up/down).
    pub const RED_GAIN_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x03];

    /// Blue gain control prefix (reset/up/down).
    pub const BLUE_GAIN_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x04];

    /// Color temperature control prefix (reset/up/down/direct).
    pub const TEMPERATURE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x20];
}

/// Gain command constants.
pub mod gain {
    use super::*;

    /// Gain control prefix (reset/up/down).
    pub const CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x0C];

    /// Gain direct value prefix.
    pub const DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x4C, 0x00, 0x00];

    /// Gain limit prefix.
    pub const GAIN_LIMIT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x2C];
}

/// Tally command constants.
pub mod tally {
    use super::*;

    /// Tally control prefix.
    pub const TALLY_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00];

    /// Tally brightness prefix (Sony BRC models).
    pub const TALLY_BRIGHT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x01];

    /// Green tally prefix (Sony FR7).
    pub const TALLY_GREEN_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x1A, 0x00];

    /// PTZOptics tally prefix.
    pub const TALLY_PTZO_PREFIX: &[u8] = visca_prefix![0x81, 0x0A, 0x02, 0x02];

    /// Tally inquiry prefix.
    pub const TALLY_INQUIRY_PREFIX: &[u8] = visca_prefix![0x81, 0x09, 0x7E, 0x01, 0x0A, 0x00];
}

/// Inquiry command constants.
///
/// These are complete inquiry commands that request status information from the camera.
/// All inquiry commands use 0x09 as the command type byte (after camera ID).
#[cfg_attr(not(test), allow(dead_code))]
pub mod inquiry {
    use super::*;

    // Core status inquiries
    /// Power status inquiry (on/off/standby).
    pub const POWER: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x00];

    /// Zoom position inquiry.
    pub const ZOOM_POSITION: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x47];

    /// Focus position inquiry.
    pub const FOCUS_POSITION: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x48];

    /// Focus mode inquiry (auto/manual).
    pub const FOCUS_MODE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x38];

    /// Auto focus on/off inquiry.
    pub const AUTO_FOCUS: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x18];

    // Menu and UI inquiries
    /// Menu open/close status inquiry.
    pub const MENU_OPEN_CLOSE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x06];

    // Tally inquiries
    /// Tally light status inquiry.
    pub const TALLY_STATUS: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0xA8];

    /// Green tally status inquiry (Sony FR7).
    pub const TALLY_GREEN: &[u8] = visca_bytes![0x81, 0x09, 0x7E, 0x04, 0x1A, 0x00];

    // Image and video settings inquiries
    /// Resolution inquiry.
    pub const RESOLUTION: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x63];

    /// Night/Day mode inquiry.
    pub const NIGHT_DAY_MODE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x60];

    /// ND filter status inquiry.
    pub const ND_FILTER: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x64];

    /// Picture effect mode inquiry.
    pub const PICTURE_EFFECT: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x32];

    /// Flip mode inquiry.
    pub const FLIP_MODE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x65];

    /// Standby mode inquiry.
    pub const STANDBY: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x70];

    // Focus and iris inquiries
    /// Focus range inquiry.
    pub const FOCUS_RANGE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x2A];

    /// Iris control mode inquiry.
    pub const IRIS_CONTROL: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x2B];

    // Image enhancement inquiries
    /// Defog mode inquiry.
    pub const DEFOG_MODE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x37];

    /// Defog level inquiry.
    pub const DEFOG_LEVEL: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0xA0];

    /// Digital PTZ status inquiry.
    pub const DIGITAL_PTZ: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x6B];

    // White balance and color inquiries
    /// Auto white balance sensitivity inquiry.
    pub const AUTO_WB_SENSITIVITY: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x59];

    /// Exposure compensation position inquiry.
    pub const EXPOSURE_COMPENSATION_POSITION: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x4E];

    /// Red tuning inquiry (white balance).
    pub const RED_TUNING: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x43];

    /// Blue tuning inquiry (white balance).
    pub const BLUE_TUNING: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x44];

    // Image quality inquiries
    /// Sharpness position inquiry.
    pub const SHARPNESS_POSITION: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x42];

    /// Noise reduction level inquiry.
    pub const NR_LEVEL: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x52];

    /// Noise reduction mode inquiry.
    pub const NR_MODE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x53];

    /// Noise reduction speed inquiry.
    pub const NR_SPEED: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x54];

    /// Black and white mode inquiry.
    pub const BLACK_WHITE_MODE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x73];

    // System and network inquiries
    /// Broadcast domain inquiry.
    pub const BROADCAST_DOMAIN: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x75];

    // Advanced feature inquiries
    /// Motion sync mode inquiry.
    pub const MOTION_SYNC_MODE: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x56];

    /// Motion sync speed inquiry.
    pub const MOTION_SYNC_SPEED: &[u8] = visca_bytes![0x81, 0x09, 0x04, 0x57];

    /// Auto trace status inquiry.
    pub const AUTO_TRACE: &[u8] = visca_bytes![0x81, 0x09, 0x50, 0x09];

    /// Focus unlock status inquiry.
    pub const FOCUS_UNLOCK: &[u8] = visca_bytes![0x81, 0x09, 0x54, 0x08];
}

/// Menu command constants.
pub mod menu {
    use super::*;

    /// Menu toggle command prefix.
    pub const TOGGLE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x06];

    /// Menu navigation command prefix.
    pub const NAVIGATE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x01, 0x0E, 0x0E];

    /// Menu settings control prefix.
    pub const SETTINGS_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x72];
}

/// Streaming command constants.
///
/// Note: These constants document the byte sequences used by streaming commands.
/// They cannot be directly used in macro-based commands due to macro limitations
/// requiring literal arrays, but serve as documentation and validation references.
pub mod streaming {
    use super::*;

    /// Multicast streaming control prefix.
    /// **Vendor-Specific**: PTZOptics streaming commands.
    /// Used by: MulticastStreamingInternal in streaming.rs
    pub const MULTICAST_PREFIX: &[u8] = visca_prefix![0x81, 0x0B, 0x01, 0x23];

    /// NDI quality control prefix.
    /// **Vendor-Specific**: PTZOptics NDI streaming commands.
    /// Used by: NDIQualityCommandInternal in streaming.rs
    pub const NDI_QUALITY_PREFIX: &[u8] = visca_prefix![0x81, 0x0B, 0x01, 0x01];
}

/// ND filter command constants.
pub mod nd_filter {
    use super::*;

    /// ND filter control prefix.
    pub const CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x52];

    /// ND filter level control prefix.
    pub const LEVEL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x53];

    /// ND filter mode prefix.
    pub const MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x12];

    /// ND filter direct value prefix.
    pub const DIRECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x42, 0x00];
}

/// Motion sync command constants (PTZOptics specific).
pub mod motion_sync {
    use super::*;

    /// Motion sync mode control prefix.
    pub const MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x0A, 0x11, 0x13];

    /// Motion sync speed control prefix.
    pub const SPEED_PREFIX: &[u8] = visca_prefix![0x81, 0x0A, 0x11, 0x14];
}

/// Variable speed command constants.
pub mod variable_speed {
    use super::*;

    /// Variable speed control prefix.
    pub const CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x1B];
}

/// System command-related constants.
pub mod system_cmd {
    use super::*;

    /// Command cancel prefix (socket number follows).
    pub const CANCEL_PREFIX: &[u8] = visca_prefix![0x81];
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod validation_tests {
    use super::*;
    use crate::command::const_encoding::VISCA_TERMINATOR;

    /// This test validates that all constants are correctly formed at compile time.
    /// The visca_bytes! and visca_prefix! macros already perform compile-time validation,
    /// but this test ensures that:
    /// 1. All constants compile without errors
    /// 2. Constants have the expected format (proper camera ID, terminator, etc.)
    #[test]
    fn test_constants_compile_time_validation() {
        // Power constants
        assert_eq!(power::ON[0], 0x81); // Camera ID
        assert_eq!(power::ON[power::ON.len() - 1], 0xFF); // Terminator
        assert_eq!(power::OFF[0], 0x81);
        assert_eq!(power::OFF[power::OFF.len() - 1], 0xFF);

        // Pan/Tilt constants
        assert_eq!(pan_tilt::HOME[0], 0x81);
        assert_eq!(pan_tilt::HOME[pan_tilt::HOME.len() - 1], 0xFF);
        assert_eq!(pan_tilt::RESET[0], 0x81);
        assert_eq!(pan_tilt::RESET[pan_tilt::RESET.len() - 1], 0xFF);

        // Zoom constants
        assert_eq!(zoom::STOP[0], 0x81);
        assert_eq!(zoom::STOP[zoom::STOP.len() - 1], 0xFF);
        assert_eq!(zoom::TELE_STD[0], 0x81);
        assert_eq!(zoom::TELE_STD[zoom::TELE_STD.len() - 1], 0xFF);
        assert_eq!(zoom::WIDE_STD[0], 0x81);
        assert_eq!(zoom::WIDE_STD[zoom::WIDE_STD.len() - 1], 0xFF);

        // Prefix validation - should not have terminators
        assert_eq!(zoom::DIGITAL_ZOOM_PREFIX[0], 0x81);
        assert_ne!(
            zoom::DIGITAL_ZOOM_PREFIX[zoom::DIGITAL_ZOOM_PREFIX.len() - 1],
            0xFF
        );

        // Focus prefixes
        assert_eq!(focus::MOVEMENT_PREFIX[0], 0x81);
        assert_ne!(
            focus::MOVEMENT_PREFIX[focus::MOVEMENT_PREFIX.len() - 1],
            0xFF
        );

        // Inquiry commands - all should have 0x09 as second byte
        assert_eq!(inquiry::POWER[0], 0x81);
        assert_eq!(inquiry::POWER[1], 0x09);
        assert_eq!(inquiry::POWER[inquiry::POWER.len() - 1], 0xFF);

        assert_eq!(inquiry::ZOOM_POSITION[0], 0x81);
        assert_eq!(inquiry::ZOOM_POSITION[1], 0x09);
        assert_eq!(
            inquiry::ZOOM_POSITION[inquiry::ZOOM_POSITION.len() - 1],
            0xFF
        );

        // Vendor-specific constants
        assert_eq!(streaming::MULTICAST_PREFIX[0], 0x81);
        assert_eq!(tally::TALLY_PTZO_PREFIX[0], 0x81);
        assert_eq!(tally::TALLY_PTZO_PREFIX[1], 0x0A); // Different command type
    }

    /// Test that inquiry constants follow the proper VISCA inquiry format
    #[test]
    fn test_inquiry_format_validation() {
        use std::collections::HashSet;

        // All inquiry commands should:
        // 1. Start with 0x81 (camera ID)
        // 2. Have 0x09 as second byte (inquiry command type)
        // 3. End with 0xFF (terminator)
        // 4. Be unique (no duplicates)

        let inquiries = vec![
            inquiry::POWER,
            inquiry::ZOOM_POSITION,
            inquiry::FOCUS_POSITION,
            inquiry::FOCUS_MODE,
            inquiry::AUTO_FOCUS,
            inquiry::MENU_OPEN_CLOSE,
            inquiry::TALLY_STATUS,
            inquiry::TALLY_GREEN,
            inquiry::RESOLUTION,
            inquiry::NIGHT_DAY_MODE,
            inquiry::ND_FILTER,
            inquiry::PICTURE_EFFECT,
            inquiry::FLIP_MODE,
            inquiry::STANDBY,
            inquiry::FOCUS_RANGE,
            inquiry::IRIS_CONTROL,
            inquiry::DEFOG_MODE,
            inquiry::DEFOG_LEVEL,
            inquiry::DIGITAL_PTZ,
            inquiry::AUTO_WB_SENSITIVITY,
            inquiry::EXPOSURE_COMPENSATION_POSITION,
            inquiry::RED_TUNING,
            inquiry::BLUE_TUNING,
            inquiry::SHARPNESS_POSITION,
            inquiry::NR_LEVEL,
            inquiry::NR_MODE,
            inquiry::NR_SPEED,
            inquiry::BLACK_WHITE_MODE,
            inquiry::BROADCAST_DOMAIN,
            inquiry::MOTION_SYNC_MODE,
            inquiry::MOTION_SYNC_SPEED,
            inquiry::AUTO_TRACE,
            inquiry::FOCUS_UNLOCK,
        ];

        let mut seen = HashSet::new();

        for (idx, &inq) in inquiries.iter().enumerate() {
            // Check format
            assert_eq!(inq[0], 0x81, "Inquiry {idx} should start with 0x81");
            assert_eq!(
                inq[1], 0x09,
                "Inquiry {idx} should have 0x09 as second byte"
            );
            assert_eq!(
                inq[inq.len() - 1],
                0xFF,
                "Inquiry {idx} should end with 0xFF"
            );

            // Check uniqueness
            let key = Vec::from(inq);
            assert!(
                seen.insert(key),
                "Duplicate inquiry constant found at index {idx}"
            );
        }
    }

    /// Test that prefix constants don't have terminators
    #[test]
    fn test_prefix_format_validation() {
        let prefixes = vec![
            // Focus prefixes
            focus::MOVEMENT_PREFIX,
            focus::POSITION_PREFIX,
            focus::MODE_PREFIX,
            focus::ONE_PUSH_PREFIX,
            focus::LOCK_PREFIX,
            focus::ZONE_PREFIX,
            focus::AF_SENSITIVITY_PREFIX,
            focus::NEAR_LIMIT_PREFIX,
            // Exposure prefixes
            exposure::MODE_PREFIX,
            exposure::SPOTLIGHT_PREFIX,
            exposure::COMPENSATION_ON_OFF_PREFIX,
            exposure::COMPENSATION_CONTROL_PREFIX,
            exposure::COMPENSATION_LEVEL_PREFIX,
            exposure::DYNAMIC_RANGE_PREFIX,
            exposure::IRIS_CONTROL_PREFIX,
            exposure::IRIS_DIRECT_PREFIX,
            exposure::SHUTTER_CONTROL_PREFIX,
            exposure::SHUTTER_DIRECT_PREFIX,
            exposure::BRIGHTNESS_CONTROL_PREFIX,
            exposure::BRIGHTNESS_DIRECT_PREFIX,
            exposure::BRIGHTNESS_VALUE_PREFIX,
            exposure::SPOT_AE_PREFIX,
            // Other prefixes
            gain::CONTROL_PREFIX,
            gain::DIRECT_PREFIX,
            gain::GAIN_LIMIT_PREFIX,
            color::SATURATION_PREFIX,
            color::HUE_PREFIX,
            color::RED_GAIN_DIRECT_PREFIX,
            color::BLUE_GAIN_DIRECT_PREFIX,
            color::RED_GAIN_CONTROL_PREFIX,
            color::BLUE_GAIN_CONTROL_PREFIX,
            color::TEMPERATURE_PREFIX,
        ];

        for (idx, &prefix) in prefixes.iter().enumerate() {
            assert_ne!(
                prefix[prefix.len() - 1],
                0xFF,
                "Prefix {idx} should not end with terminator 0xFF"
            );
        }
    }

    /// Test that constants are actually being used by commands
    #[test]
    fn test_constant_usage_in_commands() {
        use crate::camera_id::CameraId;
        use crate::command::encode_visca::EncodeVisca;

        // Test gain commands use constants
        let mut buffer = [0u8; 32];

        // Test Gain::Reset uses CONTROL_PREFIX
        let gain_reset = crate::command::gain::Gain::Reset;
        let len = gain_reset
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[0..4], gain::CONTROL_PREFIX);
        assert_eq!(buffer[4], 0x00); // Reset control byte
        assert_eq!(buffer[5], 0xFF); // Terminator
        assert_eq!(len, 6);

        // Test Gain::SetValue uses DIRECT_PREFIX
        let gain_value =
            crate::command::gain::Gain::SetValue(crate::types::GainLevel::new(0x05).unwrap());
        let _len = gain_value
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[0..6], gain::DIRECT_PREFIX);

        // Test GainLimitCommand uses GAIN_LIMIT_PREFIX
        let gain_limit = crate::command::gain::GainLimitCommand::new(
            crate::types::GainLimit::new(0x03).unwrap(),
        );
        let len = gain_limit
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[0..4], gain::GAIN_LIMIT_PREFIX);
        assert_eq!(buffer[4], 0x03); // Limit value
        assert_eq!(buffer[5], 0xFF); // Terminator
        assert_eq!(len, 6);
    }

    /// Test that the visca_test! macro test cases match our constants
    #[test]
    fn test_visca_test_macro_consistency() {
        // The visca_test! macro in tests should use the same byte sequences
        // as our constants. This test ensures they stay in sync.

        // Power commands
        assert_eq!(power::ON, &[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]);
        assert_eq!(
            power::OFF,
            &[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]
        );

        // Pan/Tilt commands
        assert_eq!(pan_tilt::HOME, &[0x81, 0x01, 0x06, 0x04, VISCA_TERMINATOR]);
        assert_eq!(pan_tilt::RESET, &[0x81, 0x01, 0x06, 0x05, VISCA_TERMINATOR]);

        // Zoom commands
        assert_eq!(
            zoom::STOP,
            &[0x81, 0x01, 0x04, 0x07, 0x00, VISCA_TERMINATOR]
        );
        assert_eq!(
            zoom::TELE_STD,
            &[0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR]
        );
        assert_eq!(
            zoom::WIDE_STD,
            &[0x81, 0x01, 0x04, 0x07, 0x03, VISCA_TERMINATOR]
        );

        // White balance one-push trigger (note: visca_bytes! adds terminator)
        assert_eq!(
            white_balance::ONE_PUSH_TRIGGER,
            &[0x81, 0x01, 0x04, 0x10, 0x05, VISCA_TERMINATOR]
        );
    }
}
