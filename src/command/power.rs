//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{Command, ResponseType},
    error::Error,
    timeout::CommandCategory,
};

/// Power state for the camera.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Power {
    /// Camera is powered on and operational.
    On = 0x02,
    /// Camera is in standby mode.
    Standby = 0x03,
}

/// Command to set camera power state.
#[derive(Debug, Copy, Clone)]
pub struct PowerCommand {
    /// The desired power state.
    pub power: Power,
}

impl Command for PowerCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(vec![0x81, 0x01, 0x04, 0x00, self.power as u8, 0xFF])
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn command_category(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_on_command() {
        let cmd = PowerCommand { power: Power::On };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

    #[test]
    fn test_power_standby_command() {
        let cmd = PowerCommand {
            power: Power::Standby,
        };
        assert_eq!(
            cmd.to_bytes().unwrap(),
            vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]
        );
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

    #[test]
    fn test_power_enum_values() {
        assert_eq!(Power::On as u8, 0x02);
        assert_eq!(Power::Standby as u8, 0x03);
    }

    #[test]
    fn test_power_enum_equality() {
        assert_eq!(Power::On, Power::On);
        assert_eq!(Power::Standby, Power::Standby);
        assert_ne!(Power::On, Power::Standby);
    }

    #[test]
    fn test_power_command_debug() {
        let cmd = PowerCommand { power: Power::On };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("PowerCommand"));
        assert!(debug_str.contains("On"));

        let cmd = PowerCommand {
            power: Power::Standby,
        };
        let debug_str = format!("{:?}", cmd);
        assert!(debug_str.contains("Standby"));
    }

    #[test]
    fn test_power_command_clone() {
        let cmd1 = PowerCommand { power: Power::On };
        let cmd2 = cmd1.clone();
        assert_eq!(cmd1.power, cmd2.power);

        let cmd1 = PowerCommand {
            power: Power::Standby,
        };
        let cmd2 = cmd1; // Copy trait
        assert_eq!(cmd2.power, Power::Standby);
    }

    #[test]
    fn test_power_enum_debug() {
        assert_eq!(format!("{:?}", Power::On), "On");
        assert_eq!(format!("{:?}", Power::Standby), "Standby");
    }

    #[test]
    fn test_power_enum_clone() {
        let power1 = Power::On;
        let power2 = power1.clone();
        assert_eq!(power1, power2);

        let power1 = Power::Standby;
        let power2 = power1; // Copy trait
        assert_eq!(power2, Power::Standby);
    }

    #[test]
    fn test_response_type_none() {
        // Power commands don't expect a response beyond ACK/completion
        let cmd_on = PowerCommand { power: Power::On };
        assert!(cmd_on.response_type().is_none());

        let cmd_standby = PowerCommand {
            power: Power::Standby,
        };
        assert!(cmd_standby.response_type().is_none());
    }

    #[test]
    fn test_command_trait_impl() {
        // Verify PowerCommand implements Command trait
        let cmd: Box<dyn Command> = Box::new(PowerCommand { power: Power::On });
        assert!(cmd.to_bytes().is_ok());
        assert!(cmd.response_type().is_none());
        assert!(matches!(cmd.command_category(), CommandCategory::Quick));
    }

    #[test]
    fn test_byte_sequence_correctness() {
        // Verify the exact byte sequences match VISCA protocol
        let on_cmd = PowerCommand { power: Power::On };
        let on_bytes = on_cmd.to_bytes().unwrap();
        assert_eq!(on_bytes[0], 0x81); // Command header
        assert_eq!(on_bytes[1], 0x01); // Command type
        assert_eq!(on_bytes[2], 0x04); // Category
        assert_eq!(on_bytes[3], 0x00); // Power command
        assert_eq!(on_bytes[4], 0x02); // On value
        assert_eq!(on_bytes[5], 0xFF); // Terminator

        let standby_cmd = PowerCommand {
            power: Power::Standby,
        };
        let standby_bytes = standby_cmd.to_bytes().unwrap();
        assert_eq!(standby_bytes[0], 0x81); // Command header
        assert_eq!(standby_bytes[1], 0x01); // Command type
        assert_eq!(standby_bytes[2], 0x04); // Category
        assert_eq!(standby_bytes[3], 0x00); // Power command
        assert_eq!(standby_bytes[4], 0x03); // Standby value
        assert_eq!(standby_bytes[5], 0xFF); // Terminator
    }
}
