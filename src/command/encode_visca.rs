//! Unified trait for encoding VISCA commands.
//!
//! This module provides the `EncodeVisca` trait which unifies the previous
//! `Command` and `ViscaCommand` traits into a single interface with zero-allocation
//! encoding support.

use crate::{
    camera_id::CameraId, constants::CameraVariant, error::Error, timeout::CommandCategory,
};

use super::response::ResponseType;

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
/// impl EncodeVisca for MyCommand {
///     type Response = ();
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
///     fn response_type(&self) -> Option<ResponseType> {
///         // Return None for action commands, Some(...) for inquiries
///         None
///     }
/// }
/// ```
pub trait EncodeVisca: Send + Sync {
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
        buffer.truncate(size);
        Ok(buffer)
    }

    /// Returns the expected response type for this command.
    ///
    /// - Returns `None` for action commands that only receive ACK/Completion
    /// - Returns `Some(ResponseType::...)` for inquiry commands that receive data
    fn response_type(&self) -> Option<ResponseType>;

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
