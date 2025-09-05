//! Type-safe payload views for VISCA inquiry response decoding.
//!
//! This module provides const-generic, zero-cost views over VISCA payload bytes
//! to eliminate manual slicing errors and provide compile-time guarantees about
//! payload shape and size.

use crate::error::Error;

/// Zero-copy view into VISCA nibble payloads.
///
/// This wrapper provides a type-safe interface to raw payload bytes,
/// ensuring nibble invariants in debug builds while maintaining zero-cost
/// abstraction in release builds.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Payload<'a>(pub(crate) &'a [u8]);

impl<'a> Payload<'a> {
    /// Create a new payload view from a slice.
    #[inline]
    pub fn new(data: &'a [u8]) -> Self {
        Self(data)
    }

    /// Get the underlying slice.
    #[inline]
    pub fn as_slice(&self) -> &'a [u8] {
        self.0
    }

    /// Get the length of the payload.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the payload is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Debug assertion that all bytes are valid nibbles (≤ 0x0F).
    ///
    /// This is only checked in debug builds to ensure data integrity
    /// without runtime overhead in release builds.
    #[inline]
    pub fn assert_nibbles(&self) {
        debug_assert!(
            self.0.iter().all(|&b| b <= 0x0F),
            "Invalid nibble values in payload: {:?}",
            self.0
        );
    }
}

/// Exact-length nibble view with compile-time size guarantee.
///
/// This type provides strongly-typed access to fixed-size payloads,
/// converting runtime length checks to compile-time guarantees where possible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nibbles<'a, const N: usize>(&'a [u8; N]);

impl<'a, const N: usize> Nibbles<'a, N> {
    /// Create from an exact-size array reference.
    #[inline]
    pub fn from_array(data: &'a [u8; N]) -> Self {
        Self(data)
    }

    /// Get the underlying array.
    #[inline]
    pub fn as_array(&self) -> &'a [u8; N] {
        self.0
    }

    /// Get a single byte at index.
    #[inline]
    pub fn byte(&self, index: usize) -> u8 {
        self.0[index]
    }

    /// Combine two nibbles into a u8 value.
    ///
    /// Takes bytes at positions [start] and [start+1] and combines them as:
    /// (byte[start] << 4) | byte[start+1]
    #[inline]
    pub fn u8_pair(&self, start: usize) -> u8 {
        debug_assert!(start + 1 < N, "u8_pair index out of bounds");
        (self.0[start] << 4) | self.0[start + 1]
    }

    /// Combine four nibbles into a u16 value.
    ///
    /// Takes bytes at positions [start..start+4] and combines them as:
    /// (byte[0] << 12) | (byte[1] << 8) | (byte[2] << 4) | byte[3]
    #[inline]
    pub fn u16_quad(&self, start: usize) -> u16 {
        debug_assert!(start + 3 < N, "u16_quad index out of bounds");
        ((self.0[start] as u16) << 12)
            | ((self.0[start + 1] as u16) << 8)
            | ((self.0[start + 2] as u16) << 4)
            | (self.0[start + 3] as u16)
    }

    /// Combine four nibbles into an i16 value.
    #[inline]
    pub fn i16_quad(&self, start: usize) -> i16 {
        self.u16_quad(start) as i16
    }

    /// Get the last nibble in the array.
    #[inline]
    pub fn last_nibble(&self) -> u8 {
        self.0[N - 1]
    }
}

impl<'a, const N: usize> TryFrom<Payload<'a>> for Nibbles<'a, N> {
    type Error = Error;

    #[inline]
    fn try_from(payload: Payload<'a>) -> Result<Self, Self::Error> {
        let array_ref: &[u8; N] = payload
            .0
            .try_into()
            .map_err(|_| Error::InvalidResponseLength)?;
        Ok(Nibbles(array_ref))
    }
}

