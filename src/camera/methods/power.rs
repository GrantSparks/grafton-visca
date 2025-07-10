//! Power control methods for cameras.

use crate::camera::Camera;
use crate::capabilities::{ProfileMetadata, SupportsPower};
use crate::command::const_encoding::constants::power;
use crate::Error;

/// Extension trait that adds power methods to cameras.
#[allow(async_fn_in_trait)]
pub trait PowerMethods {
    /// Power on the camera.
    #[cfg(not(feature = "async"))]
    fn power_on(&mut self) -> Result<(), Error>;

    /// Power on the camera.
    #[cfg(feature = "async")]
    async fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera (standby if supported).
    #[cfg(not(feature = "async"))]
    fn power_off(&mut self) -> Result<(), Error>;

    /// Power off the camera (standby if supported).
    #[cfg(feature = "async")]
    async fn power_off(&self) -> Result<(), Error>;
}

// Blocking implementation for cameras with power control
#[cfg(not(feature = "async"))]
impl<P, T> PowerMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsPower,
    T: crate::transport::blocking::BlockingTransport,
{
    fn power_on(&mut self) -> Result<(), Error> {
        self.send_const(power::ON)?;

        // Wait for camera to be ready
        std::thread::sleep(P::POWER_ON_TIME);
        Ok(())
    }

    fn power_off(&mut self) -> Result<(), Error> {
        self.send_const(power::OFF)?;

        // Wait for standby/off
        std::thread::sleep(P::STANDBY_TIME);
        Ok(())
    }
}

// Async implementation for cameras with power control
#[cfg(feature = "async")]
impl<P, T> PowerMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsPower,
    T: crate::transport::AsyncTransport,
{
    async fn power_on(&self) -> Result<(), Error> {
        self.send_const(power::ON).await?;

        // Wait for camera to be ready
        tokio::time::sleep(P::POWER_ON_TIME).await;
        Ok(())
    }

    async fn power_off(&self) -> Result<(), Error> {
        self.send_const(power::OFF).await?;

        // Wait for standby/off
        tokio::time::sleep(P::STANDBY_TIME).await;
        Ok(())
    }
}
