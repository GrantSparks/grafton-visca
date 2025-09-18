//! Tally light control commands for VISCA cameras.
//!
//! This module provides commands for controlling tally lights on compatible cameras.
//! Tally lights indicate when a camera is active or being used.
//!
//! # VISCA Compliance
//! Basic red tally light control is part of baseline VISCA.
//!
//! ## Vendor-Specific Features
//! - Green tally light (`GreenOn`, `GreenOff`) - Sony FR7 specific
//! - Flash/solid modes (`Flash`, `On`, `Off`) - PtzOptics specific

use crate::{timeout::CommandCategory, visca_command};

visca_command! {
        /// Turn red tally light on.
    pub struct TallyRedOn;
    bytes = [0x01, 0x7E, 0x01, 0x0A, 0x00, 0x02];
    category = CommandCategory::Quick;
}

visca_command! {
        /// Turn red tally light off.
    pub struct TallyRedOff;
    bytes = [0x01, 0x7E, 0x01, 0x0A, 0x00, 0x03];
    category = CommandCategory::Quick;
}

visca_command! {
        /// Set tally brightness to low.
    pub struct TallyBrightLo;
    bytes = [0x01, 0x7E, 0x01, 0x0A, 0x01, 0x04];
    category = CommandCategory::Quick;
}

visca_command! {
        /// Set tally brightness to high.
    pub struct TallyBrightHi;
    bytes = [0x01, 0x7E, 0x01, 0x0A, 0x01, 0x05];
    category = CommandCategory::Quick;
}

visca_command! {
        /// Turn green tally light on (FR7 specific).
    pub struct TallyGreenOn;
    bytes = [0x01, 0x7E, 0x04, 0x1A, 0x00, 0x02];
    category = CommandCategory::Quick;
}

visca_command! {
        /// Turn green tally light off (FR7 specific).
    pub struct TallyGreenOff;
    bytes = [0x01, 0x7E, 0x04, 0x1A, 0x00, 0x03];
    category = CommandCategory::Quick;
}

visca_command! {
        /// Set tally to flash mode (PtzOptics specific).
    pub struct TallyFlash;
    bytes = [0x0A, 0x02, 0x02, 0x01];
    category = CommandCategory::Quick;
}

visca_command! {
        /// Set tally to solid on (PtzOptics specific).
    pub struct TallyOn;
    bytes = [0x0A, 0x02, 0x02, 0x02];
    category = CommandCategory::Quick;
}

visca_command! {
        /// Turn tally off (PtzOptics specific).
    pub struct TallyOff;
    bytes = [0x0A, 0x02, 0x02, 0x03];
    category = CommandCategory::Quick;
}

impl Default for TallyRedOn {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyRedOn {
    /// Create a new red tally on command.
    pub fn new() -> Self {
        TallyRedOn
    }
}

impl Default for TallyRedOff {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyRedOff {
    /// Create a new red tally off command.
    pub fn new() -> Self {
        TallyRedOff
    }
}

impl Default for TallyBrightLo {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyBrightLo {
    /// Create a new tally low brightness command.
    pub fn new() -> Self {
        TallyBrightLo
    }
}

impl Default for TallyBrightHi {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyBrightHi {
    /// Create a new tally high brightness command.
    pub fn new() -> Self {
        TallyBrightHi
    }
}

impl Default for TallyGreenOn {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyGreenOn {
    /// Create a new green tally on command.
    pub fn new() -> Self {
        TallyGreenOn
    }
}

impl Default for TallyGreenOff {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyGreenOff {
    /// Create a new green tally off command.
    pub fn new() -> Self {
        TallyGreenOff
    }
}

impl Default for TallyFlash {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyFlash {
    /// Create a new tally flash command.
    pub fn new() -> Self {
        TallyFlash
    }
}

impl Default for TallyOn {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyOn {
    /// Create a new tally on command.
    pub fn new() -> Self {
        TallyOn
    }
}

impl Default for TallyOff {
    fn default() -> Self {
        Self::new()
    }
}

impl TallyOff {
    /// Create a new tally off command.
    pub fn new() -> Self {
        TallyOff
    }
}

#[cfg(test)]
mod tests {
    use crate::{command::bytes::VISCA_TERMINATOR, macros::test_utils::visca_test};

    use super::*;

    visca_test!(
        TallyRedOn,
        test_red_on,
        TallyRedOn::new(),
        &[0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        TallyRedOff,
        test_red_off,
        TallyRedOff::new(),
        &[0x81, 0x01, 0x7E, 0x01, 0x0A, 0x00, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        TallyBrightLo,
        test_bright_lo,
        TallyBrightLo::new(),
        &[0x81, 0x01, 0x7E, 0x01, 0x0A, 0x01, 0x04, VISCA_TERMINATOR]
    );

    visca_test!(
        TallyBrightHi,
        test_bright_hi,
        TallyBrightHi::new(),
        &[0x81, 0x01, 0x7E, 0x01, 0x0A, 0x01, 0x05, VISCA_TERMINATOR]
    );

    visca_test!(
        TallyGreenOn,
        test_green_on,
        TallyGreenOn::new(),
        &[0x81, 0x01, 0x7E, 0x04, 0x1A, 0x00, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        TallyGreenOff,
        test_green_off,
        TallyGreenOff::new(),
        &[0x81, 0x01, 0x7E, 0x04, 0x1A, 0x00, 0x03, VISCA_TERMINATOR]
    );

    visca_test!(
        TallyFlash,
        test_flash,
        TallyFlash::new(),
        &[0x81, 0x0A, 0x02, 0x02, 0x01, VISCA_TERMINATOR]
    );

    visca_test!(
        TallyOn,
        test_on,
        TallyOn::new(),
        &[0x81, 0x0A, 0x02, 0x02, 0x02, VISCA_TERMINATOR]
    );

    visca_test!(
        TallyOff,
        test_off,
        TallyOff::new(),
        &[0x81, 0x0A, 0x02, 0x02, 0x03, VISCA_TERMINATOR]
    );
}
