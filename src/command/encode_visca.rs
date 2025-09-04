//! Unified trait for encoding VISCA commands.
//!
//! This module provides the `ViscaEncode` trait which unifies the previous
//! `Command` and `ViscaCommand` traits into a single interface with zero-allocation
//! encoding support.

use super::response::ViscaResponseType;
use crate::{
    camera_id::CameraId, constants::CameraVariant, error::Error, timeout::CommandCategory,
};

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
                "VISCA command missing 0xFF terminator at position {}. Command bytes: {:02X?}",
                len - 1,
                &buffer[..len]
            )
            .into(),
        ));
    }
    Ok(())
}

/// Legacy validation function for backward compatibility.
/// Retained temporarily for migration purposes.
#[inline]
pub fn validate_terminator(buffer: &[u8], len: usize) {
    assert!(
        len == 0 || buffer[len - 1] == crate::command::bytes::VISCA_TERMINATOR,
        "VISCA command missing 0xFF terminator at position {pos}. Command bytes: {bytes:02X?}",
        pos = len - 1,
        bytes = &buffer[..len]
    );
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
                "VISCA command too short: {} bytes. Minimum is 2 bytes. Command bytes: {:02X?}",
                len,
                &buffer[..len]
            )
            .into(),
        ));
    }

    // Validate camera address byte (0x81-0x88 for cameras 1-8)
    if len > 0 && (buffer[0] < 0x81 || buffer[0] > 0x88) {
        return Err(Error::InvalidRequest(
            format!(
                "Invalid VISCA camera address byte: 0x{:02X}. Must be 0x81-0x88. Command bytes: {:02X?}",
                buffer[0],
                &buffer[..len]
            )
            .into(),
        ));
    }

    // Validate terminator
    check_terminator(buffer, len)?;
    Ok(())
}

/// Legacy validation function for backward compatibility.
/// Retained temporarily for migration purposes.
#[inline]
pub fn validate_command_structure(buffer: &[u8], len: usize) {
    // Validate minimum length (at least address + terminator)
    assert!(
        len >= 2,
        "VISCA command too short: {len} bytes. Minimum is 2 bytes. Command bytes: {bytes:02X?}",
        len = len,
        bytes = &buffer[..len]
    );

    // Validate camera address byte (0x81-0x88 for cameras 1-8)
    if len > 0 {
        assert!(
            buffer[0] >= 0x81 && buffer[0] <= 0x88,
            "Invalid VISCA camera address byte: 0x{addr:02X}. Must be 0x81-0x88. Command bytes: {bytes:02X?}",
            addr = buffer[0],
            bytes = &buffer[..len]
        );
    }

    // Validate terminator
    validate_terminator(buffer, len);
}

/// Unified trait for all VISCA commands.
///
/// This trait combines the functionality of the previous `Command` and `ViscaCommand`
/// traits, providing both zero-allocation encoding and convenient heap-allocated methods.
///
/// # Example Implementation
/// ```ignore
/// // Internal trait - not part of public API
/// # use grafton_visca::timeout::CommandCategory;
/// # use grafton_visca::camera_id::CameraId;
/// # use grafton_visca::Error;
/// struct MyCommand;
///
/// impl ViscaEncode for MyCommand {
///     type ViscaResponse = ();
///     const MAX_SIZE: usize = 6;
///     const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;
///
///     fn encode_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
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
///     fn response_type(&self) -> Option<ViscaResponseType> {
///         // Return None for action commands, Some(...) for inquiries
///         None
///     }
/// }
/// ```
pub trait ViscaEncode: Send + Sync {
    /// The type of response expected from this command.
    type ViscaResponse;

    /// Maximum size in bytes that this command can encode to.
    const MAX_SIZE: usize;

    /// The timeout category for this command.
    ///
    /// This constant determines the appropriate timeout duration for the command
    /// based on its expected execution time. Defaults to `CommandCategory::Custom`
    /// which uses the default timeout duration.
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Custom;

    /// Encodes the command into the provided buffer.
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
    fn encode_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error>;

