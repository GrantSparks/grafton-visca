//! Focus methods for cameras that support focus control.

use crate::camera::Camera;
use crate::capabilities::{Focus, ProfileMetadata};
use crate::command::Command;
use crate::Error;
use grafton_visca_macros::dual_native_method;

/// Extension trait that adds focus methods to cameras.
#[allow(async_fn_in_trait)]
pub trait FocusMethodsExt {
    /// Set auto focus mode.
    #[cfg(not(feature = "async"))]
    fn focus_auto(&mut self) -> Result<(), Error>;

    /// Set auto focus mode.
    #[cfg(feature = "async")]
    async fn focus_auto(&self) -> Result<(), Error>;

    /// Set manual focus mode.
    #[cfg(not(feature = "async"))]
    fn focus_manual(&mut self) -> Result<(), Error>;

    /// Set manual focus mode.
    #[cfg(feature = "async")]
    async fn focus_manual(&self) -> Result<(), Error>;

    /// Focus near at specified speed.
    #[cfg(not(feature = "async"))]
    fn focus_near(&mut self, speed: u8) -> Result<(), Error>;

    /// Focus near at specified speed.
    #[cfg(feature = "async")]
    async fn focus_near(&self, speed: u8) -> Result<(), Error>;

    /// Focus far at specified speed.
    #[cfg(not(feature = "async"))]
    fn focus_far(&mut self, speed: u8) -> Result<(), Error>;

    /// Focus far at specified speed.
    #[cfg(feature = "async")]
    async fn focus_far(&self, speed: u8) -> Result<(), Error>;

    /// Stop focus movement.
    #[cfg(not(feature = "async"))]
    fn focus_stop(&mut self) -> Result<(), Error>;

    /// Stop focus movement.
    #[cfg(feature = "async")]
    async fn focus_stop(&self) -> Result<(), Error>;

    /// Trigger one-push auto focus.
    #[cfg(not(feature = "async"))]
    fn focus_one_push(&mut self) -> Result<(), Error>;

    /// Trigger one-push auto focus.
    #[cfg(feature = "async")]
    async fn focus_one_push(&self) -> Result<(), Error>;

    /// Set focus to a specific position.
    #[cfg(not(feature = "async"))]
    fn set_focus(&mut self, position: crate::types::FocusPosition) -> Result<(), Error>;

    /// Set focus to a specific position.
    #[cfg(feature = "async")]
    async fn set_focus(&self, position: crate::types::FocusPosition) -> Result<(), Error>;
}

// Blanket implementation for cameras with focus support - blocking
#[cfg(not(feature = "async"))]
impl<P, T> FocusMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + Focus,
    T: crate::transport::blocking::BlockingTransport,
{
    #[dual_native_method]
    fn focus_auto(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_AUTO);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_manual(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_MANUAL);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_near(&mut self, speed: u8) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_speed, CommandBuilder};

        let speed = speed.min(P::MAX_FOCUS_SPEED);

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_NEAR_PREFIX)
            .push(encode_speed(speed));

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_far(&mut self, speed: u8) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_speed, CommandBuilder};

        let speed = speed.min(P::MAX_FOCUS_SPEED);

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_FAR_PREFIX)
            .push(encode_speed(speed));

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_stop(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_STOP);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_one_push(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_ONE_PUSH);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn set_focus(&mut self, position: crate::types::FocusPosition) -> Result<(), Error> {
        use crate::command::focus::FocusCommand;

        let cmd = FocusCommand::Position(position);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> FocusMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + Focus,
    T: crate::transport::AsyncTransport,
{
    #[dual_native_method]
    fn focus_auto(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_AUTO);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_manual(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_MANUAL);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_near(&mut self, speed: u8) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_speed, CommandBuilder};

        let speed = speed.min(P::MAX_FOCUS_SPEED);

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_NEAR_PREFIX)
            .push(encode_speed(speed));

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_far(&mut self, speed: u8) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, encode_speed, CommandBuilder};

        let speed = speed.min(P::MAX_FOCUS_SPEED);

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_FAR_PREFIX)
            .push(encode_speed(speed));

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_stop(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_STOP);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn focus_one_push(&mut self) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        let mut cmd = CommandBuilder::<6>::new();
        cmd.append(commands::FOCUS_ONE_PUSH);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    #[dual_native_method]
    fn set_focus(&mut self, position: crate::types::FocusPosition) -> Result<(), Error> {
        use crate::command::focus::FocusCommand;

        let cmd = FocusCommand::Position(position);
        let response_bytes = self.send_raw(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}
