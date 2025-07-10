//! Updated VISCA command trait with zero-allocation support.

use crate::command::const_encoding::EncodingError;
use crate::command::response::ResponseType;
use crate::constants::CameraModel;
use crate::timeout::CommandCategory;
use crate::Error;

/// Trait for all VISCA commands with stack allocation support.
///
/// This trait provides both zero-allocation and heap-allocated encoding methods.
pub trait ViscaCommand: Send + Sync {
    /// Response type expected from this command.
    type Response;

    /// Maximum size of the encoded command.
    const MAX_SIZE: usize;

    /// Encode the command to a provided buffer.
    ///
    /// Returns the number of bytes written to the buffer.
    fn encode_to(&self, buffer: &mut [u8]) -> Result<usize, EncodingError>;

    /// Helper for fixed-size encoding on the stack.
    ///
    /// This method encodes to a stack-allocated array of the specified size.
    fn encode_array<const N: usize>(&self) -> Result<[u8; N], EncodingError> {
        let mut buffer = [0u8; N];
        let len = self.encode_to(&mut buffer)?;
        if len > N {
            return Err(EncodingError::BufferTooSmall);
        }
        Ok(buffer)
    }

    /// Legacy support - allocate on heap.
    ///
    /// This method maintains backward compatibility but allocates.
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buffer = vec![0u8; Self::MAX_SIZE];
        match self.encode_to(&mut buffer) {
            Ok(len) => {
                buffer.truncate(len);
                Ok(buffer)
            }
            Err(EncodingError::BufferTooSmall) => {
                Err(Error::InvalidParameter("Command too large".to_string()))
            }
            Err(EncodingError::InvalidParameter(msg)) => {
                Err(Error::InvalidParameter(msg.to_string()))
            }
        }
    }

    /// Get the expected response type for this command.
    fn response_type(&self) -> Option<ResponseType>;

    /// Get the command category for timeout configuration.
    fn command_category(&self) -> CommandCategory {
        CommandCategory::Custom
    }

    /// Validate this command for a specific camera model.
    fn validate_for_model(&self, _model: CameraModel) -> Result<(), Error> {
        Ok(())
    }
}

/// Marker trait for commands that can be encoded at compile time.
pub trait ConstEncodable: ViscaCommand {
    /// Encode the command as a const array.
    ///
    /// This method must be const-evaluable.
    fn const_encode<const N: usize>(&self) -> [u8; N];
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestCommand;

    impl ViscaCommand for TestCommand {
        type Response = ();
        const MAX_SIZE: usize = 6;

        fn encode_to(&self, buffer: &mut [u8]) -> Result<usize, EncodingError> {
            if buffer.len() < 6 {
                return Err(EncodingError::BufferTooSmall);
            }
            buffer[0] = 0x81;
            buffer[1] = 0x01;
            buffer[2] = 0x04;
            buffer[3] = 0x00;
            buffer[4] = 0x02;
            buffer[5] = 0xFF;
            Ok(6)
        }

        fn response_type(&self) -> Option<ResponseType> {
            None
        }
    }

    #[test]
    fn test_encode_array() {
        let cmd = TestCommand;
        let array: [u8; 6] = cmd.encode_array().unwrap();
        assert_eq!(array, [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    }

    #[test]
    fn test_legacy_to_bytes() {
        let cmd = TestCommand;
        let bytes = cmd.to_bytes().unwrap();
        assert_eq!(bytes, vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    }
}
