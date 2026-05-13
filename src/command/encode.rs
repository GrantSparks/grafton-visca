//! Unified trait for encoding VISCA commands.
//!
//! This module provides the `ViscaCommand` trait which unifies the previous
//! `Command` and `ViscaCommand` traits into a single interface with zero-allocation
//! encoding support.

use bytes::Bytes;
use smallvec::SmallVec;

use crate::{error::Error, timeout::CommandCategory, CameraId};

use super::{bytes::FixedCommandBytes, response::InquiryKind};

/// Command kind classification for VISCA protocol.
///
/// This enum distinguishes between command and inquiry messages,
/// which have different response patterns and encapsulation requirements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// A command that performs an action and returns ACK/Completion
    Command,
    /// An inquiry that retrieves data and returns a data response
    Inquiry,
}

/// Checks that a VISCA command buffer has the proper terminator.
///
/// This function ensures that commands are properly terminated with 0xFF,
/// a critical safety invariant for the VISCA protocol.
///
/// # Returns
///
/// Returns `Ok(())` if the buffer has valid terminator, or an error if not.
#[inline]
fn check_terminator(buffer: &[u8], len: usize) -> Result<(), Error> {
    if len > 0 && buffer[len - 1] != crate::command::bytes::VISCA_TERMINATOR {
        return Err(Error::InvalidRequest(
            format!(
                "VISCA command missing 0xFF terminator at position {pos}. Command bytes: {bytes:02X?}",
                pos = len - 1,
                bytes = &buffer[..len]
            )
            .into(),
        ));
    }
    Ok(())
}

/// Checks that a VISCA command buffer has valid structure.
///
/// This function ensures:
/// - Commands have proper terminator (0xFF)
/// - Commands have valid camera address byte (0x81-0x88)
/// - Commands have minimum required length
///
/// These are critical safety invariants for the VISCA protocol.
///
/// # Returns
///
/// Returns `Ok(())` if the buffer meets all requirements, or an error if not.
#[inline]
fn check_command_structure(buffer: &[u8], len: usize) -> Result<(), Error> {
    // Validate minimum length (at least address + terminator)
    if len < 2 {
        return Err(Error::InvalidRequest(
            format!(
                "VISCA command too short: {len} bytes. Minimum is 2 bytes. Command bytes: {bytes:02X?}",
                bytes = &buffer[..len]
            )
            .into(),
        ));
    }

    // Validate camera address byte (0x81-0x88 for cameras 1-8)
    if len > 0 && (buffer[0] < 0x81 || buffer[0] > 0x88) {
        return Err(Error::InvalidRequest(
            format!(
                "Invalid VISCA camera address byte: 0x{addr:02X}. Must be 0x81-0x88. Command bytes: {bytes:02X?}",
                addr = buffer[0],
                bytes = &buffer[..len]
            )
            .into(),
        ));
    }

    // Validate terminator
    check_terminator(buffer, len)?;
    Ok(())
}

/// Unified trait for all VISCA commands.
///
/// This trait combines the functionality of the previous `Command` and `ViscaCommand`
/// traits, providing both zero-allocation encoding and convenient heap-allocated methods.
///
/// # Example Implementation
/// ```ignore
/// # use grafton_visca::timeout::CommandCategory;
/// # use grafton_visca::CameraId;
/// # use grafton_visca::Error;
/// struct MyCommand;
///
/// impl ViscaCommand for MyCommand {
///     type Response = ();
///     const MAX_SIZE: usize = 6;
///     const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
///
///     fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
///         // Check buffer size
///         if buffer.len() < 6 {
///             return Err(Error::BufferTooSmall { required: 6, actual: buffer.len() });
///         }
///
///         // Write VISCA command bytes
///         buffer[0] = camera_id.to_address_byte();  // Dynamic camera ID
///         buffer[1] = 0x01;
///         buffer[2] = 0x04;
///         buffer[3] = 0x00;
///         buffer[4] = 0x02;
///         buffer[5] = 0xFF;
///         Ok(6)
///     }
///
///     fn response_kind(&self) -> Option<InquiryKind> {
///         // Return None for action commands, Some(...) for inquiries
///         None
///     }
/// }
/// ```
pub trait ViscaCommand: Send + Sync {
    /// The type of response expected from this command.
    type Response;

    /// Maximum size in bytes that this command can encode to.
    const MAX_SIZE: usize;

    /// The timeout category for this command.
    ///
    /// This constant determines the appropriate timeout duration for the command
    /// based on its expected execution time. Defaults to `CommandCategory::Custom`
    /// which uses the default timeout duration.
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Custom;

    /// Exact encoded byte length for *this instance* (default: MAX_SIZE).
    ///
    /// This method enables exact-size buffer allocation, avoiding over-allocation
    /// when the actual encoded size is smaller than MAX_SIZE. Commands with
    /// variable-length parameters should override this to return the precise size.
    #[inline]
    fn encoded_size(&self) -> usize {
        Self::MAX_SIZE
    }

