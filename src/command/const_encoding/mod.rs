//! Zero-allocation, compile-time command encoding for VISCA protocol.
//!
//! This module provides const command encoding to eliminate heap allocations
//! and enable compile-time validation of VISCA command byte sequences.

pub mod builder;
pub mod constants;
pub mod macros;

pub use builder::CommandBuilder;

// ND Filter encoding functions (these are actually used)
/// Const function for fixed ND filter control.
pub const fn encode_nd_filter_fixed(enabled: bool) -> [u8; 6] {
    CommandBuilder::<6>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x06])
        .byte(if enabled { 0x02 } else { 0x03 })
        .build()
}

/// Const function for stepped ND filter control.
pub const fn encode_nd_filter_stepped(step: u8) -> [u8; 6] {
    let step = if step > 8 { 8 } else { step };
    CommandBuilder::<6>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x06])
        .byte(step)
        .build()
}

/// Const function for variable ND filter control.
pub const fn encode_nd_filter_variable(value: u8) -> [u8; 9] {
    CommandBuilder::<9>::from_prefix(&[DEFAULT_ADDRESS, 0x01, 0x04, 0x06, 0x00, 0x00])
        .visca_u14(value as u16)
        .build()
}
// pub use constants::*;  // Commented out - unused

/// VISCA command terminator byte.
pub const VISCA_TERMINATOR: u8 = 0xFF;

/// Default camera address for VISCA commands.
pub const DEFAULT_ADDRESS: u8 = 0x81;
