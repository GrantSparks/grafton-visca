//! White balance methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{ProfileMetadata, WhiteBalance},
    command::{
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        Response,
    },
    transport::core::{BlockingTransport, Transport},
    Error,
};
use core::future::Future;

/// Extension trait for CameraCore - provides future-returning methods.
pub trait WhiteBalanceCoreExt<P, T>
where
    P: ProfileMetadata + WhiteBalance,
    T: Transport,
{
    /// Set auto white balance mode - returns a future.
    fn white_balance_auto(&self) -> impl Future<Output = Result<(), Error>> + '_;
}

#[allow(clippy::manual_async_fn)]
impl<P, T> WhiteBalanceCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + WhiteBalance,
    T: Transport,
{
    fn white_balance_auto(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = WhiteBalanceCommand {
                mode: WhiteBalanceMode::Auto,
            };
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for async Camera facade.
pub trait WhiteBalanceAsyncExt<P, T>
where
    P: ProfileMetadata + WhiteBalance,
    T: Transport,
{
    /// Set auto white balance mode.
    fn white_balance_auto(&self) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> WhiteBalanceAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + WhiteBalance + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn white_balance_auto(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().white_balance_auto().await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait WhiteBalanceBlockingExt<P, T>
where
    P: ProfileMetadata + WhiteBalance,
    T: BlockingTransport,
{
    /// Set auto white balance mode.
    fn white_balance_auto(&self) -> Result<(), Error>;
}

impl<P, T> WhiteBalanceBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + WhiteBalance,
    T: BlockingTransport,
{
    fn white_balance_auto(&self) -> Result<(), Error> {
        block_on(self.core().white_balance_auto())
    }
}
