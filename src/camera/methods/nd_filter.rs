//! ND filter methods for cameras that support ND filters using the new GAT architecture.
//!
//! These methods ONLY exist for cameras that implement NDFilter.

use crate::{
    command::{NDFilterMode as CommandNDFilterMode, NDFilterStep},
    Error,
};

/// ND filter operations (async).
#[cfg(feature = "async")]
pub trait NDFilterOps: Sized {
    /// Set ND filter mode (preset or variable).
    async fn set_nd_filter_mode(&self, mode: CommandNDFilterMode) -> Result<(), Error>;

    /// Set ND filter value directly (for variable mode).
    async fn set_nd_filter_value(&self, value: u16) -> Result<(), Error>;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    async fn set_nd_filter_stops(&self, stops: f32) -> Result<(), Error>;

    /// Step ND filter up or down.
    async fn step_nd_filter(&self, direction: NDFilterStep) -> Result<(), Error>;

    /// Enable or disable auto ND.
    async fn set_auto_nd(&self, enabled: bool) -> Result<(), Error>;

    /// Get current ND filter setting.
    async fn get_nd_filter(&self) -> Result<u8, Error>;
}

/// ND filter operations (blocking).
pub trait NDFilterOpsBlocking: Sized {
    /// Set ND filter mode (preset or variable).
    fn set_nd_filter_mode(&self, mode: CommandNDFilterMode) -> Result<(), Error>;

    /// Set ND filter value directly (for variable mode).
    fn set_nd_filter_value(&self, value: u16) -> Result<(), Error>;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    fn set_nd_filter_stops(&self, stops: f32) -> Result<(), Error>;

    /// Step ND filter up or down.
    fn step_nd_filter(&self, direction: NDFilterStep) -> Result<(), Error>;

    /// Enable or disable auto ND.
    fn set_auto_nd(&self, enabled: bool) -> Result<(), Error>;

    /// Get current ND filter setting.
    fn get_nd_filter(&self) -> Result<u8, Error>;
}

// Async implementation
