//! Compile-time VISCA command constants.
//!
//! This module provides centralized constants for all VISCA protocol byte sequences.
//! Constants are organized hierarchically by:
//! - Command category (power, zoom, focus, etc.)
//! - Operation type (control commands vs inquiry commands)
//! - Vendor-specific vs standard VISCA commands

use crate::macros::support::{visca_bytes, visca_prefix};

/// Power command constants.
#[cfg(test)]
pub mod power {
    use super::*;

    /// Power on command.
    pub const ON: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x00, 0x02];

    /// Power off/standby command.
    pub const OFF: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x00, 0x03];
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

    /// Preset recall speed prefix (PTZOptics specific).
    /// Note: Shares the same 4-byte prefix as pan_tilt::MOVE_PREFIX but uses
    /// only a single speed byte (6-byte command) vs pan/tilt's multi-byte format.
    pub const RECALL_SPEED_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x06, 0x01];
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
    #[cfg(test)]
    pub const ZONE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xAA];

    /// Auto focus sensitivity prefix.
    #[cfg(test)]
    pub const AF_SENSITIVITY_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x58];

    /// Focus range/near limit prefix.
    #[cfg(test)]
    pub const NEAR_LIMIT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x28];

    /// Push AF control prefix (Sony FR7).
    pub const PUSH_AF_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00];
}

/// Exposure command constants.
pub mod exposure {
    use super::*;

    /// Exposure mode control prefix.
    #[cfg(test)]
    pub const MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x39];

    /// Anti-flicker mode prefix.
    #[cfg(test)]
    pub const ANTI_FLICKER_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x23];

    /// Spotlight prefix (Sony models).
    #[cfg(test)]
    pub const SPOTLIGHT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3A];

    /// Exposure compensation on/off prefix.
    pub const COMPENSATION_ON_OFF_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x3E];

    /// Exposure compensation control prefix (reset/up/down).
    pub const COMPENSATION_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x0E];

    /// Exposure compensation direct level prefix.
    pub const COMPENSATION_LEVEL_PREFIX: &[u8] =
        visca_prefix![0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00];

    /// Dynamic range control prefix.
    #[cfg(test)]
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

    /// Spot AE (auto exposure) control prefix.
    #[cfg(test)]
    pub const SPOT_AE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x5A];
}

/// Image flip command constants.
#[cfg(test)]
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
    #[cfg(test)]
    pub const BACKLIGHT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x33];

    /// Image flip combined mode prefix (PtzOptics specific).
    #[cfg(test)]
    pub const FLIP_COMBINED_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA4];

    /// Picture effect mode prefix.
    #[cfg(test)]
    pub const PICTURE_EFFECT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x63];

    /// 2D noise reduction prefix.
    #[cfg(test)]
    pub const NOISE_REDUCTION_2D_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x53];

    /// 3D noise reduction prefix.
    #[cfg(test)]
    pub const NOISE_REDUCTION_3D_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x54];

    /// Luminance/brightness adjustment prefix.
    #[cfg(test)]
    pub const LUMINANCE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA1, 0x00, 0x00, 0x00];

    /// Contrast adjustment prefix.
    #[cfg(test)]
    pub const CONTRAST_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA2, 0x00, 0x00, 0x00];

    /// Sharpness mode control prefix (auto/manual).
    pub const SHARPNESS_MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x05];

    /// Sharpness control prefix (reset/up/down).
    pub const SHARPNESS_CONTROL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x02];

    /// Sharpness direct level prefix.
    pub const SHARPNESS_LEVEL_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x42, 0x00, 0x00];

    /// Gamma curve selection prefix.
    ///
    /// VISCA command `81 01 04 5B 0p FF` where p selects the gamma curve
    /// (0=Standard, 1-4=different gamma curves).
    #[cfg(test)]
    pub const GAMMA_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x5B];
}

/// System command constants.
pub mod system {
    // Constants moved to macro-based implementations in system.rs:
    // - ADDRESS_SET → AddressSetCommand using visca_const_command!
    // - INTERFACE_CLEAR → InterfaceClearCommand using visca_const_command!
    // - COMMAND_CANCEL_PREFIX → CommandCancelCommand with direct encoding
}

