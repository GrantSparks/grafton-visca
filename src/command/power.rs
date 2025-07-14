//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

use crate::{command::const_encoding::constants, visca_command};

visca_command! {
    /// Command to control camera power state.
    category = "Quick",
    enum PowerCommand {
        /// Power on the camera.
        On => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(constants::power::ON)
                .build();
            Ok::<Vec<u8>, crate::Error>(cmd.to_vec())
        },
        /// Put camera in standby mode.
        Standby => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(constants::power::OFF)
                .build();
            Ok::<Vec<u8>, crate::Error>(cmd.to_vec())
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{visca_test, EncodeVisca};

    visca_test!(
        PowerCommand,
        test_power_on,
        PowerCommand::On,
        &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]
    );
    visca_test!(
        PowerCommand,
        test_power_standby,
        PowerCommand::Standby,
        &[0x81, 0x01, 0x04, 0x00, 0x03, 0xFF]
    );
}
