//! Tally light control methods for cameras.

use crate::camera::Camera;
use crate::capabilities::ProfileMetadata;
use crate::command::tally::TallyCommand;
use crate::command::Command;
use crate::Error;

/// Extension trait that adds tally light control methods to cameras.
#[allow(async_fn_in_trait)]
pub trait TallyMethodsExt {
    /// Turn red tally light on.
    #[cfg(not(feature = "async"))]
    fn tally_red_on(&mut self) -> Result<(), Error>;

    /// Turn red tally light on.
    #[cfg(feature = "async")]
    async fn tally_red_on(&self) -> Result<(), Error>;

    /// Turn red tally light off.
    #[cfg(not(feature = "async"))]
    fn tally_red_off(&mut self) -> Result<(), Error>;

    /// Turn red tally light off.
    #[cfg(feature = "async")]
    async fn tally_red_off(&self) -> Result<(), Error>;

    /// Set tally brightness to low.
    #[cfg(not(feature = "async"))]
    fn tally_bright_lo(&mut self) -> Result<(), Error>;

    /// Set tally brightness to low.
    #[cfg(feature = "async")]
    async fn tally_bright_lo(&self) -> Result<(), Error>;

    /// Set tally brightness to high.
    #[cfg(not(feature = "async"))]
    fn tally_bright_hi(&mut self) -> Result<(), Error>;

    /// Set tally brightness to high.
    #[cfg(feature = "async")]
    async fn tally_bright_hi(&self) -> Result<(), Error>;

    /// Turn green tally light on.
    #[cfg(not(feature = "async"))]
    fn tally_green_on(&mut self) -> Result<(), Error>;

    /// Turn green tally light on.
    #[cfg(feature = "async")]
    async fn tally_green_on(&self) -> Result<(), Error>;

    /// Turn green tally light off.
    #[cfg(not(feature = "async"))]
    fn tally_green_off(&mut self) -> Result<(), Error>;

    /// Turn green tally light off.
    #[cfg(feature = "async")]
    async fn tally_green_off(&self) -> Result<(), Error>;

    /// Flash tally light.
    #[cfg(not(feature = "async"))]
    fn tally_flash(&mut self) -> Result<(), Error>;

    /// Flash tally light.
    #[cfg(feature = "async")]
    async fn tally_flash(&self) -> Result<(), Error>;

    /// Turn tally light on.
    #[cfg(not(feature = "async"))]
    fn tally_on(&mut self) -> Result<(), Error>;

    /// Turn tally light on.
    #[cfg(feature = "async")]
    async fn tally_on(&self) -> Result<(), Error>;

    /// Turn tally light off.
    #[cfg(not(feature = "async"))]
    fn tally_off(&mut self) -> Result<(), Error>;

    /// Turn tally light off.
    #[cfg(feature = "async")]
    async fn tally_off(&self) -> Result<(), Error>;

    /// Get tally light status.
    #[cfg(not(feature = "async"))]
    fn get_tally_status(&mut self) -> Result<bool, Error>;

    /// Get tally light status.
    #[cfg(feature = "async")]
    async fn get_tally_status(&self) -> Result<bool, Error>;
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P, T> TallyMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::blocking::BlockingTransport,
{
    fn tally_red_on(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::RedOn.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn tally_red_off(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::RedOff.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn tally_bright_lo(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::BrightLo.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn tally_bright_hi(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::BrightHi.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn tally_green_on(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::GreenOn.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn tally_green_off(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::GreenOff.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn tally_flash(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::Flash.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn tally_on(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::On.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn tally_off(&mut self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_blocking(&TallyCommand::Off.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn get_tally_status(&mut self) -> Result<bool, Error> {
        // For now, return a NotImplemented error as there's no tally inquiry command in the protocol
        // This would need to be added to the InquiryCommand enum with the proper VISCA bytes
        Err(Error::FeatureNotSupported {
            feature: "Tally status inquiry".to_string(),
        })
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> TallyMethodsExt for Camera<P, T>
where
    P: ProfileMetadata,
    T: crate::transport::AsyncTransport,
{
    async fn tally_red_on(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::RedOn.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn tally_red_off(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::RedOff.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn tally_bright_lo(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::BrightLo.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn tally_bright_hi(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::BrightHi.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn tally_green_on(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::GreenOn.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn tally_green_off(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::GreenOff.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn tally_flash(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::Flash.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn tally_on(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::On.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn tally_off(&self) -> Result<(), Error> {
        let response_bytes = self
            .transport
            .send_async(&TallyCommand::Off.to_bytes()?)
            .await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn get_tally_status(&self) -> Result<bool, Error> {
        // For now, return a NotImplemented error as there's no tally inquiry command in the protocol
        // This would need to be added to the InquiryCommand enum with the proper VISCA bytes
        Err(Error::FeatureNotSupported {
            feature: "Tally status inquiry".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::PTZOpticsG2;

    #[test]
    fn test_tally_methods_compile() {
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

        let mut _camera: Camera<PTZOpticsG2, MockTransport> = Camera::new(MockTransport);

        #[cfg(not(feature = "async"))]
        {
            let _ = _camera.tally_red_on();
            let _ = _camera.tally_off();
            let _ = _camera.get_tally_status();
        }
    }
}
