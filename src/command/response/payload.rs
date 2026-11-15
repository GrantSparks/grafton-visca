//! Type-safe payload views for VISCA inquiry response decoding.
//!
//! This module provides const-generic, zero-cost views over VISCA payload bytes
//! to eliminate manual slicing errors and provide compile-time guarantees about
//! payload shape and size.

use crate::error::Error;

/// VISCA boolean encoding convention.
///
/// VISCA protocol uses `0x02` and `0x03` to encode boolean states, but the
/// semantic meaning varies by command. This enum makes the convention
/// explicit at each call site, preventing silent polarity bugs.
///
/// # Protocol Background
///
/// Standard VISCA on/off replies use `0x02 = on` and `0x03 = off`.
/// A register that documents the opposite polarity must explicitly select
/// [`Self::OnIs03`]. The inquiry's source determines the convention.
///
/// # Examples
///
/// ```rust
/// use grafton_visca::command::{BoolConvention, Payload};
///
/// let data = [0x02];
/// let payload = Payload::new(&data);
///
/// // Standard convention: 0x02 = on
/// let powered = payload.parse_bool("digital_zoom", BoolConvention::OnIs02)?;
/// assert!(powered);
///
/// // Use the opposite convention only for a register that documents it.
/// let enabled = payload.parse_bool("vendor_setting", BoolConvention::OnIs03)?;
/// assert!(!enabled);
/// # Ok::<(), grafton_visca::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolConvention {
    /// Register-specific opposite polarity: `0x02` = off/false, `0x03` = on/true.
    /// Select this only when the register's source documents that meaning.
    OnIs03,
    /// Standard VISCA on/off polarity: `0x02` = on/true, `0x03` = off/false.
    ///
    /// Used by sourced replies including:
    /// - MenuOpenClose (0x02 = open)
    /// - TallyGreen (0x02 = on)
    /// - Power (0x02 = on)
    /// - UsbAudio (0x02 = on)
    OnIs02,
}

