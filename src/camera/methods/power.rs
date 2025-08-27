//! Power methods for cameras using mode markers.

use crate::Error;

/// Power operations (async).
#[cfg(feature = "async")]
pub trait PowerControl: Send + Sync + 'static + Sized {
    /// Power on the camera.
    fn power_on(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Power off the camera.
    fn power_off(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Query the current power status.
    fn power_inquiry(&self) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;
}

/// Power operations (blocking).
pub trait PowerControlBlocking: Sized {
    /// Power on the camera.
    fn power_on(&mut self) -> Result<(), Error>;

    /// Power off the camera.
    fn power_off(&mut self) -> Result<(), Error>;

    /// Query the current power status.
    fn power_inquiry(&mut self) -> Result<bool, Error>;
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
        use crate::command::inquiry_structs::PowerInquiry;

        self.send_command_typed(&PowerInquiry).await
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> PowerControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + Default,
    T: crate::transport::BlockingTransport + Send + 'static,
{
    fn power_on(&mut self) -> Result<(), Error> {
        use crate::command::power::Power;

        let cmd = Power::On;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn power_off(&mut self) -> Result<(), Error> {
        use crate::command::power::Power;

        let cmd = Power::Standby;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn power_inquiry(&mut self) -> Result<bool, Error> {
        use crate::command::inquiry_structs::PowerInquiry;

        self.send_command_typed(&PowerInquiry)
    }
}
