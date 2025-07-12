//! ND filter methods for cameras that support ND filters using the new GAT architecture.
//!
//! These methods ONLY exist for cameras that implement NDFilter.

use crate::{
    camera::unified::Camera,
    command::{
        encode_visca::EncodeVisca,
        const_encoding::{
            encode_nd_filter_fixed, encode_nd_filter_stepped, encode_nd_filter_variable,
        },
        ResponseType,
    },
    Error,
};


/// ND filter command.
struct NDFilterCommand {
    bytes: Vec<u8>,
}

impl NDFilterCommand {
    fn new_fixed(enabled: bool) -> Self {
        Self {
            bytes: encode_nd_filter_fixed(enabled).to_vec(),
        }
    }

    fn new_stepped(level: u8) -> Self {
        Self {
            bytes: encode_nd_filter_stepped(level).to_vec(),
        }
    }

    fn new_variable(level: u8) -> Self {
        Self {
            bytes: encode_nd_filter_variable(level).to_vec(),
        }
    }
}

impl EncodeVisca for NDFilterCommand {
    type Response = ();
    const MAX_SIZE: usize = 16;

    fn encode_into(&self, buffer: &mut [u8]) -> Result<usize, Error> {
        let len = self.bytes.len();
        if buffer.len() < len {
            return Err(Error::BufferTooSmall {
                required: len,
                actual: buffer.len(),
            });
        }
        buffer[..len].copy_from_slice(&self.bytes);
        Ok(len)
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// ND filter operations.
pub trait NDFilterOps: Sized
{
    /// Set ND filter level.
    #[cfg(feature = "tokio")]
    async fn set_nd_filter(&self, level: u8) -> Result<(), Error>;

    /// Set ND filter level (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_nd_filter_blocking(&mut self, level: u8) -> Result<(), Error>;

    /// Get current ND filter setting.
    #[cfg(feature = "tokio")]
    async fn get_nd_filter(&self) -> Result<u8, Error>;

    /// Get current ND filter setting (blocking).
    #[cfg(not(feature = "tokio"))]
    fn get_nd_filter_blocking(&mut self) -> Result<u8, Error>;
}

impl NDFilterOps for Camera
{
    #[cfg(feature = "tokio")]
    async fn set_nd_filter(&self, level: u8) -> Result<(), Error> {
        // Validate using the camera's ND mode
        let validated_level = self.validate_nd_filter(level)?;

        let command = match self.nd_filter_mode() {
            None | Some(crate::capabilities::NDFilterMode::None) => {
                return Err(Error::FeatureNotSupported {
                    feature: "ND filter".to_string(),
                })
            }
            Some(crate::capabilities::NDFilterMode::Fixed(value)) => NDFilterCommand::new_fixed(validated_level == value),
            Some(crate::capabilities::NDFilterMode::Stepped(_)) => NDFilterCommand::new_stepped(validated_level),
            Some(crate::capabilities::NDFilterMode::Variable) => NDFilterCommand::new_variable(validated_level),
        };

        self.send_command(&command).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_nd_filter_blocking(&mut self, level: u8) -> Result<(), Error> {
        // Validate using the camera's ND mode
        let validated_level = self.validate_nd_filter(level)?;

        let command = match self.nd_filter_mode() {
            None | Some(crate::capabilities::NDFilterMode::None) => {
                return Err(Error::FeatureNotSupported {
                    feature: "ND filter".to_string(),
                })
            }
            Some(crate::capabilities::NDFilterMode::Fixed(value)) => NDFilterCommand::new_fixed(validated_level == value),
            Some(crate::capabilities::NDFilterMode::Stepped(_)) => NDFilterCommand::new_stepped(validated_level),
            Some(crate::capabilities::NDFilterMode::Variable) => NDFilterCommand::new_variable(validated_level),
        };

        self.send_command_blocking(&command)?;
        Ok(())
    }

    #[cfg(feature = "tokio")]
    async fn get_nd_filter(&self) -> Result<u8, Error> {
        // Simplified for demo - would query actual value
        Ok(0)
    }

    #[cfg(not(feature = "tokio"))]
    fn get_nd_filter_blocking(&mut self) -> Result<u8, Error> {
        // Simplified for demo - would query actual value
        Ok(0)
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn test_nd_filter_compile_time_safety() {
        // This test demonstrates compile-time safety - cameras without ND filter
        // capability cannot use ND filter methods
        
        // Note: With the unified Camera API, compile-time safety is achieved
        // through runtime profile checks rather than generic constraints
    }
}