//! Zero-allocation, compile-time command encoding for VISCA protocol.
//!
//! This module provides const command encoding to eliminate heap allocations
//! and enable compile-time validation of VISCA command byte sequences.

pub mod builder;
pub mod constants;

pub use builder::ConstCommandBuilder;

/// VISCA command terminator byte.
pub const VISCA_TERMINATOR: u8 = 0xFF;

/// Default camera ID for VISCA commands.
pub const DEFAULT_CAMERA_ID: u8 = 0x81;

/// Stack-allocated, length-aware command buffer.
///
/// This type provides a zero-allocation representation for encoded VISCA commands
/// that carries both the bytes and the actual encoded length. This prevents
/// accidental transmission of trailing bytes after the terminator.
///
/// # Design Rationale
///
/// Unlike returning a raw `[u8; N]` which loses length information, `FixedCommandBytes`
/// ensures that callers always have access to the correct slice via [`as_slice()`](Self::as_slice)
/// or [`AsRef<[u8]>`](AsRef). The underlying array may contain trailing zeros after
/// the encoded data, but these are never exposed through the safe API.
///
/// # Equality and Hashing
///
/// `FixedCommandBytes` implements `PartialEq`, `Eq`, and `Hash` based only on the
/// meaningful bytes (`&self[..len]`), not the full buffer capacity. This means two
/// `FixedCommandBytes` with different `N` but the same content compare as equal
/// when compared via slice, and hash identically.
///
/// # Examples
///
/// ```ignore
/// let cmd = MyCommand { value: 42 };
/// let encoded = cmd.to_fixed_bytes::<16>(CameraId::CAMERA_1)?;
///
/// // Safe access via as_slice() - only returns meaningful bytes
/// let slice = encoded.as_slice();
/// assert_eq!(slice.last(), Some(&0xFF)); // Properly terminated
///
/// // Works with APIs expecting &[u8]
/// transport.send(encoded.as_ref())?;
///
/// // Length is always available
/// println!("Command is {} bytes", encoded.len());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct FixedCommandBytes<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> FixedCommandBytes<N> {
    /// Creates a new `FixedCommandBytes` from a buffer and length.
    ///
    /// # Safety Invariant
    ///
    /// The caller must ensure that `len <= N` and that the bytes `[0..len]`
    /// contain a valid, terminated VISCA command.
    #[inline]
    pub(crate) const fn new(bytes: [u8; N], len: usize) -> Self {
        debug_assert!(len <= N);
        Self { bytes, len }
    }

    /// Returns the encoded command bytes as a slice.
    ///
    /// This returns only the meaningful bytes (up to the terminator),
    /// excluding any trailing zeros in the underlying buffer.
    #[inline]
    pub const fn as_slice(&self) -> &[u8] {
        // SAFETY: split_at panics if len > N, but our invariant guarantees len <= N.
        // However, const fn can't use &self.bytes[..self.len] directly, so we use
        // split_at which is const-compatible.
        self.bytes.split_at(self.len).0
    }

    /// Returns the length of the encoded command in bytes.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the command is empty (length 0).
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Consumes the wrapper and returns the underlying array.
    ///
    /// **Warning:** The returned array may contain trailing zeros after position
    /// `self.len()`. Prefer using [`as_slice()`](Self::as_slice) for transmission.
    #[inline]
    pub const fn into_array(self) -> [u8; N] {
        self.bytes
    }

    /// Returns a reference to the underlying array.
    ///
    /// **Warning:** The returned array may contain trailing zeros after position
    /// `self.len()`. Prefer using [`as_slice()`](Self::as_slice) for transmission.
    #[inline]
    pub const fn as_array(&self) -> &[u8; N] {
        &self.bytes
    }
}

impl<const N: usize> AsRef<[u8]> for FixedCommandBytes<N> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

impl<const N: usize> core::ops::Deref for FixedCommandBytes<N> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.bytes[..self.len]
    }
}

// Manual PartialEq implementation comparing only meaningful bytes
impl<const N: usize> PartialEq for FixedCommandBytes<N> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<const N: usize> Eq for FixedCommandBytes<N> {}

// Allow comparing FixedCommandBytes directly with byte slices
impl<const N: usize> PartialEq<[u8]> for FixedCommandBytes<N> {
    #[inline]
    fn eq(&self, other: &[u8]) -> bool {
        self.as_slice() == other
    }
}

impl<const N: usize> PartialEq<&[u8]> for FixedCommandBytes<N> {
    #[inline]
    fn eq(&self, other: &&[u8]) -> bool {
        self.as_slice() == *other
    }
}

// Hash only the meaningful bytes, not the full buffer
impl<const N: usize> core::hash::Hash for FixedCommandBytes<N> {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}

