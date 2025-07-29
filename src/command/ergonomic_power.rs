//! Ergonomic Power command builders with zero dead code.
//!
//! This demonstrates the hybrid approach for power management commands.

use crate::command::{ViscaCommand, ResponseType};

/// Power command that will actually be sent to the camera.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerCommand {
    bytes: Vec<u8>,
    response_type: ResponseType,
}

impl ViscaCommand for PowerCommand {
    fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn response_type(&self) -> ResponseType {
        self.response_type
    }
}

/// Ergonomic builder for Power commands.
/// 
/// Only generates the commands that are actually used.
pub struct Power;

impl Power {
    /// Turn camera power ON.
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_power::Power;
    /// 
    /// let cmd = Power::on();
    /// ```
    pub const fn on() -> PowerCommand {
        PowerCommand {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF],
            response_type: ResponseType::Completion,
        }
    }

    /// Turn camera power OFF (standby mode).
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_power::Power;
    /// 
    /// let cmd = Power::off();
    /// ```
    pub const fn off() -> PowerCommand {
        PowerCommand {
            bytes: vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF],
            response_type: ResponseType::Completion,
        }
    }

    /// Query current power state.
    /// 
    /// # Example
    /// ```
    /// use grafton_visca::command::ergonomic_power::Power;
    /// 
    /// let cmd = Power::inquiry();
    /// ```
    pub const fn inquiry() -> PowerCommand {
        PowerCommand {
            bytes: vec![0x81, 0x09, 0x04, 0x00, 0xFF],
            response_type: ResponseType::Inquiry,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_on() {
        let cmd = Power::on();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_power_off() {
        let cmd = Power::off();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Completion);
    }

    #[test]
    fn test_power_inquiry() {
        let cmd = Power::inquiry();
        assert_eq!(cmd.to_bytes(), vec![0x81, 0x09, 0x04, 0x00, 0xFF]);
        assert_eq!(cmd.response_type(), ResponseType::Inquiry);
    }
}