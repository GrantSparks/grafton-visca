//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

use crate::macros::internal::*;

use crate::command::const_encoding::constants;

visca_command! {
    /// Command to control camera power state.
    category = "Quick",
    enum PowerCommand {
        /// Power on the camera.
        On => {
            Ok(CommandBuilder::<16>::new()
                .append(constants::power::ON))
        },
        /// Put camera in standby mode.
        Standby => {
            Ok(CommandBuilder::<16>::new()
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
        PowerCommand,
        test_power_on,
        PowerCommand::On,
        &[0x81, 0x01, 0x04, 0x00, 0x02, VISCA_TERMINATOR]
    );
    visca_test!(
        PowerCommand,
        test_power_standby,
        PowerCommand::Standby,
        &[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]
    );
}
