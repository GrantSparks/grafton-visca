//! Blocking transport implementations for VISCA communication.
//!
//! This module provides synchronous I/O for VISCA camera control,
//! designed as the primary API for most use cases.

pub mod tcp;
pub mod udp;

// Re-exports
pub use tcp::Tcp;
pub use udp::Udp;

#[cfg(test)]
mod tests;