    /// Encodes the command to a fixed-size array.
    ///
    /// This method provides stack-allocated encoding for compile-time known sizes.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - The camera ID to address the command to
    ///
    /// # Errors
    ///
    /// * `Error::BufferTooSmall` if N is smaller than the encoded size
    /// * `Error::InvalidParameter` if the command contains invalid parameters
    fn encode_array<const N: usize>(&self, camera_id: CameraId) -> Result<[u8; N], Error> {
        let mut buffer = [0u8; N];
        let size = self.encode_into(camera_id, &mut buffer)?;
        if size > N {
            return Err(Error::BufferTooSmall {
                required: size,
                actual: N,
            });
        }

        // Validate command structure
        check_command_structure(&buffer, size)?;

        Ok(buffer)
    }

    /// Encodes the command to a heap-allocated vector.
    ///
    /// This is a convenience method that allocates a vector for the encoded bytes.
    /// Prefer `encode_into` or `encode_array` for performance-critical code.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - The camera ID to address the command to
    ///
    /// # Errors
    ///
    /// * `Error::InvalidParameter` if the command contains invalid parameters
    fn try_into_vec(&self, camera_id: CameraId) -> Result<Vec<u8>, Error> {
        let mut buffer = vec![0u8; Self::MAX_SIZE];
        let size = self.encode_into(camera_id, &mut buffer)?;

        // Validate full command structure before truncating (parity with encode_array)
        check_command_structure(&buffer, size)?;

        buffer.truncate(size);
        Ok(buffer)
    }

    /// Encodes the command to a `bytes::Bytes` buffer.
    ///
    /// This method provides zero-copy reference-counted buffers via `bytes::Bytes`,
    /// encoding into a stack buffer and wrapping the result. This is optimal for
    /// runtime usage where commands are sent through queues and retried.
    ///
    /// # Arguments
    ///
    /// * `camera_id` - The camera ID to address the command to
    ///
    /// # Errors
    ///
    /// * `Error::InvalidParameter` if the command contains invalid parameters
    fn try_into_bytes(&self, camera_id: CameraId) -> Result<bytes::Bytes, Error> {
        let mut stack_buffer = vec![0u8; Self::MAX_SIZE];
        let size = self.encode_into(camera_id, &mut stack_buffer)?;

        // Validate command structure before creating Bytes
        check_command_structure(&stack_buffer, size)?;

        Ok(bytes::Bytes::copy_from_slice(&stack_buffer[..size]))
    }

    /// Returns the expected response type for this command.
    ///
    /// - Returns `None` for action commands that only receive ACK/Completion
    /// - Returns `Some(ViscaResponseType::...)` for inquiry commands that receive data
    fn response_type(&self) -> Option<ViscaResponseType>;

    /// Returns the command category for timeout configuration.
    ///
    /// This is used to determine the appropriate timeout duration for the command.
    /// The default implementation returns the value of the `TIMEOUT_CATEGORY`
    /// associated constant.
    fn timeout_kind(&self) -> CommandCategory {
        Self::TIMEOUT_CATEGORY
    }

    /// Validate this command for a specific camera model.
    ///
    /// The default implementation returns `Ok(())` for backward compatibility.
    /// Commands should override this method to implement model-specific validation.
    ///
    /// # Errors
    ///
    /// Returns `Error::ModelValidation` if the command is not valid for the specified model.
    fn validate_for_model(&self, _model: CameraVariant) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyInvalidAddr;

    impl ViscaEncode for DummyInvalidAddr {
        type ViscaResponse = ();
        const MAX_SIZE: usize = 2;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn encode_into(&self, _camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = 0x01; // Invalid address byte (must be 0x81..=0x88)
            buffer[1] = crate::command::bytes::VISCA_TERMINATOR;
            Ok(2)
        }

        fn response_type(&self) -> Option<ViscaResponseType> {
            None
        }
    }

    struct DummyTooShort;

    impl ViscaEncode for DummyTooShort {
        type ViscaResponse = ();
        const MAX_SIZE: usize = 2;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn encode_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = camera_id.to_address_byte();
            // Intentionally omit terminator and return len 1
            Ok(1)
        }