    /// Writes the command into the provided buffer.
    ///
    /// This is the primary method for zero-allocation encoding. The buffer must
    /// be at least `MAX_SIZE` bytes. Returns the number of bytes written.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - The camera ID to address the command to
    /// * `buffer` - The buffer to write the command bytes into
    ///
    /// # Returns
    ///
    /// The number of bytes written to the buffer
    ///
    /// # Errors
    ///
    /// * `Error::BufferTooSmall` if the buffer is smaller than required
    /// * `Error::InvalidParameter` if the command contains invalid parameters
    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error>;

    /// Encodes the command to a fixed-size, length-aware buffer.
    ///
    /// This method provides stack-allocated encoding for compile-time known sizes,
    /// returning a [`FixedCommandBytes`] that carries both the bytes and the actual
    /// encoded length. This prevents accidental transmission of trailing bytes.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - The camera ID to address the command to
    ///
    /// # Returns
    ///
    /// A [`FixedCommandBytes<N>`] containing the encoded command. Use [`as_slice()`](FixedCommandBytes::as_slice)
    /// or [`AsRef<[u8]>`](AsRef) to access only the meaningful bytes.
    ///
    /// # Errors
    ///
    /// * `Error::BufferTooSmall` if N is smaller than the encoded size
    /// * `Error::InvalidParameter` if the command contains invalid parameters
    fn to_fixed_bytes<const N: usize>(
        &self,
        camera_id: CameraId,
    ) -> Result<FixedCommandBytes<N>, Error> {
        let mut buffer = [0u8; N];
        let size = self.write_into(camera_id, &mut buffer)?;
        if size > N {
            return Err(Error::BufferTooSmall {
                required: size,
                actual: N,
            });
        }

        // Validate command structure
        check_command_structure(&buffer, size)?;

        Ok(FixedCommandBytes::new(buffer, size))
    }

    /// Encodes the command to a `bytes::Bytes` buffer.
    ///
    /// This method provides zero-copy reference-counted buffers via `bytes::Bytes`,
    /// encoding into a BytesMut buffer, validating, and freezing the result. This is
    /// optimal for runtime usage where commands are sent through queues and retried,
    /// avoiding extra allocations and copies.
    ///
    /// Uses `encoded_size()` for exact-size allocation, avoiding over-allocation.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - The camera ID to address the command to
    ///
    /// # Errors
    ///
    /// * `Error::InvalidParameter` if the command contains invalid parameters
    /// * `Error::InvalidRequest` if command structure validation fails
    fn to_bytes(&self, camera_id: CameraId) -> Result<Bytes, Error> {
        let need = self.encoded_size();
        let mut buf = bytes::BytesMut::with_capacity(need);
        // Give write_into a full mutable slice
        buf.resize(need, 0);

        let len = self.write_into(camera_id, &mut buf)?;

        // Validate command structure before freezing
        check_command_structure(&buf, len)?;

        // Truncate to actual size and freeze
        buf.truncate(len);
        Ok(buf.freeze())
    }

    /// Returns the expected response kind for this command.
    ///
    /// - Returns `None` for action commands that only receive ACK/Completion
    /// - Returns `Some(InquiryKind::...)` for inquiry commands that receive data
    fn response_kind(&self) -> Option<InquiryKind>;

    /// Returns the command kind based on the response type.
    ///
    /// Commands with a response type are inquiries; others are commands.
    #[inline(always)]
    fn command_kind(&self) -> CommandKind {
        if self.response_kind().is_some() {
            CommandKind::Inquiry
        } else {
            CommandKind::Command
        }
    }
}

/// Inline buffer size for encoded commands - 24 bytes covers most commands without heap allocation.
///
/// Maximum VISCA command size is 15 bytes, so 24 bytes provides headroom for common cases.
pub(crate) const INLINE_COMMAND_SIZE: usize = 24;

/// Pre-encoded command that stores the VISCA bytes inline for zero-allocation sends.
///
/// This struct replaces `PreparedCommand` and uses `SmallVec` to store command bytes
/// inline on the stack for common command sizes, eliminating heap allocations in the
/// hot send path.
///
#[derive(Debug, Clone)]
pub(crate) struct EncodedCommand {
    /// The encoded VISCA bytes stored inline for zero-allocation.
    /// Uses SmallVec with inline capacity of 24 bytes (covers most commands).
    pub(crate) payload: SmallVec<[u8; INLINE_COMMAND_SIZE]>,
    /// The command kind (Command or Inquiry).
    pub(crate) kind: CommandKind,
    /// The timeout category for this command.
    pub(crate) category: CommandCategory,
    /// The expected response type for inquiry commands.
    pub(crate) response_type: Option<InquiryKind>,
}