/// White balance command constants.
#[cfg(test)]
pub mod white_balance {
    use super::*;

    /// White balance mode control prefix.
    pub const MODE_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x35];

    /// Auto white balance sensitivity prefix.
    pub const AWB_SENSITIVITY_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0xA9];

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
    #[cfg(test)]
    pub const SATURATION_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x49, 0x00, 0x00, 0x00];

    /// Color hue prefix.
    #[cfg(test)]
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
    #[cfg(test)]
    pub const GAIN_LIMIT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x04, 0x2C];
}

/// Tally command constants.
#[cfg(test)]
pub mod tally {
    use super::*;

    /// Tally control prefix.
    pub const TALLY_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00];

    /// Tally brightness prefix (Sony BRC models).
    pub const TALLY_BRIGHT_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x01, 0x0A, 0x01];

    /// Green tally prefix (Sony FR7).
    pub const TALLY_GREEN_PREFIX: &[u8] = visca_prefix![0x81, 0x01, 0x7E, 0x04, 0x1A, 0x00];

    /// PtzOptics tally prefix.
    pub const TALLY_PTZO_PREFIX: &[u8] = visca_prefix![0x81, 0x0A, 0x02, 0x02];

    /// Tally inquiry prefix.
    pub const TALLY_INQUIRY_PREFIX: &[u8] = visca_prefix![0x81, 0x09, 0x7E, 0x01, 0x0A, 0x00];
}

/// Inquiry command constants generated from the built-in inquiry table.
pub mod inquiry {
    #[allow(unused_imports)]
    pub use crate::command::inquiry_structs::bytes::*;
}

/// Menu command constants.
#[cfg(test)]
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
    /// **Vendor-Specific**: PtzOptics streaming commands.
    /// Used by: MulticastStreamingInternal in streaming.rs
    #[cfg(test)]
    pub const MULTICAST_PREFIX: &[u8] = visca_prefix![0x81, 0x0B, 0x01, 0x23];

    /// Ndi quality control prefix.
    /// **Vendor-Specific**: PtzOptics Ndi streaming commands.
    /// Used by: NdiQualityCommandInternal in streaming.rs
    #[cfg(test)]
    pub const NDI_QUALITY_PREFIX: &[u8] = visca_prefix![0x81, 0x0B, 0x01, 0x01];

    /// USB audio control prefix.
    /// **Vendor-Specific**: PtzOptics USB audio toggle.
    pub const USB_AUDIO_PREFIX: &[u8] = visca_prefix![0x81, 0x2A, 0x02, 0xA0, 0x04];
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

/// Motion sync command constants (PtzOptics specific).
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
#[cfg(test)]
pub mod system_cmd {
    use super::*;

    /// Command cancel prefix (socket number follows).
    pub const CANCEL_PREFIX: &[u8] = visca_prefix![0x81];
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod validation_tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::command::inquiry_structs::{BuiltinInquiryQuery, BUILTIN_INQUIRIES};

