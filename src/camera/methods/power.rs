//! Power methods for cameras using mode markers.

use crate::Error;

/// Power operations (async).
#[cfg(feature = "async")]
pub trait PowerOps: Sized {
    /// Power on the camera.
    async fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera.
    async fn power_off(&self) -> Result<(), Error>;

    /// Query the current power status.
    async fn power_inquiry(&self) -> Result<bool, Error>;
}

/// Power operations (blocking).
#[cfg(not(feature = "async"))]
pub trait PowerOpsBlocking: Sized {
    /// Power on the camera.
    fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera.
    fn power_off(&self) -> Result<(), Error>;

    /// Query the current power status.
    fn power_inquiry(&self) -> Result<bool, Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> PowerOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn power_on(&self) -> Result<(), Error> {
        use crate::command::power::PowerCommand;
        let cmd = PowerCommand::On;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn power_off(&self) -> Result<(), Error> {
        use crate::command::power::PowerCommand;
        let cmd = PowerCommand::Standby;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn power_inquiry(&self) -> Result<bool, Error> {
        use crate::command::{inquiry::PowerInquiry, response::Response, InquiryResponse};
        let inquiry = PowerInquiry {};
        let response = self.send_command(&inquiry).await?;
        match response {
            Response::Inquiry(InquiryResponse::Power { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> PowerOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn power_on(&self) -> Result<(), Error> {
        use crate::command::power::PowerCommand;
        let cmd = PowerCommand::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn power_off(&self) -> Result<(), Error> {
        use crate::command::power::PowerCommand;
        let cmd = PowerCommand::Standby;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn power_inquiry(&self) -> Result<bool, Error> {
        use crate::command::{inquiry::PowerInquiry, response::Response, InquiryResponse};
        let inquiry = PowerInquiry {};
        let response = self.send_command(&inquiry)?;
        match response {
            Response::Inquiry(InquiryResponse::Power { on }) => Ok(on),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