impl EncodedCommand {
    /// Create a new EncodedCommand from a ViscaCommand implementation.
    ///
    /// This encodes the command once using `write_into` and stores the bytes
    /// inline for repeated zero-allocation sends.
    ///
    /// # Arguments
    ///
    /// * `cmd` - A reference to the command to encode
    /// * `camera_id` - The camera ID to address the command to
    ///
    /// # Errors
    ///
    /// Returns an error if encoding fails or command structure is invalid.
    ///
    /// # Note
    ///
    /// This method takes a reference to the command, eliminating the need for
    /// `Clone` bounds on command types and avoiding unnecessary deep copies
    /// (particularly important for heap-backed commands like `RawCommand`).
    pub(crate) fn new<C: ViscaCommand>(cmd: &C, camera_id: CameraId) -> Result<Self, Error> {
        // Allocate inline buffer sized for the command
        let size = cmd.encoded_size();
        let mut payload = SmallVec::with_capacity(size);

        // Resize to provide mutable slice for write_into
        payload.resize(size, 0);

        // Encode directly into the inline buffer
        let len = cmd.write_into(camera_id, &mut payload)?;

        // Validate command structure
        check_command_structure(&payload, len)?;

        // Truncate to actual size
        payload.truncate(len);

        // Extract metadata from the command
        let kind = cmd.command_kind();
        let category = C::TIMEOUT_CATEGORY;
        let response_type = cmd.response_kind();

        Ok(Self {
            payload,
            kind,
            category,
            response_type,
        })
    }

    /// Get the encoded command bytes as a slice.
    ///
    /// This provides zero-copy access to the inline buffer for framing operations.
    #[inline]
    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyInvalidAddr;

    impl ViscaCommand for DummyInvalidAddr {
        type Response = ();
        const MAX_SIZE: usize = 2;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn write_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x01; // Invalid address byte (must be 0x81..=0x88)
            buffer[1] = crate::command::bytes::VISCA_TERMINATOR;
            Ok(2)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }

    struct DummyTooShort;

    impl ViscaCommand for DummyTooShort {
        type Response = ();
        const MAX_SIZE: usize = 2;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = camera_id.to_address_byte();
            // Intentionally omit terminator and return len 1
            Ok(1)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }

    struct DummyMissingTerminator;

    impl ViscaCommand for DummyMissingTerminator {
        type Response = ();
        const MAX_SIZE: usize = 3;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = camera_id.to_address_byte();
            buffer[1] = 0x01;
            buffer[2] = 0x02; // Missing terminator
            Ok(3)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }

    #[test]
    fn to_fixed_bytes_validates_command_structure() {
        use crate::command::bytes::FixedCommandBytes;

        let cmd = DummyInvalidAddr;
        let result: Result<FixedCommandBytes<2>, Error> = cmd.to_fixed_bytes(CameraId::CAMERA_1);
        assert!(result.is_err(), "Expected error for invalid address");

        let cmd2 = DummyMissingTerminator;
        let result2: Result<FixedCommandBytes<3>, Error> = cmd2.to_fixed_bytes(CameraId::CAMERA_1);
        assert!(result2.is_err(), "Expected error for missing terminator");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn to_fixed_bytes_returns_correct_length() {
        // Create a valid command for testing
        struct DummyValid;

        impl ViscaCommand for DummyValid {
            type Response = ();
            const MAX_SIZE: usize = 6;
            const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

            fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
                buffer[0] = camera_id.to_address_byte();
                buffer[1] = 0x01;
                buffer[2] = 0x04;
                buffer[3] = 0x00;
                buffer[4] = crate::command::bytes::VISCA_TERMINATOR;
                Ok(5)
            }

            fn response_kind(&self) -> Option<InquiryKind> {
                None
            }
        }

        let cmd = DummyValid;
        let result = cmd.to_fixed_bytes::<8>(CameraId::CAMERA_1);
        assert!(result.is_ok());

        let fixed = result.unwrap();
        assert_eq!(fixed.len(), 5);
        assert_eq!(
            fixed.as_slice(),
            &[
                0x81,
                0x01,
                0x04,
                0x00,
                crate::command::bytes::VISCA_TERMINATOR
            ]
        );
        assert_eq!(
            fixed.as_slice().last(),
            Some(&crate::command::bytes::VISCA_TERMINATOR)
        );

        // Verify that the underlying array may be larger
        assert_eq!(fixed.as_array().len(), 8);
    }

    #[test]
    fn to_bytes_validates_command_structure() {
        let cmd = DummyTooShort;
        let result = cmd.to_bytes(CameraId::CAMERA_1);
        assert!(result.is_err(), "Expected error for too short command");
        assert!(
            matches!(result, Err(Error::InvalidRequest(ref msg)) if msg.contains("VISCA command too short")),
            "Expected InvalidRequest error with too short message, got: {:?}",
            result
        );
    }

