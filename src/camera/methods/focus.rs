//! Focus methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{Focus, ProfileMetadata},
    command::{
        const_encoding::{commands, encode_speed, CommandBuilder},
        Command, Response, ResponseType,
    },
    transport::core::{BlockingTransport, Transport},
    Error,
};
use core::future::Future;

/// Focus auto command.
struct FocusAutoCommand([u8; 6]);

impl FocusAutoCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_AUTO);
        Self(cmd.build())
    }
}

impl Command for FocusAutoCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Focus manual command.
struct FocusManualCommand([u8; 6]);

impl FocusManualCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_MANUAL);
        Self(cmd.build())
    }
}

impl Command for FocusManualCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Focus near command.
struct FocusNearCommand([u8; 6]);

impl FocusNearCommand {
    fn new(speed: u8) -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_NEAR_PREFIX)
            .push(encode_speed(speed));
        Self(cmd.build())
    }
}

impl Command for FocusNearCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Focus far command.
struct FocusFarCommand([u8; 6]);

impl FocusFarCommand {
    fn new(speed: u8) -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_FAR_PREFIX)
            .push(encode_speed(speed));
        Self(cmd.build())
    }
}

impl Command for FocusFarCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Focus stop command.
struct FocusStopCommand([u8; 6]);

impl FocusStopCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_STOP);
        Self(cmd.build())
    }
}

impl Command for FocusStopCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Focus one push command.
struct FocusOnePushCommand([u8; 6]);

impl FocusOnePushCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_ONE_PUSH);
        Self(cmd.build())
    }
}

impl Command for FocusOnePushCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

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
    fn focus_near(&self, speed: u8) -> impl Future<Output = Result<(), Error>> + '_;

    /// Focus far at specified speed - returns a future.
    fn focus_far(&self, speed: u8) -> impl Future<Output = Result<(), Error>> + '_;

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
            let command = FocusAutoCommand::new();
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
            let command = FocusManualCommand::new();
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn focus_near(&self, speed: u8) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let speed = speed.min(P::MAX_FOCUS_SPEED);
            let command = FocusNearCommand::new(speed);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn focus_far(&self, speed: u8) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let speed = speed.min(P::MAX_FOCUS_SPEED);
            let command = FocusFarCommand::new(speed);
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
            let command = FocusStopCommand::new();
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
            let command = FocusOnePushCommand::new();
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
            use crate::command::focus::FocusCommand;

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
    fn focus_near(&self, speed: u8) -> impl Future<Output = Result<(), Error>> + Send;

    /// Focus far at specified speed.
    fn focus_far(&self, speed: u8) -> impl Future<Output = Result<(), Error>> + Send;

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

    fn focus_near(&self, speed: u8) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().focus_near(speed).await }
    }

    fn focus_far(&self, speed: u8) -> impl Future<Output = Result<(), Error>> + Send {
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
    fn focus_near(&self, speed: u8) -> Result<(), Error>;

    /// Focus far at specified speed.
    fn focus_far(&self, speed: u8) -> Result<(), Error>;

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

    fn focus_near(&self, speed: u8) -> Result<(), Error> {
        block_on(self.core().focus_near(speed))
    }

    fn focus_far(&self, speed: u8) -> Result<(), Error> {
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
