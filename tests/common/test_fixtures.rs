//! Test fixtures for VISCA testing.
//!
//! Provides predefined command sets and generators for comprehensive testing.

use std::collections::HashMap;

/// VISCA command terminator byte.
const VISCA_TERMINATOR: u8 = 0xFF;

/// Command fixtures providing common VISCA commands for testing
pub struct CommandFixtures;

impl CommandFixtures {
    /// Get all power command fixtures
    pub fn power_commands() -> HashMap<&'static str, Vec<u8>> {
        let mut commands = HashMap::new();
        commands.insert(
            "power_on",
            vec![0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
        );
        commands.insert(
            "power_off",
            vec![0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR],
        );
        commands
    }

    /// Get all zoom command fixtures
    pub fn zoom_commands() -> HashMap<&'static str, Vec<u8>> {
        let mut commands = HashMap::new();
        commands.insert(
            "zoom_stop",
            vec![0x81, 0x01, 0x04, 0x07, 0x00, VISCA_TERMINATOR],
        );
        commands.insert(
            "zoom_tele_std",
            vec![0x81, 0x01, 0x04, 0x07, 0x02, VISCA_TERMINATOR],
        );
        commands.insert(
            "zoom_wide_std",
            vec![0x81, 0x01, 0x04, 0x07, 0x03, VISCA_TERMINATOR],
        );
        commands.insert(
            "zoom_tele_var_5",
            vec![0x81, 0x01, 0x04, 0x07, 0x25, VISCA_TERMINATOR],
        );
        commands.insert(
            "zoom_wide_var_5",
            vec![0x81, 0x01, 0x04, 0x07, 0x35, VISCA_TERMINATOR],
        );
        commands
    }

    /// Get all preset command fixtures
    pub fn preset_commands() -> HashMap<&'static str, Vec<u8>> {
        let mut commands = HashMap::new();
        commands.insert(
            "preset_set_1",
            vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x01, VISCA_TERMINATOR],
        );
        commands.insert(
            "preset_recall_1",
            vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, VISCA_TERMINATOR],
        );
        commands.insert(
            "preset_set_2",
            vec![0x81, 0x01, 0x04, 0x3F, 0x01, 0x02, VISCA_TERMINATOR],
        );
        commands.insert(
            "preset_recall_2",
            vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x02, VISCA_TERMINATOR],
        );
        commands
    }

    /// Get all focus command fixtures
    pub fn focus_commands() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            (
                "focus_auto",
                vec![0x81, 0x01, 0x04, 0x38, 0x02, VISCA_TERMINATOR],
            ),
            (
                "focus_manual",
                vec![0x81, 0x01, 0x04, 0x38, 0x03, VISCA_TERMINATOR],
            ),
            (
                "focus_stop",
                vec![0x81, 0x01, 0x04, 0x08, 0x00, VISCA_TERMINATOR],
            ),
            (
                "focus_far",
                vec![0x81, 0x01, 0x04, 0x08, 0x02, VISCA_TERMINATOR],
            ),
            (
                "focus_near",
                vec![0x81, 0x01, 0x04, 0x08, 0x03, VISCA_TERMINATOR],
            ),
        ]
    }

    /// Get edge case commands for testing error handling
    pub fn edge_case_commands() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("empty_command", vec![0x81, VISCA_TERMINATOR]), // Too short
            ("missing_terminator", vec![0x81, 0x01, 0x04, 0x00, 0x02]), // No FF
            (
                "invalid_header",
                vec![0x71, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR],
            ), // Wrong header
            (
                "maximum_length",
                vec![
                    0x81, 0x01, 0x06, 0x02, 0x18, 0x18, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F, 0x0F,
                    0x0F, 0xFF,
                ],
            ), // Max nibbles
        ]
    }
}

/// Test data generators
pub mod generators {
    /// Generate zoom position values for testing
    pub fn zoom_positions() -> Vec<u16> {
        vec![
            0x0000, // Minimum zoom (wide)
            0x4000, // Mid zoom
            0x7AC0, // Maximum zoom varies by camera
            0x1000, // Low zoom
            0x6000, // High zoom
        ]
    }

    /// Generate pan/tilt position pairs for testing
    pub fn pan_tilt_positions() -> Vec<(i16, i16)> {
        vec![
            (0, 0),        // Center/home
            (-2448, -432), // Top-left
            (2448, -432),  // Top-right
            (-2448, 1296), // Bottom-left
            (2448, 1296),  // Bottom-right
            (1000, 500),   // Arbitrary position
            (-1000, -500), // Arbitrary position
        ]
    }

    /// Generate speed values for testing
    pub fn speed_values() -> Vec<u8> {
        vec![
            0x01, // Minimum speed
            0x0A, // Low speed
            0x10, // Medium speed
            0x18, // High speed (typical max)
            0x1F, // Maximum speed (not all cameras support)
        ]
    }

    /// Generate preset IDs for testing
    pub fn preset_ids() -> Vec<u8> {
        vec![0, 1, 2, 3, 4, 5, 10, 15, 20, 50, 99, 127]
    }

    /// Alias for preset_ids for backward compatibility
    pub fn preset_numbers() -> Vec<u8> {
        preset_ids()
    }

    /// Generate exposure mode values
    pub fn exposure_modes() -> Vec<u8> {
        vec![
            0x00, // Auto
            0x03, // Manual
            0x0A, // Shutter priority
            0x0B, // Iris priority
            0x0D, // Bright mode
        ]
    }

    /// Generate white balance modes
    pub fn white_balance_modes() -> Vec<u8> {
        vec![
            0x00, // Auto
            0x01, // Indoor
            0x02, // Outdoor
            0x03, // One push
            0x04, // Auto tracing
            0x05, // Manual
        ]
    }

    /// Generate shutter speed indices
    pub fn shutter_speeds() -> Vec<u8> {
        vec![
            0x00, // 1/1
            0x05, // 1/6
            0x0A, // 1/25
            0x0F, // 1/60
            0x14, // 1/120
            0x19, // 1/250
            0x1E, // 1/500
            0x23, // 1/1000
            0x28, // 1/2000
            0x2D, // 1/4000
            0x32, // 1/10000
        ]
    }

    /// Generate gain values
    pub fn gain_values() -> Vec<u8> {
        vec![
            0x00, // 0dB
            0x01, // +2dB
            0x07, // +14dB
            0x0F, // +30dB (varies by camera)
        ]
    }
}
