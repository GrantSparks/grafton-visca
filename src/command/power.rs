//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

use crate::{timeout::CommandCategory, visca_cmd};

visca_cmd! {
    /// Power on command.
    pub struct PowerOn;
    bytes = [0x01, 0x04, 0x00, 0x02];
    category = CommandCategory::Quick;
}

impl Default for PowerOn {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerOn {
    /// Create a new power on command.
    pub fn new() -> Self {
        PowerOn
    }
}

visca_cmd! {
    /// Power standby command.
    pub struct PowerStandby;
    bytes = [0x01, 0x04, 0x00, 0x03];
    category = CommandCategory::Quick;
}

impl Default for PowerStandby {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerStandby {
    /// Create a new power standby command.
    pub fn new() -> Self {
        PowerStandby
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    visca_test!(
        PowerOn,
        test_power_on,
        PowerOn::new(),
        &[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        PowerStandby,
        test_power_standby,
        PowerStandby::new(),
        &[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]
    );
}
