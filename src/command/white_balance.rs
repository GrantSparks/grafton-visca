//! White balance control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera white balance settings,
//! including auto, manual, and preset modes like indoor/outdoor/one-push/color temperature.
//!
//! # VISCA Compliance
//! Most white balance modes (Auto, Indoor, Outdoor, OnePush, Manual) are baseline VISCA.
//!
//! ## Vendor-Specific Features
//! - `ATW` (Auto Tracking White Balance) - Sony FR7 specific
//! - `AWBSensitivity` - PtzOptics specific

use grafton_visca_macros::ViscaEnum;

use crate::visca_command;

/// White balance modes.
///
/// Controls how the camera adjusts color temperature to ensure
/// white objects appear white under different lighting conditions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum WhiteBalanceMode {
    /// Automatic white balance adjustment.
    Auto = 0x00,
    /// Indoor preset (optimized for incandescent/tungsten lighting).
    Indoor = 0x01,
    /// Outdoor preset (optimized for daylight).
    Outdoor = 0x02,
    /// One-push white balance (calibrate once based on current scene).
    OnePush = 0x03,
    /// Auto tracking white balance.
    ///
    /// **Vendor-Specific**: This mode is specific to Sony FR7 cameras.
    #[cfg_attr(feature = "serde", serde(rename = "atw"))]
    ATW = 0x04,
    /// Manual white balance control.
    Manual = 0x05,
    /// Color temperature mode (specify exact color temperature).
    ColorTemperature = 0x20,
}

/// Auto white balance sensitivity levels.
///
/// Controls how aggressively the automatic white balance algorithm
/// adjusts to changing lighting conditions.
///
/// Per the VISCA specification (command 0x81 0x01 0x04 0xA9):
/// - High = 0x00
/// - Normal = 0x01
/// - Low = 0x02
///
/// Both commands and inquiry responses use the same byte values.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum AutoWhiteBalanceSensitivity {
    /// High sensitivity - faster adjustments to changing conditions.
    High = 0x00,
    /// Normal sensitivity - balanced adjustment speed.
    Normal = 0x01,
    /// Low sensitivity - slower, more stable adjustments.
    Low = 0x02,
}

impl AutoWhiteBalanceSensitivity {
    /// Convert to command byte value.
    ///
    /// Returns the VISCA byte representation: High=0x00, Normal=0x01, Low=0x02.
    pub fn to_command_byte(self) -> u8 {
        self as u8
    }
}

visca_command! {
        /// Command to set the white balance mode.
    pub struct WhiteBalanceCommand { mode: WhiteBalanceMode };
    prefix = [0x01, 0x04, 0x35];
    param = *mode as u8;
    max_param_size = 1;
    category = crate::timeout::CommandCategory::Quick;
}

visca_command! {
        /// Command to set AWB sensitivity.
    pub struct AWBSensitivityCommand { sensitivity: AutoWhiteBalanceSensitivity };
    prefix = [0x01, 0x04, 0xA9];
    param = sensitivity.to_command_byte();
    max_param_size = 1;
    category = crate::timeout::CommandCategory::Quick;
}

impl WhiteBalanceCommand {
    /// Create a new white balance command.
    pub fn new(mode: WhiteBalanceMode) -> Self {
        Self { mode }
    }
}

