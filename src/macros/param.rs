//! Zero-allocation parameter handling for VISCA command macros.
//!
//! This module provides stack-based parameter encoding to eliminate
//! heap allocations in the VISCA command encoding hot path.

/// Stack-backed parameter buffer for zero-allocation encoding.
///
/// This type holds parameter bytes on the stack, avoiding heap allocation
/// during command encoding. The const generic N specifies the maximum size.
#[derive(Debug, Clone, Copy)]
pub struct ParamBuf<const N: usize> {
    buf: [u8; N],
    len: usize,
}

impl<const N: usize> ParamBuf<N> {
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
}

/// Trait for converting values to ParamBuf without heap allocation.
///
/// This trait is implemented for common parameter types used in VISCA commands,
/// enabling zero-allocation encoding directly into stack buffers.
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

// Implementation for dynamic slices with runtime length
// Tuple of (slice, length) for cases where the actual length is known at runtime
impl<const N: usize> IntoParamBuf<N> for (&[u8], usize) {
    #[inline]
    fn encode_param(self) -> ParamBuf<N> {
        let (src, len) = self;
        debug_assert!(
            len <= N,
            "Parameter length {} exceeds buffer size {}",
            len,
            N
        );
        debug_assert!(
            len <= src.len(),
            "Specified length {} exceeds source slice length {}",
            len,
            src.len()
        );
        let mut buf = [0u8; N];
        buf[..len].copy_from_slice(&src[..len]);
        ParamBuf { buf, len }
    }
}

// Implementation for Vec<u8> to support existing code patterns
impl<const N: usize> IntoParamBuf<N> for Vec<u8> {
    #[inline]
    fn encode_param(self) -> ParamBuf<N> {
        debug_assert!(
            self.len() <= N,
            "Parameter length {} exceeds buffer size {}",
            self.len(),
            N
        );
        let mut buf = [0u8; N];
        let len = self.len();
        buf[..len].copy_from_slice(&self);
        ParamBuf { buf, len }
    }
}