    /// This test validates that all constants are correctly formed at compile time.
    /// The visca_bytes! and visca_prefix! macros already perform compile-time validation,
    /// but this test ensures that:
    /// 1. All constants compile without errors
    /// 2. Constants have the expected format (proper camera ID, terminator, etc.)
    #[test]
    fn test_constants_compile_time_validation() {
        // Power constants (using visca_prefix! so no terminator)
        assert_eq!(power::ON[0], 0x81); // Camera ID
        assert_ne!(power::ON[power::ON.len() - 1], 0xFF); // No Terminator
        assert_eq!(power::OFF[0], 0x81);
        assert_ne!(power::OFF[power::OFF.len() - 1], 0xFF); // No Terminator

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
        // All inquiry commands should:
        // 1. Start with 0x81 (camera ID)
        // 2. Use the baseline 0x09 inquiry family unless explicitly
        //    classified as a vendor-specific extension
        // 3. End with 0xFF (terminator)
        // 4. Duplicate bytes must be explicitly classified in metadata.

        let mut seen = Vec::new();

        for meta in BUILTIN_INQUIRIES {
            let Some(inq) = meta.bytes else {
                continue;
            };
            // Check format
            assert_eq!(inq[0], 0x81, "Inquiry {} should start with 0x81", meta.name);
            if inq[1] != 0x09 {
                assert!(
                    meta.vendor_specific,
                    "Baseline inquiry {} should have 0x09 as second byte",
                    meta.name
                );
            }
            assert_eq!(
                inq[inq.len() - 1],
                0xFF,
                "Inquiry {} should end with 0xFF",
                meta.name
            );

            for (prev_name, prev_query, prev_bytes) in &seen {
                if *prev_bytes == inq {
                    let prev_explicit = matches!(
                        prev_query,
                        BuiltinInquiryQuery::Alias { .. }
                            | BuiltinInquiryQuery::AlternateTypedInterpretation { .. }
                            | BuiltinInquiryQuery::Unavailable
                    );
                    let current_explicit = matches!(
                        meta.query,
                        BuiltinInquiryQuery::Alias { .. }
                            | BuiltinInquiryQuery::AlternateTypedInterpretation { .. }
                            | BuiltinInquiryQuery::Unavailable
                    );
                    assert!(
                        prev_explicit || current_explicit,
                        "Duplicate inquiry bytes for {prev_name} and {} must be explicit",
                        meta.name
                    );
                }
            }
            seen.push((meta.name, meta.query, inq));
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
            exposure::ANTI_FLICKER_PREFIX,
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
            exposure::SPOT_AE_PREFIX,
            // Flip and image-processing prefixes
            flip::PREFIX,
            flip::HFLIP_PREFIX,
            flip::FREEZE_PREFIX,
            image::BACKLIGHT_PREFIX,
            image::FLIP_COMBINED_PREFIX,
            image::PICTURE_EFFECT_PREFIX,
            image::NOISE_REDUCTION_2D_PREFIX,
            image::NOISE_REDUCTION_3D_PREFIX,
            image::LUMINANCE_PREFIX,
            image::CONTRAST_PREFIX,
            image::GAMMA_PREFIX,
            // White balance prefixes
            white_balance::MODE_PREFIX,
            white_balance::AWB_SENSITIVITY_PREFIX,
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
            tally::TALLY_PREFIX,
            tally::TALLY_BRIGHT_PREFIX,
            tally::TALLY_GREEN_PREFIX,
            tally::TALLY_INQUIRY_PREFIX,
            menu::TOGGLE_PREFIX,
            menu::NAVIGATE_PREFIX,
            menu::SETTINGS_PREFIX,
            streaming::NDI_QUALITY_PREFIX,
            system_cmd::CANCEL_PREFIX,
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
        use crate::command::encode::ViscaCommand;

        // Test gain commands use constants
        let mut buffer = [0u8; 32];

        // Test Gain::Reset uses CONTROL_PREFIX
        let gain_reset = crate::command::gain::Gain::Reset;
        let len = gain_reset
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[0..4], gain::CONTROL_PREFIX);
        assert_eq!(buffer[4], 0x00); // Reset control byte
        assert_eq!(buffer[5], 0xFF); // Terminator
        assert_eq!(len, 6);

        // Test Gain::SetValue uses DIRECT_PREFIX
        let gain_value =
            crate::command::gain::Gain::SetValue(crate::types::GainLevel::new(0x05).unwrap());
        let _len = gain_value
            .write_into(CameraId::CAMERA_1, &mut buffer)
            .unwrap();
        assert_eq!(&buffer[0..6], gain::DIRECT_PREFIX);

        // Test GainLimit uses GAIN_LIMIT_PREFIX
        let gain_limit = crate::command::gain::GainLimitCommand::new(
            crate::types::GainLimit::new(0x03).unwrap(),
        );
        let len = gain_limit
            .write_into(CameraId::CAMERA_1, &mut buffer)
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

        // Power commands (using visca_prefix! so no terminator)
        assert_eq!(power::ON, &[0x81, 0x01, 0x04, 0x00, 0x02]);
        assert_eq!(power::OFF, &[0x81, 0x01, 0x04, 0x00, 0x03]);

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
