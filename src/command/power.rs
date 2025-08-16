//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

use crate::macros::internal::*;

use crate::command::const_encoding::constants;

visca_command! {
    /// Command to control camera power state.
    category = "Quick",
    enum Power {
        /// Power on the camera.
        On => {
            Ok(ConstCommandBuilder::<16>::new()
                .append(constants::power::ON))
        },
        /// Put camera in standby mode.
        Standby => {
            Ok(ConstCommandBuilder::<16>::new()
                .append(constants::power::OFF))
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::const_encoding::VISCA_TERMINATOR;
    use crate::macros::test_utils::visca_test;

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
}
