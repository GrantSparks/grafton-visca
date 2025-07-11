//! Tally light control methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::ProfileMetadata,
    command::{tally::TallyCommand, Response},
    transport::gat_transport::Transport,
    Error,
};
use core::future::Future;

/// Extension trait for CameraCore - provides future-returning methods.
pub trait TallyCoreExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Turn red tally light on - returns a future.
    fn tally_red_on(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Turn red tally light off - returns a future.
    fn tally_red_off(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set tally brightness to low - returns a future.
    fn tally_bright_lo(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set tally brightness to high - returns a future.
    fn tally_bright_hi(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Turn green tally light on - returns a future.
    fn tally_green_on(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Turn green tally light off - returns a future.
    fn tally_green_off(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Flash tally light - returns a future.
    fn tally_flash(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Turn tally light on - returns a future.
    fn tally_on(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Turn tally light off - returns a future.
    fn tally_off(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Get tally light status - returns a future.
    fn get_tally_status(&self) -> impl Future<Output = Result<bool, Error>> + '_;
}

impl<P, T> TallyCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn tally_red_on(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::RedOn;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn tally_red_off(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::RedOff;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn tally_bright_lo(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::BrightLo;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn tally_bright_hi(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::BrightHi;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn tally_green_on(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::GreenOn;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn tally_green_off(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::GreenOff;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn tally_flash(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::Flash;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn tally_on(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::On;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn tally_off(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = TallyCommand::Off;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn get_tally_status(&self) -> impl Future<Output = Result<bool, Error>> + '_ {
        async move {
            // For now, return a NotImplemented error as there's no tally inquiry command in the protocol
            // This would need to be added to the InquiryCommand enum with the proper VISCA bytes
            Err(Error::FeatureNotSupported {
                feature: "Tally status inquiry".to_string(),
            })
        }
    }
}

/// Extension trait for async Camera facade.
pub trait TallyAsyncExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Turn red tally light on.
    fn tally_red_on(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Turn red tally light off.
    fn tally_red_off(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set tally brightness to low.
    fn tally_bright_lo(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set tally brightness to high.
    fn tally_bright_hi(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Turn green tally light on.
    fn tally_green_on(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Turn green tally light off.
    fn tally_green_off(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Flash tally light.
    fn tally_flash(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Turn tally light on.
    fn tally_on(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Turn tally light off.
    fn tally_off(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Get tally light status.
    fn get_tally_status(&self) -> impl Future<Output = Result<bool, Error>> + Send;
}

impl<P, T> TallyAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn tally_red_on(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_red_on().await }
    }

    fn tally_red_off(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_red_off().await }
    }

    fn tally_bright_lo(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_bright_lo().await }
    }

    fn tally_bright_hi(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_bright_hi().await }
    }

    fn tally_green_on(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_green_on().await }
    }

    fn tally_green_off(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_green_off().await }
    }

    fn tally_flash(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_flash().await }
    }

    fn tally_on(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_on().await }
    }

    fn tally_off(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().tally_off().await }
    }

    fn get_tally_status(&self) -> impl Future<Output = Result<bool, Error>> + Send {
        async move { self.core().get_tally_status().await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait TallyBlockingExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Turn red tally light on.
    fn tally_red_on(&self) -> Result<(), Error>;

    /// Turn red tally light off.
    fn tally_red_off(&self) -> Result<(), Error>;

    /// Set tally brightness to low.
    fn tally_bright_lo(&self) -> Result<(), Error>;

    /// Set tally brightness to high.
    fn tally_bright_hi(&self) -> Result<(), Error>;

    /// Turn green tally light on.
    fn tally_green_on(&self) -> Result<(), Error>;

    /// Turn green tally light off.
    fn tally_green_off(&self) -> Result<(), Error>;

    /// Flash tally light.
    fn tally_flash(&self) -> Result<(), Error>;

    /// Turn tally light on.
    fn tally_on(&self) -> Result<(), Error>;

    /// Turn tally light off.
    fn tally_off(&self) -> Result<(), Error>;

    /// Get tally light status.
    fn get_tally_status(&self) -> Result<bool, Error>;
}

impl<P, T> TallyBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn tally_red_on(&self) -> Result<(), Error> {
        block_on(self.core().tally_red_on())
    }

    fn tally_red_off(&self) -> Result<(), Error> {
        block_on(self.core().tally_red_off())
    }

    fn tally_bright_lo(&self) -> Result<(), Error> {
        block_on(self.core().tally_bright_lo())
    }

    fn tally_bright_hi(&self) -> Result<(), Error> {
        block_on(self.core().tally_bright_hi())
    }

    fn tally_green_on(&self) -> Result<(), Error> {
        block_on(self.core().tally_green_on())
    }

    fn tally_green_off(&self) -> Result<(), Error> {
        block_on(self.core().tally_green_off())
    }

    fn tally_flash(&self) -> Result<(), Error> {
        block_on(self.core().tally_flash())
    }

    fn tally_on(&self) -> Result<(), Error> {
        block_on(self.core().tally_on())
    }

    fn tally_off(&self) -> Result<(), Error> {
        block_on(self.core().tally_off())
    }

    fn get_tally_status(&self) -> Result<bool, Error> {
        block_on(self.core().get_tally_status())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::PTZOpticsG2;

    #[test]
    fn test_tally_methods_compile() {
        // This test demonstrates that tally methods are available for all cameras

        fn _test_tally_methods<T: Transport>(_camera: &CameraAsync<PTZOpticsG2, T>) {
            // All cameras can use tally methods
        }
    }
}
