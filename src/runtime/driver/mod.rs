//! Runtime-neutral driver for VISCA send/recv pipeline.
//!
//! This module provides unified abstractions for the send/recv pipeline that
//! work across both async and blocking modes, eliminating code duplication
//! while maintaining zero-cost abstractions through monomorphization.

pub(crate) mod receive;
pub mod scheduler;
pub mod send;

#[cfg(feature = "mode-async")]
pub(crate) use receive::IgnoreReason;
pub(crate) use receive::{receive_one, ReceiveDisposition};
pub use scheduler::SchedulerLike;
pub(crate) use send::send_one;

#[cfg(not(feature = "mode-async"))]
pub(crate) use send::SendResult;

/// Zero-allocation hex display wrapper for wire-level byte logging.
///
/// Formats bytes as uppercase hex with spaces (e.g., `81 01 04 2C 07 FF`).
/// Implements `Display` so it can be used directly in `tracing` fields.
pub(crate) struct HexBytes<'a>(pub &'a [u8]);

impl std::fmt::Display for HexBytes<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, b) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{b:02X}")?;
        }
        Ok(())
    }
}

/// Create a displayable hex representation of a byte slice.
pub(crate) fn hex_bytes(bytes: &[u8]) -> HexBytes<'_> {
    HexBytes(bytes)
}
