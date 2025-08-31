//! Power methods for unified camera API.

use crate::Error;

/// Power operations for cameras.
///
/// This trait provides power control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait PowerControl {
    /// Power on the camera.
    #[cfg(feature = "async")]
    fn power_on(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Power on the camera.
    #[cfg(not(feature = "async"))]
    fn power_on(&mut self) -> Result<(), Error>;

    /// Power off the camera.
    #[cfg(feature = "async")]
    fn power_off(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Power off the camera.
    #[cfg(not(feature = "async"))]
    fn power_off(&mut self) -> Result<(), Error>;

    /// Query the current power status.
    #[cfg(feature = "async")]
    fn power_inquiry(&self) -> impl std::future::Future<Output = Result<bool, Error>> + Send + '_;

    /// Query the current power status.
    #[cfg(not(feature = "async"))]
    fn power_inquiry(&mut self) -> Result<bool, Error>;
}

// Keep the old trait names for backward compatibility during transition

// Async implementation for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> PowerControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> PowerControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
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
