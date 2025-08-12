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
impl<P, T> PowerOps for crate::camera::Camera<crate::camera::AsyncMode, P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
{
    async fn power_on(&self) -> Result<(), Error> {
        // Forward to the inherent method
        self.power_on().await
    }

    async fn power_off(&self) -> Result<(), Error> {
        // Forward to the inherent method
        self.power_off().await
    }

    async fn power_inquiry(&self) -> Result<bool, Error> {
        // Forward to the inherent method
        self.power_inquiry().await
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> PowerOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn power_on(&self) -> Result<(), Error> {
        // Forward to the inherent method
        self.power_on()
    }

    fn power_off(&self) -> Result<(), Error> {
        // Forward to the inherent method
        self.power_off()
    }

    fn power_inquiry(&self) -> Result<bool, Error> {
        // Forward to the inherent method
        self.power_inquiry()
    }
}
