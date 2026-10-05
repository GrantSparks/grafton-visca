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

use crate::{command::bytes::constants::tally, visca_command};

visca_command! {
    /// Turn red tally light on.
    pub struct TallyRedOn;
    bytes = tally::RED_ON;
}

visca_command! {
    /// Turn red tally light off.
    pub struct TallyRedOff;
    bytes = tally::RED_OFF;
}

visca_command! {
    /// Set tally brightness to low.
    pub struct TallyBrightLo;
    bytes = tally::BRIGHT_LOW;
}

visca_command! {
    /// Set tally brightness to high.
    pub struct TallyBrightHi;
    bytes = tally::BRIGHT_HIGH;
}

visca_command! {
    /// Turn green tally light on (FR7 specific).
    pub struct TallyGreenOn;
    bytes = tally::GREEN_ON;
}

visca_command! {
    /// Turn green tally light off (FR7 specific).
    pub struct TallyGreenOff;
    bytes = tally::GREEN_OFF;
}

visca_command! {
    /// Set tally to flash mode (PtzOptics specific).
    pub struct TallyFlash;
    bytes = tally::PTZOPTICS_FLASH;
}

visca_command! {
    /// Set tally to solid on (PtzOptics specific).
    pub struct TallyOn;
    bytes = tally::PTZOPTICS_ON;
}

visca_command! {
    /// Turn tally off (PtzOptics specific).
    pub struct TallyOff;
    bytes = tally::PTZOPTICS_OFF;
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
