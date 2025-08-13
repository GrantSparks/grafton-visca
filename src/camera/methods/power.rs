//! Power methods for cameras using mode markers.

use crate::{impl_camera_ops, Error};

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

// Use macro to generate implementations
impl_camera_ops!(
    async,
    PowerOps,
    async fn power_on(&self) -> Result<(), Error>;
    async fn power_off(&self) -> Result<(), Error>;
    async fn power_inquiry(&self) -> Result<bool, Error>;
);

impl_camera_ops!(
    blocking,
    PowerOpsBlocking,
    fn power_on(&self) -> Result<(), Error>;
    fn power_off(&self) -> Result<(), Error>;
    fn power_inquiry(&self) -> Result<bool, Error>;
);
