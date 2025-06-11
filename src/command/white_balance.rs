//! White balance control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera white balance settings,
//! including auto, manual, and preset modes like indoor/outdoor/one-push/color temperature.

// Standard library imports
use std::convert::TryFrom;

// Crate imports
use crate::{
    command::{Command, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// White balance modes.
///
/// Controls how the camera adjusts color temperature to ensure
/// white objects appear white under different lighting conditions.
#[derive(Debug, Copy, Clone)]
pub enum WhiteBalanceMode {
    /// Automatic white balance adjustment.
    Auto = 0x00,
    /// Indoor preset (optimized for incandescent/tungsten lighting).
    Indoor = 0x01,
    /// Outdoor preset (optimized for daylight).
    Outdoor = 0x02,
    /// One-push white balance (calibrate once based on current scene).
    OnePush = 0x03,
    /// Manual white balance control.
    Manual = 0x05,
    /// Color temperature mode (specify exact color temperature).
    ColorTemperature = 0x20,
}

/// Command to set the white balance mode.
#[derive(Debug, Copy, Clone)]
pub struct WhiteBalanceCommand {
    /// The white balance mode to set.
    pub mode: WhiteBalanceMode,
}

impl Command for WhiteBalanceCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![0x81, 0x01, 0x04, 0x35, self.mode as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl TryFrom<u8> for WhiteBalanceMode {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(Self::Auto),
            0x01 => Ok(Self::Indoor),
            0x02 => Ok(Self::Outdoor),
            0x03 => Ok(Self::OnePush),
            0x05 => Ok(Self::Manual),
            0x20 => Ok(Self::ColorTemperature),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_white_balance_mode_values() {
        assert_eq!(WhiteBalanceMode::Auto as u8, 0x00);
        assert_eq!(WhiteBalanceMode::Indoor as u8, 0x01);
        assert_eq!(WhiteBalanceMode::Outdoor as u8, 0x02);
        assert_eq!(WhiteBalanceMode::OnePush as u8, 0x03);
        assert_eq!(WhiteBalanceMode::Manual as u8, 0x05);
        assert_eq!(WhiteBalanceMode::ColorTemperature as u8, 0x20);
    }

    #[test]
    fn test_white_balance_command_auto() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x35, 0x00, 0xFF]
        );
    }

    #[test]
    fn test_white_balance_command_indoor() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Indoor,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x35, 0x01, 0xFF]
        );
    }

    #[test]
    fn test_white_balance_command_outdoor() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Outdoor,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x35, 0x02, 0xFF]
        );
    }

    #[test]
    fn test_white_balance_command_one_push() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::OnePush,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x35, 0x03, 0xFF]
        );
    }

    #[test]
    fn test_white_balance_command_manual() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Manual,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x35, 0x05, 0xFF]
        );
    }

    #[test]
    fn test_white_balance_command_color_temperature() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::ColorTemperature,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x35, 0x20, 0xFF]
        );
    }

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
            WhiteBalanceMode::try_from(0x05),
            Ok(WhiteBalanceMode::Manual)
        ));
        assert!(matches!(
            WhiteBalanceMode::try_from(0x20),
            Ok(WhiteBalanceMode::ColorTemperature)
        ));

        // Test invalid conversions
        assert!(WhiteBalanceMode::try_from(0x04).is_err());
        assert!(WhiteBalanceMode::try_from(0x10).is_err());
        assert!(WhiteBalanceMode::try_from(0xFF).is_err());
    }

    #[test]
    fn test_command_category() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        assert_eq!(cmd.command_category(), CommandCategory::Quick);

        // Test with different modes
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Manual,
        };
        assert_eq!(cmd.command_category(), CommandCategory::Quick);
    }

    #[test]
    fn test_response_type() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        assert!(cmd.response_type().is_none());

        // Test with different modes
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::ColorTemperature,
        };
        assert!(cmd.response_type().is_none());
    }

    #[test]
    fn test_white_balance_command_debug() {
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Auto,
        };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("WhiteBalanceCommand"));
        assert!(debug_str.contains("Auto"));
    }

    #[test]
    fn test_white_balance_mode_debug() {
        let mode = WhiteBalanceMode::Indoor;
        let debug_str = format!("{:?}", mode);
        assert!(debug_str.contains("Indoor"));
    }

    #[test]
    fn test_white_balance_command_clone() {
        let cmd1 = WhiteBalanceCommand {
            mode: WhiteBalanceMode::OnePush,
        };
        let cmd2 = cmd1; // Copy
        let cmd3 = cmd1.clone(); // Clone

        assert_eq!(cmd1.to_bytes().unwrap(), cmd2.to_bytes().unwrap());
        assert_eq!(cmd1.to_bytes().unwrap(), cmd3.to_bytes().unwrap());
    }

    #[test]
    fn test_all_modes_produce_valid_commands() {
        let modes = vec![
            WhiteBalanceMode::Auto,
            WhiteBalanceMode::Indoor,
            WhiteBalanceMode::Outdoor,
            WhiteBalanceMode::OnePush,
            WhiteBalanceMode::Manual,
            WhiteBalanceMode::ColorTemperature,
        ];

        for mode in modes {
            let cmd = WhiteBalanceCommand { mode };
            let bytes = cmd.to_bytes().unwrap();

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
}
