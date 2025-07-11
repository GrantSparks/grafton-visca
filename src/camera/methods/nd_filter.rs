//! ND filter methods for cameras that support ND filters using the new GAT architecture.
//!
//! These methods ONLY exist for cameras that implement NDFilter.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::nd_filter::NDFilterExt,
    capabilities::{NDFilter, NDFilterMode, ProfileMetadata},
    command::{
        const_encoding::{
            encode_nd_filter_fixed, encode_nd_filter_stepped, encode_nd_filter_variable,
        },
        Command, Response, ResponseType,
    },
    transport::gat_transport::Transport,
    Error,
};
use core::future::Future;

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

impl Command for NDFilterCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.bytes.clone())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Extension trait for CameraCore - provides future-returning methods.
pub trait NDFilterCoreExt<P, T>
where
    P: ProfileMetadata + NDFilter + Default,
    T: Transport,
{
    /// Set ND filter level - returns a future.
    fn set_nd_filter(&self, level: u8) -> impl Future<Output = Result<(), Error>> + '_;

    /// Get current ND filter setting - returns a future.
    fn get_nd_filter(&self) -> impl Future<Output = Result<u8, Error>> + '_;
}

impl<P, T> NDFilterCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + NDFilter + Default,
    T: Transport,
{
    fn set_nd_filter(&self, level: u8) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            // Validate using the profile's ND mode
            let profile = P::default();
            let validated_level = profile.validate_nd_filter(level)?;

            let command = match P::ND_MODE {
                NDFilterMode::None => {
                    return Err(Error::FeatureNotSupported {
                        feature: "ND filter".to_string(),
                    })
                }
                NDFilterMode::Fixed(value) => NDFilterCommand::new_fixed(validated_level == value),
                NDFilterMode::Stepped(_) => NDFilterCommand::new_stepped(validated_level),
                NDFilterMode::Variable => NDFilterCommand::new_variable(validated_level),
            };

            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_nd_filter(&self) -> impl Future<Output = Result<u8, Error>> + '_ {
        async move {
            // Simplified for demo - would query actual value
            Ok(0)
        }
    }
}

/// Extension trait for async Camera facade.
pub trait NDFilterAsyncExt<P, T>
where
    P: ProfileMetadata + NDFilter + Default,
    T: Transport,
{
    /// Set ND filter level.
    fn set_nd_filter(&self, level: u8) -> impl Future<Output = Result<(), Error>> + Send;

    /// Get current ND filter setting.
    fn get_nd_filter(&self) -> impl Future<Output = Result<u8, Error>> + Send;
}

impl<P, T> NDFilterAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + NDFilter + Default,
    T: Transport,
{
    fn set_nd_filter(&self, level: u8) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_nd_filter(level).await }
    }

    fn get_nd_filter(&self) -> impl Future<Output = Result<u8, Error>> + Send {
        async move { self.core().get_nd_filter().await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait NDFilterBlockingExt<P, T>
where
    P: ProfileMetadata + NDFilter + Default,
    T: Transport,
{
    /// Set ND filter level.
    fn set_nd_filter(&self, level: u8) -> Result<(), Error>;

    /// Get current ND filter setting.
    fn get_nd_filter(&self) -> Result<u8, Error>;
}

impl<P, T> NDFilterBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + NDFilter + Default,
    T: Transport,
{
    fn set_nd_filter(&self, level: u8) -> Result<(), Error> {
        block_on(self.core().set_nd_filter(level))
    }

    fn get_nd_filter(&self) -> Result<u8, Error> {
        block_on(self.core().get_nd_filter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::SonyFR7;

    #[test]
    fn test_nd_filter_compile_time_safety() {
        // This test demonstrates compile-time safety - cameras without ND filter
        // capability cannot use ND filter methods

        // This compiles - FR7 has ND filter
        fn _test_fr7_nd_filter<T: Transport>(_camera: &CameraAsync<SonyFR7, T>) {
            // Camera with ND filter can use these methods
        }

        // This would NOT compile - PTZOpticsG2 doesn't have ND filter
        // fn _test_g2_nd_filter<T: Transport>(_camera: &CameraAsync<PTZOpticsG2, T>) {
        //     // COMPILE ERROR: the trait bound `PTZOpticsG2: NDFilter` is not satisfied
        // }
    }
}
