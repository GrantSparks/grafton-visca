//! Nibble combination utilities for VISCA response parsing.

/// Combines 4 nibbles into a u16 value.
///
/// Takes 4 bytes in nibble format (0x0p 0x0q 0x0r 0x0s) and combines them
/// into a single 16-bit value: (p << 12) | (q << 8) | (r << 4) | s
#[inline]
pub(crate) fn combine_nibbles_u16(n: &[u8]) -> u16 {
    if n.len() < 4 {
        return 0;
    }
    ((n[0] as u16) << 12) | ((n[1] as u16) << 8) | ((n[2] as u16) << 4) | (n[3] as u16)
}

/// Combines 4 nibbles into an i16 value.
///
/// Uses the same conversion as combine_nibbles_u16 but interprets the result as signed.
#[inline]
pub(crate) fn combine_nibbles_i16(n: &[u8]) -> i16 {
    combine_nibbles_u16(n) as i16
}

/// Combines 2 nibbles into a u8 value.
///
/// Takes 2 bytes in nibble format (0x0p 0x0q) and combines them
/// into a single 8-bit value: (p << 4) | q
#[inline]
pub(crate) fn combine_nibbles_u8(n: &[u8]) -> u8 {
    if n.len() < 2 {
        return 0;
    }
    ((n[0] & 0x0F) << 4) | (n[1] & 0x0F)
}
