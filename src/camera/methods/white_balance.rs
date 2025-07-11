//! White balance methods for cameras that support white balance control.

use crate::camera::Camera;
use crate::capabilities::{ProfileMetadata, WhiteBalance};
use crate::Error;
use grafton_visca_macros::dual_native_method;

/// Extension trait that adds white balance methods to cameras.
#[allow(async_fn_in_trait)]
pub trait WhiteBalanceMethodsExt {
    /// Set auto white balance mode.
    #[cfg(not(feature = "async"))]
    fn white_balance_auto(&mut self) -> Result<(), Error>;

    /// Set auto white balance mode.
    #[cfg(feature = "async")]
    async fn white_balance_auto(&self) -> Result<(), Error>;
}

// Blanket implementation for cameras with white balance support - blocking
#[cfg(not(feature = "async"))]
impl<P, T> WhiteBalanceMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + WhiteBalance,
    T: crate::transport::blocking::BlockingTransport,
{
    #[dual_native_method]
    fn white_balance_auto(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::WHITE_BALANCE_AUTO);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

// Blanket implementation for cameras with white balance support - async
#[cfg(feature = "async")]
impl<P, T> WhiteBalanceMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + WhiteBalance,
    T: crate::transport::AsyncTransport,
{
    #[dual_native_method]
    fn white_balance_auto(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::WHITE_BALANCE_AUTO);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}
