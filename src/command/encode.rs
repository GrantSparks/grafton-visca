//! Crate-private VISCA wire encoding primitives.
//!
//! Wire encoding deliberately knows only the size and write mechanics for a
//! VISCA frame. Request policy, completion semantics, routing, and timeout
//! selection belong to the typed request/preparation layers.

use crate::{error::Error, CameraId};

/// Protocol discriminator retained by transport framing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// A VISCA action command.
    Command,
    /// A VISCA inquiry.
    Inquiry,
}

/// The single crate-private wire encoder contract.
///
/// This trait intentionally exposes only frame mechanics. It is not a
/// command registry and carries no request class, timeout, retry, routing, or
/// completion metadata.
pub(crate) trait WireEncode: Send + Sync {
    /// Write one complete VISCA frame into `buffer`.
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error>;
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn wire_encoder_owns_only_size_and_write_mechanics() {
        struct Dummy;
        impl WireEncode for Dummy {
            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                let bytes = [
                    camera_id.to_address_byte(),
                    0x01,
                    0x04,
                    0x00,
                    crate::command::bytes::VISCA_TERMINATOR,
                ];
                if buffer.len() < bytes.len() {
                    return Err(Error::BufferTooSmall {
                        required: bytes.len(),
                        actual: buffer.len(),
                    });
                }
                buffer[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
        }

        let mut buffer = [0u8; 5];
        assert_eq!(
            Dummy.write_into(CameraId::CAMERA_1, &mut buffer).unwrap(),
            5
        );
        assert_eq!(&buffer, &[0x81, 0x01, 0x04, 0x00, 0xff]);
    }
}