        fn response_type(&self) -> Option<ViscaResponseType> {
            None
        }
    }

    struct DummyValid;

    impl ViscaEncode for DummyValid {
        type ViscaResponse = ();
        const MAX_SIZE: usize = 2;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn encode_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = camera_id.to_address_byte();
            buffer[1] = crate::command::bytes::VISCA_TERMINATOR;
            Ok(2)
        }

        fn response_type(&self) -> Option<ViscaResponseType> {
            None
        }
    }

    struct DummyMissingTerminator;

    impl ViscaEncode for DummyMissingTerminator {
        type ViscaResponse = ();
        const MAX_SIZE: usize = 3;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn encode_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            buffer[0] = camera_id.to_address_byte();
            buffer[1] = 0x01;
            buffer[2] = 0x02; // Missing terminator
            Ok(3)
        }

        fn response_type(&self) -> Option<ViscaResponseType> {
            None
        }
    }

    #[test]
    fn try_into_vec_validates_address_byte() {
        let cmd = DummyInvalidAddr;
        // Should return error due to invalid address byte validation
        let result = cmd.try_into_vec(CameraId::CAMERA_1);
        assert!(result.is_err(), "Expected error for invalid address byte");
        assert!(
            matches!(result, Err(Error::InvalidRequest(ref msg)) if msg.contains("Invalid VISCA camera address byte")),
            "Expected InvalidRequest error with address byte message, got: {:?}",
            result
        );
    }

    #[test]
    fn try_into_vec_validates_min_length_and_terminator() {
        let cmd = DummyTooShort;
        // Should return error due to too-short command (missing terminator)
        let result = cmd.try_into_vec(CameraId::CAMERA_1);
        assert!(result.is_err(), "Expected error for too-short command");
        assert!(
            matches!(result, Err(Error::InvalidRequest(ref msg)) if msg.contains("VISCA command too short")),
            "Expected InvalidRequest error with too short message, got: {:?}",
            result
        );
    }

    #[test]
    fn try_into_vec_succeeds_on_valid_command() {
        let cmd = DummyValid;
        let res = cmd.try_into_vec(CameraId::CAMERA_2);
        assert!(res.is_ok(), "try_into_vec should succeed, got: {res:?}");
        let v = res.unwrap_or_default();
        assert_eq!(v.len(), 2);
        assert!(v[0] >= 0x81 && v[0] <= 0x88);
        assert_eq!(v[1], crate::command::bytes::VISCA_TERMINATOR);
    }

    #[test]
    fn try_into_vec_validates_missing_terminator() {
        let cmd = DummyMissingTerminator;
        // Should return error due to missing terminator
        let result = cmd.try_into_vec(CameraId::CAMERA_1);
        assert!(result.is_err(), "Expected error for missing terminator");
        assert!(
            matches!(result, Err(Error::InvalidRequest(ref msg)) if msg.contains("missing 0xFF terminator")),
            "Expected InvalidRequest error with missing terminator message, got: {:?}",
            result
        );
    }

    #[test]
    fn encode_array_validates_command_structure() {
        let cmd = DummyInvalidAddr;
        let result: Result<[u8; 2], Error> = cmd.encode_array(CameraId::CAMERA_1);
        assert!(result.is_err(), "Expected error for invalid address");

        let cmd2 = DummyMissingTerminator;
        let result2: Result<[u8; 3], Error> = cmd2.encode_array(CameraId::CAMERA_1);
        assert!(result2.is_err(), "Expected error for missing terminator");
    }

    #[test]
    fn try_into_bytes_validates_command_structure() {
        let cmd = DummyTooShort;
        let result = cmd.try_into_bytes(CameraId::CAMERA_1);
        assert!(result.is_err(), "Expected error for too short command");
        assert!(
            matches!(result, Err(Error::InvalidRequest(ref msg)) if msg.contains("VISCA command too short")),
            "Expected InvalidRequest error with too short message, got: {:?}",
            result
        );
    }
}