// Borrow as slice for use in HashMap/HashSet lookups
impl<const N: usize> core::borrow::Borrow<[u8]> for FixedCommandBytes<N> {
    #[inline]
    fn borrow(&self) -> &[u8] {
        self.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_command_bytes_as_slice() {
        let bytes = [0x81, 0x01, 0x04, 0x47, VISCA_TERMINATOR, 0x00, 0x00, 0x00];
        let fixed = FixedCommandBytes::new(bytes, 5);

        assert_eq!(
            fixed.as_slice(),
            &[0x81, 0x01, 0x04, 0x47, VISCA_TERMINATOR]
        );
        assert_eq!(fixed.len(), 5);
        assert!(!fixed.is_empty());
    }

    #[test]
    fn test_fixed_command_bytes_as_ref() {
        let bytes = [0x81, 0x01, VISCA_TERMINATOR, 0x00];
        let fixed = FixedCommandBytes::new(bytes, 3);

        let slice: &[u8] = fixed.as_ref();
        assert_eq!(slice, &[0x81, 0x01, VISCA_TERMINATOR]);
    }

    #[test]
    fn test_fixed_command_bytes_deref() {
        let bytes = [0x81, VISCA_TERMINATOR, 0x00, 0x00];
        let fixed = FixedCommandBytes::new(bytes, 2);

        // Test Deref trait
        assert_eq!(&*fixed, &[0x81, VISCA_TERMINATOR]);
        assert_eq!(fixed.last(), Some(&VISCA_TERMINATOR));
    }

    #[test]
    fn test_fixed_command_bytes_into_array() {
        let bytes = [0x81, 0x01, VISCA_TERMINATOR, 0x00];
        let fixed = FixedCommandBytes::new(bytes, 3);

        let array = fixed.into_array();
        assert_eq!(array, [0x81, 0x01, VISCA_TERMINATOR, 0x00]);
    }

    #[test]
    fn test_fixed_command_bytes_as_array() {
        let bytes = [0x81, 0x01, VISCA_TERMINATOR, 0x00];
        let fixed = FixedCommandBytes::new(bytes, 3);

        assert_eq!(fixed.as_array(), &[0x81, 0x01, VISCA_TERMINATOR, 0x00]);
    }

    #[test]
    fn test_fixed_command_bytes_terminates_correctly() {
        let bytes = [0x81, 0x01, 0x04, 0x47, VISCA_TERMINATOR, 0x00, 0x00, 0x00];
        let fixed = FixedCommandBytes::new(bytes, 5);

        // Verify terminator is accessible
        assert_eq!(fixed.as_slice().last(), Some(&VISCA_TERMINATOR));

        // Verify trailing zeros are not exposed
        assert!(!fixed.as_slice().contains(&0x00) || fixed.as_slice()[0..4].contains(&0x00));
    }

    #[test]
    fn test_fixed_command_bytes_equality_ignores_trailing_bytes() {
        // Two buffers with same meaningful content but different trailing bytes
        let bytes1 = [0x81, 0x01, VISCA_TERMINATOR, 0x00];
        let bytes2 = [0x81, 0x01, VISCA_TERMINATOR, 0xAB]; // Different trailing byte

        let fixed1 = FixedCommandBytes::new(bytes1, 3);
        let fixed2 = FixedCommandBytes::new(bytes2, 3);

        // Should be equal since only first 3 bytes matter
        assert_eq!(fixed1, fixed2);
    }

    #[test]
    fn test_fixed_command_bytes_partial_eq_with_slice() {
        let bytes = [0x81, 0x01, VISCA_TERMINATOR, 0x00];
        let fixed = FixedCommandBytes::new(bytes, 3);

        // Compare directly with slice via deref
        assert!(*fixed == [0x81, 0x01, VISCA_TERMINATOR]);
        assert!(fixed.as_slice() == [0x81, 0x01, VISCA_TERMINATOR]);

        // Should not equal different slice
        assert!(*fixed != [0x81, 0x01, 0x00]);
    }

    #[test]
    fn test_fixed_command_bytes_hash_consistency() {
        use core::hash::{Hash, Hasher};

        // Same content, different trailing bytes - should hash the same
        let bytes1 = [0x81, 0x01, VISCA_TERMINATOR, 0x00];
        let bytes2 = [0x81, 0x01, VISCA_TERMINATOR, 0xAB]; // Different trailing byte

        let fixed1 = FixedCommandBytes::new(bytes1, 3);
        let fixed2 = FixedCommandBytes::new(bytes2, 3);

        // Use a simple hasher to verify hash equality
        fn hash_it<H: Hash>(val: &H) -> u64 {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            val.hash(&mut hasher);
            hasher.finish()
        }

        assert_eq!(hash_it(&fixed1), hash_it(&fixed2));
    }

    #[test]
    fn test_fixed_command_bytes_borrow() {
        use core::borrow::Borrow;

        let bytes = [0x81, 0x01, VISCA_TERMINATOR, 0x00];
        let fixed = FixedCommandBytes::new(bytes, 3);

        // Can borrow as slice
        let slice: &[u8] = fixed.borrow();
        assert_eq!(slice, &[0x81, 0x01, VISCA_TERMINATOR]);
    }
}
