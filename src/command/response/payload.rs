//! Type-safe payload views for VISCA inquiry response decoding.
//!
//! This module provides const-generic, zero-cost views over VISCA payload bytes
//! to eliminate manual slicing errors and provide compile-time guarantees about
//! payload shape and size.

use std::borrow::Cow;

use crate::error::Error;

/// VISCA boolean encoding convention.
///
/// VISCA protocol uses `0x02` and `0x03` to encode boolean states, but the
/// semantic meaning varies by command. This enum makes the convention
/// explicit at each call site, preventing silent polarity bugs.
///
/// # Protocol Background
///
/// The VISCA protocol inherited an inconsistent boolean encoding from its
/// origins in Sony broadcast equipment. Most commands use `0x03 = on`,
/// but some legacy commands inverted this to `0x02 = on`.
///
/// # Examples
///
/// ```ignore
/// use grafton_visca::command::{BoolConvention, Payload};
///
/// let data = [0x02];
/// let payload = Payload::new(&data);
///
/// // Standard convention: 0x02 = off
/// let enabled = payload.parse_bool("autofocus", BoolConvention::OnIs03)?;
/// assert!(!enabled);
///
/// // Inverted convention: 0x02 = on
/// let is_open = payload.parse_bool("menu_status", BoolConvention::OnIs02)?;
/// assert!(is_open);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolConvention {
    /// Standard: `0x02` = off/false, `0x03` = on/true.
    ///
    /// Used by the majority of VISCA commands including:
    /// - AutoFocus, Standby, IrisControl, DefogMode, DigitalPtz
    /// - NightDayMode, NightDaySwitch, AutoTrace, FocusUnlock
    /// - Rtmp, Digital, TallyAutoAdjust
    /// - IrisUp, IrisDown, FocusNearFar
    /// - ZoomOut, ZoomIn, ZoomTeleWide
    OnIs03,
    /// Inverted: `0x02` = on/true, `0x03` = off/false.
    ///
    /// Used by a small number of legacy commands:
    /// - MenuOpenClose (0x02 = open)
    /// - TallyGreen (0x02 = on)
    /// - Power (0x02 = on)
    /// - UsbAudio (0x02 = on)
    OnIs02,
}

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

    /// Parse a single-byte boolean response with explicit convention.
    ///
    /// VISCA uses `0x02` and `0x03` for boolean states. The semantic meaning
    /// depends on the command—most use `0x03=on` (standard), but some legacy
    /// commands use `0x02=on` (inverted). This method requires the caller to
    /// specify the convention, making the polarity explicit.
    ///
    /// # Arguments
    ///
    /// * `param_name` - Parameter name for error messages (e.g., "autofocus_status")
    /// * `convention` - Which byte mapping to use (see [`BoolConvention`])
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The payload is not exactly one byte (`InvalidResponseLength`)
    /// - The byte is not `0x02` or `0x03` (`InvalidParameter`)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // Standard convention (0x03 = on)
    /// let enabled = payload.parse_bool("autofocus", BoolConvention::OnIs03)?;
    ///
    /// // Inverted convention (0x02 = on)
    /// let is_open = payload.parse_bool("menu_status", BoolConvention::OnIs02)?;
    /// ```
    pub fn parse_bool(
        &self,
        param_name: &'static str,
        convention: BoolConvention,
    ) -> Result<bool, Error> {
        if self.0.len() != 1 {
            return Err(Error::invalid_response_length(1, self.0));
        }
        let byte = self.0[0];
        match (byte, convention) {
            (0x02, BoolConvention::OnIs03) => Ok(false),
            (0x03, BoolConvention::OnIs03) => Ok(true),
            (0x02, BoolConvention::OnIs02) => Ok(true),
            (0x03, BoolConvention::OnIs02) => Ok(false),
            _ => Err(Error::InvalidParameter {
                parameter: param_name,
                value: Cow::Owned(format!("0x{byte:02X}")),
                reason: Cow::Borrowed("Expected 0x02 or 0x03"),
            }),
        }
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
    /// Takes bytes at positions \[start\] and \[start+1\] and combines them as:
    /// (byte\[start\] << 4) | byte\[start+1\]
    ///
    /// Each nibble is masked with 0x0F to ensure only the lower 4 bits are used.
    #[inline]
    pub fn u8_pair(&self, start: usize) -> u8 {
        debug_assert!(start + 1 < N, "u8_pair index out of bounds");
        ((self.0[start] & 0x0F) << 4) | (self.0[start + 1] & 0x0F)
    }

    /// Combine four nibbles into a u16 value.
    ///
    /// Takes bytes at positions \[start..start+4\] and combines them as:
    /// (byte\[0\] << 12) | (byte\[1\] << 8) | (byte\[2\] << 4) | byte\[3\]
    ///
    /// Each nibble is masked with 0x0F to ensure only the lower 4 bits are used.
    #[inline]
    pub fn u16_quad(&self, start: usize) -> u16 {
        debug_assert!(start + 3 < N, "u16_quad index out of bounds");
        (((self.0[start] & 0x0F) as u16) << 12)
            | (((self.0[start + 1] & 0x0F) as u16) << 8)
            | (((self.0[start + 2] & 0x0F) as u16) << 4)
            | ((self.0[start + 3] & 0x0F) as u16)
    }

    /// Combine four nibbles into an i16 value.
    #[inline]
    pub fn i16_quad(&self, start: usize) -> i16 {
        self.u16_quad(start) as i16
    }

    /// Combine five nibbles into a 20-bit unsigned value.
    #[inline]
    pub(crate) fn u20_penta(&self, start: usize) -> u32 {
        debug_assert!(start + 4 < N, "u20_penta index out of bounds");
        (((self.0[start] & 0x0F) as u32) << 16)
            | (((self.0[start + 1] & 0x0F) as u32) << 12)
            | (((self.0[start + 2] & 0x0F) as u32) << 8)
            | (((self.0[start + 3] & 0x0F) as u32) << 4)
            | ((self.0[start + 4] & 0x0F) as u32)
    }

    /// Combine five nibbles into a signed two's-complement 20-bit value.
    #[inline]
    pub(crate) fn i20_penta(&self, start: usize) -> i32 {
        let value = self.u20_penta(start);
        if value & 0x0008_0000 != 0 {
            value as i32 - 0x0010_0000
        } else {
            value as i32
        }
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
            .map_err(|_| Error::invalid_response_length(N, payload.0))?;

        // Validate that all bytes are valid nibbles (≤ 0x0F) in release builds too
        for &byte in array_ref {
            if byte > 0x0F {
                return Err(Error::InvalidResponseFormat);
            }
        }

        Ok(Nibbles(array_ref))
    }
}

