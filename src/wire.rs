//! Wire representations of checked public values.

/// Converts a checked value to and from its canonical wire representation.
///
/// A value's own domain is checked here. Camera-specific restrictions and
/// profile-selected command encodings are checked during request preparation.
pub trait WireValue: Sized {
    /// The scalar or tuple carried by the wire representation.
    type Wire;

    /// Returns this value's canonical wire representation.
    fn to_wire(self) -> Self::Wire;

    /// Checks and decodes a wire representation.
    ///
    /// # Errors
    /// Returns an error when the representation is outside the value's domain.
    fn from_wire(value: Self::Wire) -> crate::Result<Self>;
}

macro_rules! identity {
    ($($ty:ty),* $(,)?) => {$(
        impl WireValue for $ty {
            type Wire = Self;

            fn to_wire(self) -> Self::Wire {
                self
            }

            fn from_wire(value: Self::Wire) -> crate::Result<Self> {
                Ok(value)
            }
        }
    )*};
}

identity!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, bool);

pub use crate::command::{
    bytes::VISCA_TERMINATOR,
    encode::CommandKind,
    response::{
        parse_inquiry_payload, BoolConvention, Nibbles, Payload, RawInquiryPayload, Response,
    },
    typed::ResponseParser,
};
