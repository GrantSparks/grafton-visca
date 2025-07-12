//! Focus methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{Focus, ProfileMetadata},
    command::{
        focus::{FocusCommand, FocusSpeed},
        Response,
    },
    transport::core::{BlockingTransport, Transport},
    types::SpeedLevel,
    Error,
};
use core::future::Future;

/// Extension trait for CameraCore - provides future-returning methods.
pub trait FocusCoreExt<P, T>
where
    P: ProfileMetadata + Focus,
    T: Transport,
{
    /// Set auto focus mode - returns a future.
    fn focus_auto(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set manual focus mode - returns a future.
    fn focus_manual(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Focus near at specified speed - returns a future.
    fn focus_near(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + '_;

    /// Focus far at specified speed - returns a future.
    fn focus_far(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + '_;

    /// Stop focus movement - returns a future.
    fn focus_stop(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Trigger one-push auto focus - returns a future.
    fn focus_one_push(&self) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set focus to a specific position - returns a future.
    fn set_focus(
        &self,
        position: crate::types::FocusPosition,
    ) -> impl Future<Output = Result<(), Error>> + '_;
}

#[allow(clippy::manual_async_fn)]
impl<P, T> FocusCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + Focus,
    T: Transport,
{
    fn focus_auto(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = FocusCommand::Auto;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn focus_manual(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = FocusCommand::Manual;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn focus_near(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let speed_level = speed;
            let focus_speed_val = speed_level.to_focus_speed().min(P::MAX_FOCUS_SPEED);
            let focus_speed = FocusSpeed::new(focus_speed_val)?;
            let command = FocusCommand::NearWithSpeed(focus_speed);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn focus_far(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let speed_level = speed;
            let focus_speed_val = speed_level.to_focus_speed().min(P::MAX_FOCUS_SPEED);
            let focus_speed = FocusSpeed::new(focus_speed_val)?;
            let command = FocusCommand::FarWithSpeed(focus_speed);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn focus_stop(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = FocusCommand::Stop;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn focus_one_push(&self) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = FocusCommand::OnePushTrigger;
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn set_focus(
        &self,
        position: crate::types::FocusPosition,
    ) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let command = FocusCommand::Position(position);
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
pub trait FocusAsyncExt<P, T>
where
    P: ProfileMetadata + Focus,
    T: Transport,
{
    /// Set auto focus mode.
    fn focus_auto(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set manual focus mode.
    fn focus_manual(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Focus near at specified speed.
    fn focus_near(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + Send;

    /// Focus far at specified speed.
    fn focus_far(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + Send;

    /// Stop focus movement.
    fn focus_stop(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Trigger one-push auto focus.
    fn focus_one_push(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set focus to a specific position.
    fn set_focus(
        &self,
        position: crate::types::FocusPosition,
    ) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> FocusAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + Focus + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn focus_auto(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().focus_auto().await }
    }

    fn focus_manual(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().focus_manual().await }
    }

    fn focus_near(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().focus_near(speed).await }
    }

    fn focus_far(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().focus_far(speed).await }
    }

    fn focus_stop(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().focus_stop().await }
    }

    fn focus_one_push(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().focus_one_push().await }
    }

    fn set_focus(
        &self,
        position: crate::types::FocusPosition,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().set_focus(position).await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait FocusBlockingExt<P, T>
where
    P: ProfileMetadata + Focus,
    T: BlockingTransport,
{
    /// Set auto focus mode.
    fn focus_auto(&self) -> Result<(), Error>;

    /// Set manual focus mode.
    fn focus_manual(&self) -> Result<(), Error>;

    /// Focus near at specified speed.
    fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error>;

    /// Focus far at specified speed.
    fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error>;

    /// Stop focus movement.
    fn focus_stop(&self) -> Result<(), Error>;

    /// Trigger one-push auto focus.
    fn focus_one_push(&self) -> Result<(), Error>;

    /// Set focus to a specific position.
    fn set_focus(&self, position: crate::types::FocusPosition) -> Result<(), Error>;
}

impl<P, T> FocusBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + Focus,
    T: BlockingTransport,
{
    fn focus_auto(&self) -> Result<(), Error> {
        block_on(self.core().focus_auto())
    }

    fn focus_manual(&self) -> Result<(), Error> {
        block_on(self.core().focus_manual())
    }

    fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error> {
        block_on(self.core().focus_near(speed))
    }

    fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error> {
        block_on(self.core().focus_far(speed))
    }

    fn focus_stop(&self) -> Result<(), Error> {
        block_on(self.core().focus_stop())
    }

    fn focus_one_push(&self) -> Result<(), Error> {
        block_on(self.core().focus_one_push())
    }

    fn set_focus(&self, position: crate::types::FocusPosition) -> Result<(), Error> {
        block_on(self.core().set_focus(position))
    }
}
