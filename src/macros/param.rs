//! Fixed-size, stack-backed operands for VISCA command macros.
//!
//! The operand type fixes its encoded length: a single byte or an array of
//! exactly `N` bytes. Encoding never allocates or validates a second time.

/// An encoded operand whose complete length is fixed by its type.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ParamBuf<const N: usize> {
    buf: [u8; N],
}

impl<const N: usize> ParamBuf<N> {
    /// Returns every encoded operand byte.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.buf
    }
}

/// Infallibly encodes a fixed-size operand without allocating.
pub(crate) trait IntoParamBuf<const N: usize> {
    /// Encodes the value into its complete operand buffer.
    fn encode_param(self) -> ParamBuf<N>;
}

impl IntoParamBuf<1> for u8 {
    #[inline]
    fn encode_param(self) -> ParamBuf<1> {
        ParamBuf { buf: [self] }
    }
}

impl<const N: usize> IntoParamBuf<N> for [u8; N] {
    #[inline]
    fn encode_param(self) -> ParamBuf<N> {
        ParamBuf { buf: self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_u8_encode_param() {
        let buf: ParamBuf<1> = 0x42u8.encode_param();
        assert_eq!(buf.as_slice().len(), 1);
        assert_eq!(buf.as_slice(), &[0x42]);
    }

    #[test]
    fn test_array_encode_param() {
        let buf: ParamBuf<4> = [0x01, 0x02, 0x03, 0x04].encode_param();
        assert_eq!(buf.as_slice().len(), 4);
        assert_eq!(buf.as_slice(), &[0x01, 0x02, 0x03, 0x04]);
    }
}
