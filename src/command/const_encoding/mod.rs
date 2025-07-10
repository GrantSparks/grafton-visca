//! Zero-allocation, compile-time command encoding for VISCA protocol.
//!
//! This module provides const command encoding to eliminate heap allocations
//! and enable compile-time validation of VISCA command byte sequences.

pub mod builder;
pub mod commands;
pub mod constants;
pub mod encoding;
pub mod macros;

pub use builder::CommandBuilder;
pub use constants::*;
pub use encoding::*;

/// Maximum size for any VISCA command.
pub const MAX_COMMAND_SIZE: usize = 32;

/// VISCA command terminator byte.
pub const VISCA_TERMINATOR: u8 = 0xFF;

/// Default camera address for VISCA commands.
pub const DEFAULT_ADDRESS: u8 = 0x81;