    /// A non-Clone command type that holds owned data.
    ///
    /// This test type verifies that the API doesn't require Clone.
    /// We use a simple struct with no Clone derive to prove the point.
    /// The struct is naturally Send+Sync since it only contains primitive data.
    struct NonCloneCommand {
        /// Some data field.
        value: u8,
    }

    // Note: NonCloneCommand does NOT derive Clone, proving that
    // EncodedCommand::new doesn't require Clone on the command type.

    impl ViscaCommand for NonCloneCommand {
        type Response = ();
        const MAX_SIZE: usize = 6;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = camera_id.to_address_byte();
            buffer[1] = 0x01;
            buffer[2] = 0x04;
            buffer[3] = self.value;
            buffer[4] = crate::command::bytes::VISCA_TERMINATOR;
            Ok(5)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn encoded_command_works_with_non_clone_types() {
        // Create a non-Clone command
        let cmd = NonCloneCommand { value: 42 };

        // This test proves that EncodedCommand::new accepts a reference
        // without requiring Clone. If Clone were required, this would
        // fail to compile since NonCloneCommand doesn't implement Clone.
        let result = EncodedCommand::new(&cmd, CameraId::CAMERA_1);
        assert!(result.is_ok(), "Should encode non-Clone command");

        let encoded = result.unwrap();
        assert_eq!(encoded.as_slice().len(), 5);
        assert_eq!(encoded.category, CommandCategory::Quick);

        // Verify the value was encoded
        assert_eq!(encoded.as_slice()[3], 42);
    }

    /// A command that wraps a vector (heap-allocated, non-Copy).
    ///
    /// This test type verifies that commands with heap-allocated data
    /// work correctly with the reference-based API.
    struct HeapCommand {
        /// Heap-allocated data. Send+Sync is derived automatically for Vec<u8>.
        data: Vec<u8>,
    }

    // Note: HeapCommand does NOT derive Clone, proving that
    // EncodedCommand::new doesn't require Clone on heap-backed commands.

    impl ViscaCommand for HeapCommand {
        type Response = ();
        const MAX_SIZE: usize = 32;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Custom;

        fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            let len = 2 + self.data.len() + 1;
            if len > buffer.len() {
                return Err(Error::InvalidRequest("Buffer too small".into()));
            }
            buffer[0] = camera_id.to_address_byte();
            buffer[1] = 0x01;
            buffer[2..2 + self.data.len()].copy_from_slice(&self.data);
            buffer[2 + self.data.len()] = crate::command::bytes::VISCA_TERMINATOR;
            Ok(len)
        }

        fn response_kind(&self) -> Option<InquiryKind> {
            None
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn encoded_command_works_with_heap_backed_data() {
        // Create a heap-backed command (Vec allocates on heap)
        let cmd = HeapCommand {
            data: vec![0x04, 0x00, 0x03],
        };

        // EncodedCommand::new should accept &cmd without requiring Clone.
        // This is important because cloning Vec<u8> would allocate.
        let result = EncodedCommand::new(&cmd, CameraId::CAMERA_1);
        assert!(result.is_ok(), "Should encode heap-backed command");

        let encoded = result.unwrap();
        // 1 (addr) + 1 (0x01) + 3 (data) + 1 (terminator) = 6
        assert_eq!(encoded.as_slice().len(), 6);

        // Verify the encoded bytes contain the data
        let bytes = encoded.as_slice();
        assert_eq!(bytes[0], 0x81); // Camera 1 address
        assert_eq!(bytes[1], 0x01);
        assert_eq!(&bytes[2..5], &[0x04, 0x00, 0x03]);
        assert_eq!(bytes[5], crate::command::bytes::VISCA_TERMINATOR);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn encoded_command_captures_data_not_reference() {
        // This test verifies that EncodedCommand captures the encoded bytes,
        // not a reference to the original command. This ensures the encoded
        // command remains valid even after the original command is dropped.

        let encoded = {
            let cmd = HeapCommand {
                data: vec![0x04, 0x00],
            };

            // Encode the command - this should capture the bytes
            EncodedCommand::new(&cmd, CameraId::CAMERA_1).unwrap()
            // `cmd` and its Vec are dropped here
        };

        // The encoded command should still be valid and contain the correct bytes
        assert_eq!(encoded.as_slice().len(), 5);
        let bytes = encoded.as_slice();
        assert_eq!(bytes[0], 0x81); // Camera 1 address
        assert_eq!(bytes[1], 0x01);
        assert_eq!(bytes[2], 0x04);
        assert_eq!(bytes[3], 0x00);
        assert_eq!(bytes[4], crate::command::bytes::VISCA_TERMINATOR);
    }
}
