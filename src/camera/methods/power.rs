//! Power methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{Power, ProfileMetadata},
    command::{
        power::{Power as PowerState, PowerCommand},
        Response,
    },
    transport::core::{BlockingTransport, Transport},
    Error,
};
use core::future::Future;

/// Extension trait for CameraCore - provides future-returning methods.
pub trait PowerCoreExt<P, T>
where
    P: ProfileMetadata + Power,
    T: Transport,
{
    /// Power on the camera - returns a future.
    fn power_on(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Power off the camera - returns a future.
    fn power_off(&self) -> impl Future<Output = Result<(), Error>> + '_;
}

#[allow(clippy::manual_async_fn)]
impl<P, T> PowerCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + Power,
    T: Transport,
{
    fn power_on(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = PowerCommand {
                power: PowerState::On,
            };
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => {
                    // Wait for camera to be ready
                    #[cfg(feature = "tokio")]
                    tokio::time::sleep(P::POWER_ON_TIME).await;
                    Ok(())
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn power_off(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = PowerCommand {
                power: PowerState::Standby,
            };
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => {
                    // Wait for standby/off
                    #[cfg(feature = "tokio")]
                    tokio::time::sleep(P::STANDBY_TIME).await;
                    Ok(())
                }
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for async Camera facade.
pub trait PowerAsyncExt<P, T>
where
    P: ProfileMetadata + Power,
    T: Transport,
{
    /// Power on the camera.
    fn power_on(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Power off the camera.
    fn power_off(&self) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> PowerAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + Power + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn power_on(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().power_on().await }
    }

    fn power_off(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().power_off().await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait PowerBlockingExt<P, T>
where
    P: ProfileMetadata + Power,
    T: BlockingTransport,
{
    /// Power on the camera.
    fn power_on(&self) -> Result<(), Error>;

    /// Power off the camera.
    fn power_off(&self) -> Result<(), Error>;
}

impl<P, T> PowerBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + Power,
    T: BlockingTransport,
{
    fn power_on(&self) -> Result<(), Error> {
        block_on(self.core().power_on())
    }

    fn power_off(&self) -> Result<(), Error> {
        block_on(self.core().power_off())
    }
}
