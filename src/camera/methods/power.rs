//! Power methods for cameras using mode markers.

use crate::Error;

/// Power operations (async).
#[cfg(feature = "async")]
pub trait PowerControl: Sized {
    /// Power on the camera.
    async fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera.
    async fn power_off(&self) -> Result<(), Error>;

    /// Query the current power status.
    async fn power_inquiry(&self) -> Result<bool, Error>;
}

/// Power operations (blocking).
pub trait PowerControlBlocking: Sized {
    /// Power on the camera.
    fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera.
    fn power_off(&self) -> Result<(), Error>;

    /// Query the current power status.
    fn power_inquiry(&self) -> Result<bool, Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> PowerControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn power_on(&self) -> Result<(), Error> {
        use crate::command::power::Power;
        let cmd = Power::On;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn power_off(&self) -> Result<(), Error> {
        use crate::command::power::Power;
        let cmd = Power::Standby;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn power_inquiry(&self) -> Result<bool, Error> {
        use crate::command::{inquiry::PowerInquiry, response::ViscaResponse, InquiryResponse};
        let inquiry = PowerInquiry {};
        let response = self.send_command(&inquiry).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Power { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> PowerControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn power_on(&self) -> Result<(), Error> {
        use crate::command::power::Power;
        let cmd = Power::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn power_off(&self) -> Result<(), Error> {
        use crate::command::power::Power;
        let cmd = Power::Standby;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn power_inquiry(&self) -> Result<bool, Error> {
        use crate::command::{inquiry::PowerInquiry, response::ViscaResponse, InquiryResponse};
        let inquiry = PowerInquiry {};
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::Power { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
