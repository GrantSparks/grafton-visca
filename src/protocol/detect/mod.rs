//! Protocol detection implementation using a runtime-agnostic state machine.
//!
//! This module provides a unified protocol detection algorithm that works
//! with both async and blocking transports by separating the detection logic
//! (in `core`) from the I/O operations (in the runners).

pub(crate) mod core;

#[cfg(feature = "async")]
pub(crate) mod async_runner;

#[cfg(not(feature = "async"))]
pub(crate) mod blocking_runner;