impl AWBSensitivityCommand {
    /// Create a new AWB sensitivity command.
    pub fn new(sensitivity: AutoWhiteBalanceSensitivity) -> Self {
        Self { sensitivity }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    use crate::{
        command::{bytes::VISCA_TERMINATOR, encode::ViscaCommand},
        macros::test_utils::visca_test,
        timeout::{CommandCategory, CommandTimeout},
    };

    #[test]
    fn test_white_balance_mode_values() {
        assert_eq!(WhiteBalanceMode::Auto as u8, 0x00);
        assert_eq!(WhiteBalanceMode::Indoor as u8, 0x01);
        assert_eq!(WhiteBalanceMode::Outdoor as u8, 0x02);
        assert_eq!(WhiteBalanceMode::OnePush as u8, 0x03);
        assert_eq!(WhiteBalanceMode::ATW as u8, 0x04);
        assert_eq!(WhiteBalanceMode::Manual as u8, 0x05);
        assert_eq!(WhiteBalanceMode::ColorTemperature as u8, 0x20);
    }

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_auto,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto
        },
        &[0x81, 0x01, 0x04, 0x35, 0x00, VISCA_TERMINATOR]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_indoor,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::Indoor
        },
        &[0x81, 0x01, 0x04, 0x35, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_outdoor,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::Outdoor
        },
        &[0x81, 0x01, 0x04, 0x35, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_one_push,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::OnePush
        },
        &[0x81, 0x01, 0x04, 0x35, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_atw,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::ATW
        },
        &[0x81, 0x01, 0x04, 0x35, 0x04, VISCA_TERMINATOR]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_manual,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::Manual
        },
        &[0x81, 0x01, 0x04, 0x35, 0x05, VISCA_TERMINATOR]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_color_temperature,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::ColorTemperature
        },
        &[0x81, 0x01, 0x04, 0x35, 0x20, VISCA_TERMINATOR]
    );

    #[test]
    fn test_white_balance_mode_try_from() {
        // Test valid conversions
        assert!(matches!(
            WhiteBalanceMode::try_from(0x00),
            Ok(WhiteBalanceMode::Auto)
        ));
        assert!(matches!(
            WhiteBalanceMode::try_from(0x01),
            Ok(WhiteBalanceMode::Indoor)
        ));
        assert!(matches!(
            WhiteBalanceMode::try_from(0x02),
            Ok(WhiteBalanceMode::Outdoor)
        ));
        assert!(matches!(
            WhiteBalanceMode::try_from(0x03),
            Ok(WhiteBalanceMode::OnePush)
        ));
        assert!(matches!(
            WhiteBalanceMode::try_from(0x04),
            Ok(WhiteBalanceMode::ATW)
        ));
        assert!(matches!(
            WhiteBalanceMode::try_from(0x05),
            Ok(WhiteBalanceMode::Manual)
        ));
        assert!(matches!(
            WhiteBalanceMode::try_from(0x20),
            Ok(WhiteBalanceMode::ColorTemperature)
        ));

        // Test invalid conversions
        assert!(WhiteBalanceMode::try_from(0x06).is_err());
        assert!(WhiteBalanceMode::try_from(0x10).is_err());
        assert!(WhiteBalanceMode::try_from(0xFF).is_err());
    }

    #[test]
    fn test_timeout_kind() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        assert_eq!(cmd.timeout_class(), CommandCategory::Quick);

        // Test with different modes
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Manual,
        };
        assert_eq!(cmd.timeout_class(), CommandCategory::Quick);
    }

    #[test]
    fn test_response_type() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        assert!(cmd.response_kind().is_none());

        // Test with different modes
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::ColorTemperature,
        };
        assert!(cmd.response_kind().is_none());
    }

    #[test]
    fn test_white_balance_command_debug() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        let debug_str = format!("{cmd:?}");
        assert!(debug_str.contains("WhiteBalanceCommand"));
        assert!(debug_str.contains("Auto"));
    }

    #[test]
    fn test_white_balance_mode_debug() {
        let mode = WhiteBalanceMode::Indoor;
        let debug_str = format!("{mode:?}");
        assert!(debug_str.contains("Indoor"));
    }

    #[test]
    fn test_white_balance_command_clone() {
        let cmd1 = WhiteBalanceCommand {
            mode: WhiteBalanceMode::OnePush,
        };
        let cmd2 = cmd1; // Copy
        let cmd3 = cmd1; // Copy (clone() not needed for Copy types)

        assert_eq!(
            cmd1.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd2.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
        assert_eq!(
            cmd1.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd3.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
    }

    #[test]
    fn test_all_modes_produce_valid_commands() {
        let modes = vec![
            WhiteBalanceMode::Auto,
            WhiteBalanceMode::Indoor,
            WhiteBalanceMode::Outdoor,
            WhiteBalanceMode::OnePush,
            WhiteBalanceMode::ATW,
            WhiteBalanceMode::Manual,
            WhiteBalanceMode::ColorTemperature,
        ];

        for mode in modes {
            let cmd = WhiteBalanceCommand { mode };
            let bytes = cmd
                .to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));

            // Verify command structure
            assert_eq!(bytes.len(), 6);
            assert_eq!(bytes[0], 0x81); // Command header
            assert_eq!(bytes[1], 0x01); // Command
            assert_eq!(bytes[2], 0x04); // Category
            assert_eq!(bytes[3], 0x35); // White balance command ID
            assert_eq!(bytes[4], mode as u8); // Mode value
            assert_eq!(bytes[5], 0xFF); // Terminator
        }
    }

    #[test]
    fn test_awb_sensitivity_commands() {
        // Test High sensitivity
        let cmd = AWBSensitivityCommand::new(AutoWhiteBalanceSensitivity::High);
        assert_eq!(
            cmd.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xA9, 0x00, VISCA_TERMINATOR]
        );

        // Test Normal sensitivity
        let cmd = AWBSensitivityCommand::new(AutoWhiteBalanceSensitivity::Normal);
        assert_eq!(
            cmd.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xA9, 0x01, VISCA_TERMINATOR]
        );

        // Test Low sensitivity
        let cmd = AWBSensitivityCommand::new(AutoWhiteBalanceSensitivity::Low);
        assert_eq!(
            cmd.to_bytes(crate::camera_id::CameraId::CAMERA_1)
                .map(|b| b.to_vec())
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xA9, 0x02, VISCA_TERMINATOR]
        );
    }
}
