//! White balance control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera white balance settings,
//! including auto, manual, and preset modes like indoor/outdoor/one-push/color temperature.

// Standard library imports
use std::convert::TryFrom;

// Crate imports
use crate::{
    command::{encode_visca::EncodeVisca, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// White balance modes.
///
/// Controls how the camera adjusts color temperature to ensure
/// white objects appear white under different lighting conditions.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum WhiteBalanceMode {
    /// Automatic white balance adjustment.
    Auto = 0x00,
    /// Indoor preset (optimized for incandescent/tungsten lighting).
    Indoor = 0x01,
    /// Outdoor preset (optimized for daylight).
    Outdoor = 0x02,
    /// One-push white balance (calibrate once based on current scene).
    OnePush = 0x03,
    /// Auto tracking white balance (FR7 specific).
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
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AutoWhiteBalanceSensitivity {
    /// Low sensitivity - slower, more stable adjustments.
    Low = 0x00,
    /// Normal sensitivity - balanced adjustment speed.
    Normal = 0x01,
    /// High sensitivity - faster adjustments to changing conditions.
    High = 0x02,
}

crate::visca_param_command! {
    /// Command to set the white balance mode.
    pub(crate) struct WhiteBalanceCommand {
        mode: WhiteBalanceMode,
    }
    prefix = [0x81, 0x01, 0x04, 0x35];
    param_byte = *mode as u8;
    timeout = Quick;
}

/// AWB Sensitivity levels (PTZOptics specific).
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AWBSensitivity {
    /// High sensitivity.
    High,
    /// Normal sensitivity (default).
    Normal,
    /// Low sensitivity.
    Low,
}

/// Command to set AWB sensitivity.
#[derive(Debug, Copy, Clone)]
pub(crate) struct AWBSensitivityCommand {
    /// The sensitivity level to set.
    pub sensitivity: AWBSensitivity,
}

impl EncodeVisca for AWBSensitivityCommand {
    type Response = ();
    const MAX_SIZE: usize = 6;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        let level = match self.sensitivity {
            AWBSensitivity::High => 0x00,
            AWBSensitivity::Normal => 0x01,
            AWBSensitivity::Low => 0x02,
        };

        buffer[0] = 0x81;
        buffer[1] = 0x01;
        buffer[2] = 0x04;
        buffer[3] = 0xA9;
        buffer[4] = level;
        buffer[5] = 0xFF;

        Ok(Self::MAX_SIZE)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

impl TryFrom<u8> for WhiteBalanceMode {
    type Error = Error;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0x00 => Ok(Self::Auto),
            0x01 => Ok(Self::Indoor),
            0x02 => Ok(Self::Outdoor),
            0x03 => Ok(Self::OnePush),
            0x04 => Ok(Self::ATW),
            0x05 => Ok(Self::Manual),
            0x20 => Ok(Self::ColorTemperature),
            _ => Err(Error::InvalidResponse {
                expected: "0x00 (Auto), 0x01 (Indoor), 0x02 (Outdoor), 0x03 (OnePush), 0x04 (ATW), 0x05 (Manual), or 0x20 (ColorTemperature)".to_string(),
                actual: vec![v],
            }),
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::{visca_test, EncodeVisca};

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
        &[0x81, 0x01, 0x04, 0x35, 0x00, 0xFF]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_indoor,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::Indoor
        },
        &[0x81, 0x01, 0x04, 0x35, 0x01, 0xFF]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_outdoor,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::Outdoor
        },
        &[0x81, 0x01, 0x04, 0x35, 0x02, 0xFF]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_one_push,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::OnePush
        },
        &[0x81, 0x01, 0x04, 0x35, 0x03, 0xFF]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_atw,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::ATW
        },
        &[0x81, 0x01, 0x04, 0x35, 0x04, 0xFF]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_manual,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::Manual
        },
        &[0x81, 0x01, 0x04, 0x35, 0x05, 0xFF]
    );

    visca_test!(
        WhiteBalanceCommand,
        test_white_balance_command_color_temperature,
        WhiteBalanceCommand {
            mode: WhiteBalanceMode::ColorTemperature
        },
        &[0x81, 0x01, 0x04, 0x35, 0x20, 0xFF]
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
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);

        // Test with different modes
        let cmd = WhiteBalanceCommand {
            mode: WhiteBalanceMode::Manual,
        };
        assert_eq!(cmd.timeout_kind(), CommandCategory::Quick);
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
            cmd1.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd2.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        );
        assert_eq!(
            cmd1.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            cmd3.try_into_vec()
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
                .try_into_vec()
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
        let cmd = AWBSensitivityCommand {
            sensitivity: AWBSensitivity::High,
        };
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xA9, 0x00, 0xFF]
        );

        // Test Normal sensitivity
        let cmd = AWBSensitivityCommand {
            sensitivity: AWBSensitivity::Normal,
        };
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xA9, 0x01, 0xFF]
        );

        // Test Low sensitivity
        let cmd = AWBSensitivityCommand {
            sensitivity: AWBSensitivity::Low,
        };
        assert_eq!(
            cmd.try_into_vec()
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}")),
            vec![0x81, 0x01, 0x04, 0xA9, 0x02, 0xFF]
        );
    }
}
