//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

use crate::{command::bytes::constants::power, visca_command};

visca_command! {
    /// Power on command.
    pub struct PowerOn;
    bytes = power::ON;
}

visca_command! {
    /// Power standby command.
    pub struct PowerStandby;
    bytes = power::STANDBY;
}

#[cfg(test)]
mod tests {
    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    use super::*;

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
