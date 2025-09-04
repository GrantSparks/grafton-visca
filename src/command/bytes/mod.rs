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

/// Check if VISCA bytes represent an inquiry command.
///
/// VISCA protocol defines inquiries by the second byte being 0x09.
/// This is the canonical indicator per the protocol specification.
///
/// # Arguments
/// * `bytes` - The VISCA command bytes to check
///
/// # Returns
/// * `true` if the bytes represent an inquiry (bytes[1] == 0x09)
/// * `false` otherwise (including if bytes is too short)
#[inline(always)]
pub fn is_inquiry_bytes(bytes: &[u8]) -> bool {
    matches!(bytes.get(1), Some(&0x09))
}
