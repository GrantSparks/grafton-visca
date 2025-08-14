//! Tally light control methods for cameras.

use crate::Error;

/// Tally light control operations (async).
#[cfg(feature = "async")]
pub trait TallyOps: Sized {
    /// Turn red tally light on.
    async fn tally_red_on(&self) -> Result<(), Error>;

    /// Turn red tally light off.
    async fn tally_red_off(&self) -> Result<(), Error>;

    /// Set tally brightness to low.
    async fn tally_bright_lo(&self) -> Result<(), Error>;

    /// Set tally brightness to high.
    async fn tally_bright_hi(&self) -> Result<(), Error>;

    /// Turn green tally light on.
    async fn tally_green_on(&self) -> Result<(), Error>;

    /// Turn green tally light off.
    async fn tally_green_off(&self) -> Result<(), Error>;

    /// Flash tally light.
    async fn tally_flash(&self) -> Result<(), Error>;

    /// Turn tally light on.
    async fn tally_on(&self) -> Result<(), Error>;

    /// Turn tally light off.
    async fn tally_off(&self) -> Result<(), Error>;

    /// Get tally light status.
    async fn get_tally_status(&self) -> Result<bool, Error>;

    /// Query red tally light state.
    async fn get_red_tally_status(&self) -> Result<bool, Error>;

    /// Query green tally light state (FR7 specific).
    async fn get_green_tally_status(&self) -> Result<bool, Error>;
}

/// Tally light control operations (blocking).
pub trait TallyOpsBlocking: Sized {
    /// Turn red tally light on.
    fn tally_red_on(&self) -> Result<(), Error>;

    /// Turn red tally light off.
    fn tally_red_off(&self) -> Result<(), Error>;

    /// Set tally brightness to low.
    fn tally_bright_lo(&self) -> Result<(), Error>;

    /// Set tally brightness to high.
    fn tally_bright_hi(&self) -> Result<(), Error>;

    /// Turn green tally light on.
    fn tally_green_on(&self) -> Result<(), Error>;

    /// Turn green tally light off.
    fn tally_green_off(&self) -> Result<(), Error>;

    /// Flash tally light.
    fn tally_flash(&self) -> Result<(), Error>;

    /// Turn tally light on.
    fn tally_on(&self) -> Result<(), Error>;

    /// Turn tally light off.
    fn tally_off(&self) -> Result<(), Error>;

    /// Get tally light status.
    fn get_tally_status(&self) -> Result<bool, Error>;

    /// Query red tally light state.
    fn get_red_tally_status(&self) -> Result<bool, Error>;

    /// Query green tally light state (FR7 specific).
    fn get_green_tally_status(&self) -> Result<bool, Error>;
}

// Async implementation
