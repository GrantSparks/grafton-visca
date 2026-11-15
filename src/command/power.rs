//! Power control commands for VISCA cameras.
//!
//! This module provides commands for controlling camera power state.

use grafton_visca_macros::ViscaEnum;

use crate::{command::bytes::constants::power, macros::consolidated::visca_command};

/// The status reported by a camera's power inquiry.
///
/// A circuit error is a documented camera status, not a malformed reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub enum PowerStatus {
    /// The camera is powered on (`02`).
    On = 0x02,
    /// The camera is in standby (`03`).
    Standby = 0x03,
    /// The camera reports an internal power circuit error (`04`).
    CircuitError = 0x04,
}

impl PowerStatus {
    /// Returns whether the camera reports that it is powered on.
    ///
    /// ```
    /// use grafton_visca::PowerStatus;
    /// assert!(PowerStatus::On.is_on());
    /// assert!(!PowerStatus::CircuitError.is_on());
    /// ```
    #[must_use]
    pub const fn is_on(self) -> bool {
        matches!(self, Self::On)
    }
}

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