/// Zero-copy view of unvalidated VISCA reply payload bytes.
///
/// [`Self::new`] retains raw bytes without checking their shape or nibble values.
/// [`Self::assert_nibbles`] validates the values; conversion to [`Nibbles`]
/// additionally establishes the fixed length without copying the bytes.
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

    /// Validates that every payload byte is a nibble in every build.
    ///
    /// # Errors
    /// Returns [`Error::InvalidResponseFormat`] if a byte exceeds `0x0F`.
    #[inline]
    pub fn assert_nibbles(&self) -> Result<(), Error> {
        if self.0.iter().all(|&byte| byte <= 0x0F) {
            Ok(())
        } else {
            Err(Error::InvalidResponseFormat)
        }
    }

    /// Parse a single-byte boolean response with explicit convention.
    ///
    /// VISCA uses `0x02` and `0x03` for boolean states. The semantic meaning
    /// depends on the register: standard VISCA on/off replies use `0x02=on`.
    /// Select the opposite convention only when that register's source documents
    /// it. This method requires the convention explicitly at each call.
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
    /// - The byte is not `0x02` or `0x03` (`InvalidResponse`, the variant
    ///   every inquiry decoder uses for a value outside its code set)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use grafton_visca::command::{BoolConvention, Payload};
    /// let payload = Payload::new(&[0x02]);
    /// // Standard convention (0x02 = on)
    /// let powered = payload.parse_bool("digital_zoom", BoolConvention::OnIs02)?;
    ///
    /// // Opposite polarity, only for a register that documents it.
    /// let enabled = payload.parse_bool("vendor_setting", BoolConvention::OnIs03)?;
    /// assert!(powered && !enabled);
    /// # Ok::<(), grafton_visca::Error>(())
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
            _ => Err(Error::invalid_response(
                format!("{param_name}: 0x02 or 0x03"),
                vec![byte],
            )),
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
    /// Creates a validated view from an exact-size array reference.
    ///
    /// # Errors
    /// Returns [`Error::InvalidResponseFormat`] if a byte exceeds `0x0F`.
    #[inline]
    pub fn from_array(data: &'a [u8; N]) -> Result<Self, Error> {
        Self::try_from(Payload::new(data))
    }

    /// Get the underlying array.
    #[inline]
    pub fn as_array(&self) -> &'a [u8; N] {
        self.0
    }

    /// Gets a nibble, or `None` when `index` is out of bounds.
    #[inline]
    pub fn byte(&self, index: usize) -> Option<u8> {
        self.0.get(index).copied()
    }

    /// Combines four nibbles into a `u16`, or `None` if the range is out of bounds.
    #[inline]
    pub fn u16_quad(&self, start: usize) -> Option<u16> {
        let end = start.checked_add(4)?;
        self.0
            .get(start..end)
            .map(|nibbles| combine(nibbles) as u16)
    }

    /// Combines four nibbles into an `i16`, or `None` if the range is out of bounds.
    #[inline]
    pub fn i16_quad(&self, start: usize) -> Option<i16> {
        self.u16_quad(start).map(|value| value as i16)
    }

    /// Combines five nibbles, or `None` if the range is out of bounds.
    #[inline]
    pub(crate) fn u20_penta(&self, start: usize) -> Option<u32> {
        let end = start.checked_add(5)?;
        self.0.get(start..end).map(combine)
    }

    /// Combines a signed 20-bit value, or `None` if the range is out of bounds.
    #[inline]
    pub(crate) fn i20_penta(&self, start: usize) -> Option<i32> {
        let value = self.u20_penta(start)?;
        Some(if value & 0x0008_0000 != 0 {
            value as i32 - 0x0010_0000
        } else {
            value as i32
        })
    }

    /// The value of the trailing two nibbles of a zero-padded reply
    /// (`00 … 0p 0q` → `pq`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidResponseFormat`] when a leading padding nibble
    /// is not zero (the reply carries a value the field cannot hold) or the
    /// view is shorter than two nibbles.
    pub fn zero_extended_u8(&self) -> Result<u8, Error> {
        self.zero_extended(2)
    }

    /// The value of the trailing nibble of a zero-padded reply
    /// (`00 … 00 0p` → `p`).
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidResponseFormat`] when a leading padding nibble
    /// is not zero or the view is empty.
    pub fn zero_extended_nibble(&self) -> Result<u8, Error> {
        self.zero_extended(1)
    }

    /// Combines the trailing `digits` nibbles, requiring every leading nibble
    /// to be zero, as the reference's padded reply layouts do.
    fn zero_extended(&self, digits: usize) -> Result<u8, Error> {
        let Some(padding) = N.checked_sub(digits) else {
            return Err(Error::InvalidResponseFormat);
        };
        let (padding, value) = self.0.split_at(padding);
        if padding.iter().any(|&nibble| nibble != 0) {
            return Err(Error::InvalidResponseFormat);
        }
        Ok(combine(value) as u8)
    }
}

/// Combines nibble bytes, most significant first, masking each to its low
/// four bits. This is the decode side's one nibble combiner.
#[inline]
fn combine(nibbles: &[u8]) -> u32 {
    nibbles
        .iter()
        .fold(0, |value, &nibble| (value << 4) | u32::from(nibble & 0x0F))
}

impl<'a, const N: usize> TryFrom<Payload<'a>> for Nibbles<'a, N> {
    type Error = Error;

    #[inline]
    fn try_from(payload: Payload<'a>) -> Result<Self, Self::Error> {
        let array_ref: &[u8; N] = payload
            .0
            .try_into()
            .map_err(|_| Error::invalid_response_length(N, payload.0))?;

        payload.assert_nibbles()?;

        Ok(Nibbles(array_ref))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    proptest::proptest! {
        #[test]
        fn indexed_nibble_access_is_total_for_arbitrary_indices(index in proptest::prelude::any::<usize>()) {
            let data = [0, 1, 2, 3, 4, 5, 6, 7];
            let nibbles = Nibbles::from_array(&data).expect("valid nibbles");
            proptest::prop_assert_eq!(nibbles.byte(index), data.get(index).copied());
            let expected = index.checked_add(4).and_then(|end| data.get(index..end)).map(|bytes| combine(bytes) as u16);
            proptest::prop_assert_eq!(nibbles.u16_quad(index), expected);
            proptest::prop_assert_eq!(nibbles.i16_quad(index), expected.map(|value| value as i16));
            let empty = Nibbles::<0>::from_array(&[]).expect("empty validated view");
            proptest::prop_assert_eq!(empty.byte(index), None);
            proptest::prop_assert_eq!(empty.u16_quad(index), None);
            proptest::prop_assert_eq!(empty.i16_quad(index), None);
        }
    }

    #[test]
    fn maximum_indices_and_short_views_never_overflow() {
        let data = [0; 8];
        let nibbles = Nibbles::from_array(&data).expect("valid nibbles");
        for index in [usize::MAX, usize::MAX - 1, usize::MAX - 3, 5, 8] {
            assert_eq!(nibbles.u16_quad(index), None);
            assert_eq!(nibbles.i16_quad(index), None);
            assert_eq!(nibbles.u20_penta(index), None);
            assert_eq!(nibbles.i20_penta(index), None);
        }
        let short = Nibbles::<3>::from_array(&[0, 1, 2]).expect("short validated view");
        assert_eq!(short.u16_quad(0), None);
    }

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
        let nibbles = Nibbles::<4>::from_array(&data).expect("valid nibbles");
        assert_eq!(nibbles.byte(0), Some(0x01));
        assert_eq!(nibbles.byte(3), Some(0x04));
        assert_eq!(nibbles.u16_quad(0), Some(0x1234));
    }

    #[test]
    fn test_nibbles_conversion() {
        let data = vec![0x0A, 0x0B, 0x0C, 0x0D];
        let payload = Payload::new(&data);
        let nibbles = Nibbles::<4>::try_from(payload).expect("Should convert to Nibbles<4>");
        assert_eq!(nibbles.u16_quad(0), Some(0xABCD));
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
    fn test_i16_conversion() {
        let data = [0x0F, 0x0F, 0x0F, 0x0F];
        let nibbles = Nibbles::<4>::from_array(&data).expect("valid nibbles");
        assert_eq!(nibbles.i16_quad(0), Some(-1));
    }

    #[test]
    fn array_and_payload_validation_are_fallible_in_every_build() {
        let data = [0x10, 0x02];
        assert!(matches!(
            Payload::new(&data).assert_nibbles(),
            Err(Error::InvalidResponseFormat)
        ));
        assert!(matches!(
            Nibbles::from_array(&data),
            Err(Error::InvalidResponseFormat)
        ));
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
        let data = [0x0F, 0x0F, 0x0F, 0x0F];
        let nibbles = Nibbles::<4>::from_array(&data).expect("valid nibbles");

        assert_eq!(nibbles.u16_quad(0), Some(0xFFFF)); // All nibbles are 0x0F
    }

    /// Issue #828 L1: a padded `00 … 0p 0q` reply whose padding is not zero
    /// carries a value its field cannot hold. The accessors used to drop the
    /// leading nibbles silently; they now reject them.
    #[test]
    fn zero_extended_accessors_reject_nonzero_padding() {
        let pair = Nibbles::<4>::from_array(&[0x00, 0x00, 0x0A, 0x05]).expect("valid nibbles");
        assert_eq!(pair.zero_extended_u8().unwrap(), 0xA5);
        let single = Nibbles::<4>::from_array(&[0x00, 0x00, 0x00, 0x0E]).expect("valid nibbles");
        assert_eq!(single.zero_extended_nibble().unwrap(), 0x0E);
        assert_eq!(
            Nibbles::<2>::from_array(&[0x01, 0x0F])
                .expect("valid nibbles")
                .zero_extended_u8()
                .unwrap(),
            0x1F
        );

        for padded in [[0x01, 0x00, 0x0A, 0x05], [0x00, 0x0F, 0x0A, 0x05]] {
            assert!(matches!(
                Nibbles::<4>::from_array(&padded)
                    .expect("valid nibbles")
                    .zero_extended_u8(),
                Err(Error::InvalidResponseFormat)
            ));
        }
        for padded in [[0x00, 0x00, 0x01, 0x0E], [0x08, 0x00, 0x00, 0x0E]] {
            assert!(matches!(
                Nibbles::<4>::from_array(&padded)
                    .expect("valid nibbles")
                    .zero_extended_nibble(),
                Err(Error::InvalidResponseFormat)
            ));
        }
        assert!(matches!(
            Nibbles::<1>::from_array(&[0x01])
                .expect("valid nibbles")
                .zero_extended_u8(),
            Err(Error::InvalidResponseFormat)
        ));
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
                let expected_u16 = ((value as u16) << 12)
                    | ((value as u16) << 8)
                    | ((value as u16) << 4)
                    | (value as u16);
                assert_eq!(nibbles.u16_quad(0), Some(expected_u16));
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
    fn test_parse_bool_register_specific_opposite_convention() {
        // Register-specific opposite convention: 0x02 = off, 0x03 = on
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
    fn test_parse_bool_standard_convention() {
        // Standard convention: 0x02 = on, 0x03 = off
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

            // A camera reply outside the code set is a protocol error, the
            // variant every inquiry decoder uses (#809).
            for convention in [BoolConvention::OnIs03, BoolConvention::OnIs02] {
                let result = payload.parse_bool("test_param", convention);
                assert!(
                    matches!(
                        &result,
                        Err(Error::InvalidResponse { actual, .. }) if actual == &[invalid_byte]
                    ),
                    "Expected InvalidResponse for byte 0x{invalid_byte:02X}: {result:?}",
                );
            }
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
            Err(Error::InvalidResponse { expected, .. }) => {
                assert!(expected.contains("autofocus_status"), "{expected}");
            }
            _ => panic!("Expected InvalidResponse error"),
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
