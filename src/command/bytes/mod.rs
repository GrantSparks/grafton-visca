//! Zero-allocation, compile-time command encoding for VISCA protocol.
//!
//! This module provides const command encoding to eliminate heap allocations
//! and enable compile-time validation of VISCA command byte sequences.

pub mod builder;
pub mod constants;

pub use builder::ConstCommandBuilder;

/// VISCA command terminator byte.
pub const VISCA_TERMINATOR: u8 = 0xFF;

/// Default camera address for VISCA commands.
pub const DEFAULT_ADDRESS: u8 = 0x81;
