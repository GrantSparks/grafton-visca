//! Zero-allocation parameter handling for VISCA command macros.
//!
//! This module provides stack-based parameter encoding to eliminate
//! heap allocations in the VISCA command encoding hot path.
//!
//! # Design Philosophy
//!
//! Parameter encoding follows a "validated-at-construction" pattern:
//! - Fixed-size types (`u8`, `[u8; N]`) are infallible to encode
//! - Variable-length data must be validated via checked constructors
//!   that return `Result<ParamBuf, Error>` instead of panicking
//!
//! This design eliminates release-mode panics and aligns with the crate's
//! type-driven validation approach (like `visca_range_type!`).

use crate::error::Error;
use std::borrow::Cow;

/// Stack-backed parameter buffer for zero-allocation encoding.
///
/// This type holds parameter bytes on the stack, avoiding heap allocation
/// during command encoding. The const generic N specifies the maximum size.
///
/// # Construction
///
/// For fixed-size data, use `IntoParamBuf` implementations directly:
/// ```ignore
/// let buf: ParamBuf<4> = [0x01, 0x02, 0x03, 0x04].encode_param();
/// ```
///
/// For variable-length data, use the checked constructor:
/// ```ignore
/// let buf = ParamBuf::<8>::try_from_slice(&data)?;
/// ```
#[derive(Debug, Clone, Copy)]
pub struct ParamBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> ParamBuf<N> {
    /// Create an empty ParamBuf.
    #[inline]
    #[allow(dead_code)] // Utility method for future use
    pub const fn empty() -> Self {
        Self {
            buf: [0u8; N],
            len: 0,
        }
    }

    /// Try to create a ParamBuf from a slice.
    ///
    /// Returns `Error::InvalidParameter` if the slice length exceeds N.
    ///
    /// # Errors
    ///
    /// Returns an error if `src.len() > N`.
    #[inline]
    pub fn try_from_slice(src: &[u8]) -> Result<Self, Error> {
        if src.len() > N {
            return Err(Error::InvalidParameter {
                parameter: "param_buf",
                value: Cow::Owned(format!("slice of length {}", src.len())),
                reason: Cow::Owned(format!(
                    "length {} exceeds maximum buffer size {}",
                    src.len(),
                    N
                )),
            });
        }
        let mut buf = [0u8; N];
        buf[..src.len()].copy_from_slice(src);
        Ok(Self {
            buf,
            len: src.len(),
        })
    }

    /// Get the parameter bytes as a slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }

    /// Get the length of the parameter bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the buffer is empty.
    #[inline]
    #[allow(dead_code)] // Utility method for future use
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<const N: usize> TryFrom<&[u8]> for ParamBuf<N> {
    type Error = Error;

    #[inline]
    fn try_from(src: &[u8]) -> Result<Self, Self::Error> {
        Self::try_from_slice(src)
    }
}

impl<const N: usize> TryFrom<Vec<u8>> for ParamBuf<N> {
    type Error = Error;

    #[inline]
    fn try_from(src: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from_slice(&src)
    }
}

/// Trait for converting values to ParamBuf without heap allocation.
///
/// This trait is implemented only for types that cannot violate invariants
/// at runtime, ensuring infallible encoding:
/// - `u8`: Single byte, always fits in any buffer
/// - `[u8; N]`: Fixed-size array, size is known at compile time
/// - `ParamBuf<N>`: Already validated, identity conversion
///
/// For variable-length data, use the checked `ParamBuf::try_from_slice`
/// or `TryFrom` implementations instead.
pub trait IntoParamBuf<const N: usize> {
    /// Encode the value into a ParamBuf.
    fn encode_param(self) -> ParamBuf<N>;
}

// Implementation for single byte parameters
impl IntoParamBuf<1> for u8 {
    #[inline]
    fn encode_param(self) -> ParamBuf<1> {
        ParamBuf {
            buf: [self],
            len: 1,
        }
    }
}

// Implementation for fixed-size arrays
impl<const N: usize> IntoParamBuf<N> for [u8; N] {
    #[inline]
    fn encode_param(self) -> ParamBuf<N> {
        ParamBuf { buf: self, len: N }
    }
}

// Implementation for ParamBuf itself - identity conversion
// This allows commands to store a pre-validated ParamBuf and use it directly
impl<const N: usize> IntoParamBuf<N> for ParamBuf<N> {
    #[inline]
    fn encode_param(self) -> ParamBuf<N> {
        self
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parambuf_try_from_slice_valid() {
        let data = [0x01, 0x02, 0x03];
        let buf = ParamBuf::<4>::try_from_slice(&data).expect("should create from slice");
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.as_slice(), &[0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_parambuf_try_from_slice_exact_size() {
        let data = [0x01, 0x02, 0x03, 0x04];
        let buf = ParamBuf::<4>::try_from_slice(&data).expect("should create from slice");
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.as_slice(), &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parambuf_try_from_slice_oversize() {
        let data = [0x01, 0x02, 0x03, 0x04, 0x05];
        let buf = ParamBuf::<4>::try_from_slice(&data);
        assert!(buf.is_err());
        assert!(matches!(buf, Err(Error::InvalidParameter { .. })));
    }

    #[test]
    fn test_parambuf_try_from_vec() {
        let data = vec![0x01, 0x02, 0x03];
        let buf: ParamBuf<4> = data.try_into().expect("should create from vec");
        assert_eq!(buf.len(), 3);
    }

    #[test]
    fn test_parambuf_try_from_vec_oversize() {
        let data = vec![0x01, 0x02, 0x03, 0x04, 0x05];
        let buf: Result<ParamBuf<4>, _> = data.try_into();
        assert!(buf.is_err());
    }

    #[test]
    fn test_parambuf_empty() {
        let buf = ParamBuf::<4>::empty();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        let empty: &[u8] = &[];
        assert_eq!(buf.as_slice(), empty);
    }

    #[test]
    fn test_u8_encode_param() {
        let buf: ParamBuf<1> = 0x42u8.encode_param();
        assert_eq!(buf.len(), 1);
        assert_eq!(buf.as_slice(), &[0x42]);
    }

    #[test]
    fn test_array_encode_param() {
        let buf: ParamBuf<4> = [0x01, 0x02, 0x03, 0x04].encode_param();
        assert_eq!(buf.len(), 4);
        assert_eq!(buf.as_slice(), &[0x01, 0x02, 0x03, 0x04]);
    }

    #[test]
    fn test_parambuf_encode_param_identity() {
        let original =
            ParamBuf::<4>::try_from_slice(&[0x01, 0x02]).expect("should create from slice");
        let buf: ParamBuf<4> = original.encode_param();
        assert_eq!(buf.len(), 2);
        assert_eq!(buf.as_slice(), &[0x01, 0x02]);
    }
}