/// Variable-length nibble view for responses that support multiple formats.
///
/// Used for responses like Zoom and PanTilt that can return either 4 or 8 nibbles
/// depending on the camera model and query context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nibbles4Or8<'a> {
    /// 4-nibble response format.
    N4(Nibbles<'a, 4>),
    /// 8-nibble extended response format.
    N8(Nibbles<'a, 8>),
}

impl<'a> Nibbles4Or8<'a> {
    /// Get the u16 value from the first 4 nibbles.
    ///
    /// Works for both 4 and 8 nibble variants, extracting from positions 0..4.
    #[inline]
    pub fn first_u16(&self) -> u16 {
        match self {
            Self::N4(n) => n.u16_quad(0),
            Self::N8(n) => n.u16_quad(0),
        }
    }

    /// Get the u16 value from nibbles at the specified position.
    ///
    /// Returns None if the position would exceed the available nibbles.
    #[inline]
    pub fn u16_at(&self, start: usize) -> Option<u16> {
        match self {
            Self::N4(n) if start + 3 < 4 => Some(n.u16_quad(start)),
            Self::N8(n) if start + 3 < 8 => Some(n.u16_quad(start)),
            _ => None,
        }
    }
}

impl<'a> TryFrom<Payload<'a>> for Nibbles4Or8<'a> {
    type Error = Error;

    fn try_from(payload: Payload<'a>) -> Result<Self, Self::Error> {
        match payload.len() {
            4 => Ok(Self::N4(Nibbles::<4>::try_from(payload)?)),
            8 => Ok(Self::N8(Nibbles::<8>::try_from(payload)?)),
            _ => Err(Error::InvalidResponseLength),
        }
    }
}

/// 6-nibble view for responses that use this specific format.
pub type Nibbles6<'a> = Nibbles<'a, 6>;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_payload_basic() {
        let data = vec![0x01, 0x02, 0x03, 0x04];
        let payload = Payload::new(&data);
        assert_eq!(payload.len(), 4);
        assert!(!payload.is_empty());
        assert_eq!(payload.as_slice(), &data);
    }

    #[test]
    fn test_nibbles_4() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let nibbles = Nibbles::<4>::from_array(&data);
        assert_eq!(nibbles.byte(0), 0x01);
        assert_eq!(nibbles.byte(3), 0x04);
        assert_eq!(nibbles.u8_pair(0), 0x12);
        assert_eq!(nibbles.u8_pair(2), 0x34);
        assert_eq!(nibbles.u16_quad(0), 0x1234);
        assert_eq!(nibbles.last_nibble(), 0x04);
    }

    #[test]
    fn test_nibbles_conversion() {
        let data = vec![0x0A, 0x0B, 0x0C, 0x0D];
        let payload = Payload::new(&data);
        let nibbles = Nibbles::<4>::try_from(payload).expect("Should convert to Nibbles<4>");
        assert_eq!(nibbles.u16_quad(0), 0xABCD);
    }

    #[test]
    fn test_nibbles_wrong_size() {
        let data = vec![0x01, 0x02, 0x03];
        let payload = Payload::new(&data);
        let result = Nibbles::<4>::try_from(payload);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));
    }

    #[test]
    fn test_nibbles_4_or_8() {
        // Test 4-nibble variant
        let data4 = vec![0x01, 0x02, 0x03, 0x04];
        let payload4 = Payload::new(&data4);
        let nibbles4 = Nibbles4Or8::try_from(payload4).expect("Should convert to Nibbles4Or8");
        assert!(matches!(nibbles4, Nibbles4Or8::N4(_)));
        assert_eq!(nibbles4.first_u16(), 0x1234);

        // Test 8-nibble variant
        let data8 = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let payload8 = Payload::new(&data8);
        let nibbles8 = Nibbles4Or8::try_from(payload8).expect("Should convert to Nibbles4Or8");
        assert!(matches!(nibbles8, Nibbles4Or8::N8(_)));
        assert_eq!(nibbles8.first_u16(), 0x1234);
        assert_eq!(nibbles8.u16_at(4), Some(0x5678));

        // Test invalid size
        let data_invalid = vec![0x01, 0x02];
        let payload_invalid = Payload::new(&data_invalid);
        let result = Nibbles4Or8::try_from(payload_invalid);
        assert!(matches!(result, Err(Error::InvalidResponseLength)));
    }

    #[test]
    fn test_i16_conversion() {
        let data = [0x0F, 0x0F, 0x0F, 0x0F];
        let nibbles = Nibbles::<4>::from_array(&data);
        assert_eq!(nibbles.i16_quad(0), -1);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Invalid nibble values")]
    fn test_debug_assert_nibbles() {
        let data = vec![0x10, 0x02]; // 0x10 is not a valid nibble
        let payload = Payload::new(&data);
        payload.assert_nibbles();
    }
}