/// Variable-length nibble view for responses that support multiple formats.
///
/// Used by the Zoom-position response parser, which supports both 4-nibble and
/// 8-nibble reply forms. Standard pan/tilt replies use an exact eight-nibble
/// [`Nibbles`] view instead.
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
            // Expected either 4 or 8 bytes; report as "4 or 8" expectation
            _ => Err(Error::InvalidResponseLength {
                expected: 4, // Primary expected size (can't express "4 or 8" with single usize)
                actual: payload.len(),
                payload_hex: {
                    // Use a descriptive message that captures the alternative expectation
                    let hex = payload
                        .0
                        .iter()
                        .take(32)
                        .map(|b| format!("{b:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ");
                    if payload.len() > 32 {
                        format!("{hex}... ({} bytes total, expected 4 or 8)", payload.len())
                            .into_boxed_str()
                    } else {
                        format!("{hex} (expected 4 or 8 bytes)").into_boxed_str()
                    }
                },
            }),
        }
    }
}

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
        assert!(matches!(
            result,
            Err(Error::InvalidResponseLength {
                expected: 4,
                actual: 3,
                ..
            })
        ));
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
        assert!(matches!(
            result,
            Err(Error::InvalidResponseLength {
                expected: 4,
                actual: 2,
                ..
            })
        ));
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

    #[test]
    fn test_nibbles_validation_in_release() {
        // Test that invalid nibbles are rejected in release builds
        let data = vec![0x1F, 0x02, 0x03, 0x04]; // 0x1F is not a valid nibble
        let payload = Payload::new(&data);
        let result = Nibbles::<4>::try_from(payload);
        assert!(matches!(result, Err(Error::InvalidResponseFormat)));

        // Test with high bits in multiple positions
        let data = vec![0x01, 0xFF, 0x03, 0x04]; // 0xFF is not a valid nibble
        let payload = Payload::new(&data);
        let result = Nibbles::<4>::try_from(payload);
        assert!(matches!(result, Err(Error::InvalidResponseFormat)));
    }

    #[test]
    fn test_nibbles_masking() {
        // Even if we somehow construct Nibbles with invalid data,
        // the u8_pair and u16_quad methods should mask the values
        let data = [0x0F, 0x0F, 0x0F, 0x0F];
        let nibbles = Nibbles::<4>::from_array(&data);

        // Test u8_pair masking
        assert_eq!(nibbles.u8_pair(0), 0xFF); // (0x0F << 4) | 0x0F = 0xFF
        assert_eq!(nibbles.u8_pair(2), 0xFF);

        // Test u16_quad masking
        assert_eq!(nibbles.u16_quad(0), 0xFFFF); // All nibbles are 0x0F
    }

    #[test]
    fn test_nibbles_with_valid_data() {
        // Test all valid nibble values (0x00 to 0x0F)
        for value in 0x00..=0x0F {
            let data = vec![value; 4];
            let payload = Payload::new(&data);
            let result = Nibbles::<4>::try_from(payload);
            assert!(
                result.is_ok(),
                "Failed for valid nibble value: 0x{:02X}",
                value
            );

            if let Ok(nibbles) = result {
                // Verify the values are correctly combined
                let expected_u8 = (value << 4) | value;
                let expected_u16 = ((value as u16) << 12)
                    | ((value as u16) << 8)
                    | ((value as u16) << 4)
                    | (value as u16);
                assert_eq!(nibbles.u8_pair(0), expected_u8);
                assert_eq!(nibbles.u16_quad(0), expected_u16);
            }
        }
    }

    #[test]
    fn test_nibbles_error_on_invalid_high_bits() {
        // Test that any value > 0x0F is rejected
        for value in 0x10..=0xFF {
            let data = vec![0x01, 0x02, value, 0x04];
            let payload = Payload::new(&data);
            let result = Nibbles::<4>::try_from(payload);
            assert!(
                matches!(result, Err(Error::InvalidResponseFormat)),
                "Should reject nibble value: 0x{:02X}",
                value
            );
        }
    }

    // =========================================================================
    // BoolConvention and parse_bool tests
    // =========================================================================

    #[test]
    fn test_parse_bool_standard_convention() {
        // Standard convention: 0x02 = off, 0x03 = on
        let data_off = vec![0x02];
        let payload_off = Payload::new(&data_off);
        assert!(!payload_off
            .parse_bool("test_param", BoolConvention::OnIs03)
            .unwrap());

        let data_on = vec![0x03];
        let payload_on = Payload::new(&data_on);
        assert!(payload_on
            .parse_bool("test_param", BoolConvention::OnIs03)
            .unwrap());
    }

    #[test]
    fn test_parse_bool_inverted_convention() {
        // Inverted convention: 0x02 = on, 0x03 = off
        let data_on = vec![0x02];
        let payload_on = Payload::new(&data_on);
        assert!(payload_on
            .parse_bool("test_param", BoolConvention::OnIs02)
            .unwrap());

        let data_off = vec![0x03];
        let payload_off = Payload::new(&data_off);
        assert!(!payload_off
            .parse_bool("test_param", BoolConvention::OnIs02)
            .unwrap());
    }

    #[test]
    fn test_parse_bool_invalid_byte() {
        use crate::command::bytes::VISCA_TERMINATOR;

        // Test invalid byte values (not 0x02 or 0x03)
        for invalid_byte in [0x00, 0x01, 0x04, 0x05, VISCA_TERMINATOR] {
            let data = vec![invalid_byte];
            let payload = Payload::new(&data);

            let result = payload.parse_bool("test_param", BoolConvention::OnIs03);
            assert!(
                matches!(result, Err(Error::InvalidParameter { .. })),
                "Expected InvalidParameter for byte 0x{:02X}",
                invalid_byte
            );

            let result = payload.parse_bool("test_param", BoolConvention::OnIs02);
            assert!(
                matches!(result, Err(Error::InvalidParameter { .. })),
                "Expected InvalidParameter for byte 0x{:02X}",
                invalid_byte
            );
        }
    }

    #[test]
    fn test_parse_bool_empty_payload() {
        let data: Vec<u8> = vec![];
        let payload = Payload::new(&data);

        let result = payload.parse_bool("test_param", BoolConvention::OnIs03);
        assert!(matches!(
            result,
            Err(Error::InvalidResponseLength {
                expected: 1,
                actual: 0,
                ..
            })
        ));
    }

    #[test]
    fn test_parse_bool_rejects_trailing_payload_bytes() {
        let data = [0x02, 0x00];
        let payload = Payload::new(&data);

        let result = payload.parse_bool("test_param", BoolConvention::OnIs03);

        assert!(matches!(
            result,
            Err(Error::InvalidResponseLength {
                expected: 1,
                actual: 2,
                ..
            })
        ));
    }

    #[test]
    fn test_parse_bool_error_message_contains_param_name() {
        let data = vec![0x00]; // Invalid byte
        let payload = Payload::new(&data);

        let result = payload.parse_bool("autofocus_status", BoolConvention::OnIs03);
        match result {
            Err(Error::InvalidParameter { parameter, .. }) => {
                assert_eq!(parameter, "autofocus_status");
            }
            _ => panic!("Expected InvalidParameter error"),
        }
    }

    #[test]
    fn test_bool_convention_equality() {
        assert_eq!(BoolConvention::OnIs03, BoolConvention::OnIs03);
        assert_eq!(BoolConvention::OnIs02, BoolConvention::OnIs02);
        assert_ne!(BoolConvention::OnIs03, BoolConvention::OnIs02);
    }

    #[test]
    fn test_bool_convention_debug() {
        assert_eq!(format!("{:?}", BoolConvention::OnIs03), "OnIs03");
        assert_eq!(format!("{:?}", BoolConvention::OnIs02), "OnIs02");
    }
}
