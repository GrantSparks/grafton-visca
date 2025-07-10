//! ND filter methods for cameras that support ND filters.
//!
//! These methods ONLY exist for cameras that implement NDFilter.

use crate::camera::Camera;
use crate::capabilities::nd_filter::NDFilterExt;
use crate::capabilities::{NDFilter, NDFilterMode, ProfileMetadata};
use crate::command::const_encoding::{
    encode_nd_filter_fixed, encode_nd_filter_stepped, encode_nd_filter_variable,
};
use crate::Error;

/// Extension trait that adds ND filter methods to cameras.
#[allow(async_fn_in_trait)]
pub trait NDFilterMethodsExt {
    /// Set ND filter level.
    #[cfg(not(feature = "async"))]
    fn set_nd_filter(&mut self, level: u8) -> Result<(), Error>;

    /// Set ND filter level.
    #[cfg(feature = "async")]
    async fn set_nd_filter(&self, level: u8) -> Result<(), Error>;

    /// Get current ND filter setting.
    #[cfg(not(feature = "async"))]
    fn get_nd_filter(&mut self) -> Result<u8, Error>;

    /// Get current ND filter setting.
    #[cfg(feature = "async")]
    async fn get_nd_filter(&self) -> Result<u8, Error>;
}

// Blocking implementation ONLY for cameras with ND filter
#[cfg(not(feature = "async"))]
impl<P, T> NDFilterMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + NDFilter + Default,
    T: crate::transport::blocking::BlockingTransport,
{
    fn set_nd_filter(&mut self, level: u8) -> Result<(), Error> {
        // Validate using the profile's ND mode
        let profile = P::default();
        let validated_level = profile.validate_nd_filter(level)?;

        match P::ND_MODE {
            NDFilterMode::None => Err(Error::FeatureNotSupported {
                feature: "ND filter".to_string(),
            }),
            NDFilterMode::Fixed(value) => {
                let cmd = encode_nd_filter_fixed(validated_level == value);
                self.send_array(cmd)
            }
            NDFilterMode::Stepped(_) => {
                let cmd = encode_nd_filter_stepped(validated_level);
                self.send_array(cmd)
            }
            NDFilterMode::Variable => {
                let cmd = encode_nd_filter_variable(validated_level);
                self.send_array(cmd)
            }
        }
    }

    fn get_nd_filter(&mut self) -> Result<u8, Error> {
        // Simplified for demo - would query actual value
        Ok(0)
    }
}

// Async implementation ONLY for cameras with ND filter
#[cfg(feature = "async")]
impl<P, T> NDFilterMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + NDFilter,
    T: crate::transport::AsyncTransport,
{
    async fn set_nd_filter(&self, level: u8) -> Result<(), Error> {
        // Create validator
        struct Validator<P>(std::marker::PhantomData<P>);
        impl<P: NDFilter> Validator<P> {
            fn validate(&self, level: u8) -> Result<u8, Error> {
                struct Dummy<P>(std::marker::PhantomData<P>);

                impl<P> Default for Dummy<P> {
                    fn default() -> Self {
                        Self(std::marker::PhantomData)
                    }
                }
                impl<P: NDFilter> NDFilter for Dummy<P> {
                    const ND_MODE: NDFilterMode = P::ND_MODE;
                    const ND_STEPS: Option<u8> = P::ND_STEPS;
                }

                let dummy = Dummy::<P>::default();
                dummy
                    .validate_nd_filter(level)
                    .map_err(Error::ValidationError)
            }
        }

        let validator = Validator::<P>(std::marker::PhantomData);
        let validated_level = validator.validate(level)?;

        match P::ND_MODE {
            NDFilterMode::None => Err(Error::FeatureNotSupported {
                feature: "ND filter".to_string(),
            }),
            NDFilterMode::Fixed(value) => {
                let cmd = encode_nd_filter_fixed(validated_level == value);
                self.send_array(cmd).await
            }
            NDFilterMode::Stepped(_) => {
                let cmd = encode_nd_filter_stepped(validated_level);
                self.send_array(cmd).await
            }
            NDFilterMode::Variable => {
                let cmd = encode_nd_filter_variable(validated_level);
                self.send_array(cmd).await
            }
        }
    }

    async fn get_nd_filter(&self) -> Result<u8, Error> {
        // Simplified for demo
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::SonyFR7;

    #[test]
    fn test_nd_filter_compile_time_safety() {
        #[derive(Debug)]
        struct MockTransport;

        #[cfg(not(feature = "async"))]
        impl crate::transport::blocking::BlockingTransport for MockTransport {
            fn send(&mut self, _data: &[u8]) -> Result<(), Error> {
                Ok(())
            }
            fn receive(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, Error> {
                Ok(vec![0x90, 0x50, 0xFF])
            }
            fn is_connected(&self) -> bool {
                true
            }
            fn description(&self) -> &str {
                "MockTransport"
            }
        }

        // This compiles - FR7 has ND filter
        let mut _fr7_camera: Camera<SonyFR7, MockTransport> = Camera::new(MockTransport);
        #[cfg(not(feature = "async"))]
        {
            let _ = _fr7_camera.set_nd_filter(2);
        }

        // This would NOT compile - PTZOpticsG2 doesn't have ND filter
        // let mut g2_camera: Camera<PTZOpticsG2, MockTransport> = Camera::new(MockTransport);
        // let _ = g2_camera.set_nd_filter(2); // COMPILE ERROR!
    }
}
