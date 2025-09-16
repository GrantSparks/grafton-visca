//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

use crate::command::bytes::constants;
use crate::macros::internal::*;
use crate::timeout::CommandCategory;

// Legacy command enum - maintained for backwards compatibility
visca_command! {
    /// Command to control camera power state.
    category = CommandCategory::Quick,
    max_size = 6, // POWER constants (5 bytes) + 1 terminator = 6
    enum Power {
        /// Power on the camera.
        On => {
            Ok(ConstCommandBuilder::<6>::new()
                .append(constants::power::ON))
        },
        /// Put camera in standby mode.
        Standby => {
            Ok(ConstCommandBuilder::<6>::new()
                .append(constants::power::OFF))
        },
    }
}

// New consolidated macro examples - demonstrating the new API
visca_cmd! {
    /// Power on the camera using new consolidated macro.
    pub struct PowerOn;
    bytes = [0x01, 0x04, 0x00, 0x02];
    category = CommandCategory::Quick;
}

visca_cmd! {
    /// Put camera in standby mode using new consolidated macro.
    pub struct PowerStandby;
    bytes = [0x01, 0x04, 0x00, 0x03];
    category = CommandCategory::Quick;
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::command::bytes::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

    // Legacy tests
    visca_test!(
        Power,
        test_power_on,
        Power::On,
        &[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]
    );
    visca_test!(
        Power,
        test_power_standby,
        Power::Standby,
        &[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]
    );

    // New macro tests
    visca_test!(
        PowerOn,
        test_power_on_new,
        PowerOn::new(),
        &[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]
    );
    visca_test!(
        PowerStandby,
        test_power_standby_new,
        PowerStandby::new(),
        &[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]
    );
}
