//! Pan/Tilt methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{pan_tilt::PanTiltExt, PanTilt, ProfileMetadata},
    command::{
        const_encoding::{
            commands, encode_pan_tilt_absolute, encode_pan_tilt_relative, CommandBuilder,
        },
        pan_tilt::{PanTiltCommand, PanTiltDirection},
        Command, Response,
    },
    transport::gat_transport::Transport,
    types::{PanSpeed, TiltSpeed},
    Error,
};
use core::future::Future;

/// Pan tilt stop command.
struct PanTiltStopCommand([u8; 9]);

impl PanTiltStopCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<9>::new();
        cmd.append(commands::PAN_TILT_STOP);
        Self(cmd.build())
    }
}

impl Command for PanTiltStopCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<crate::command::ResponseType> {
        None // Action command
    }
}

/// Pan tilt home command.
struct PanTiltHomeCommand([u8; 7]);

impl PanTiltHomeCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(commands::PAN_TILT_HOME);
        Self(cmd.build())
    }
}

impl Command for PanTiltHomeCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<crate::command::ResponseType> {
        None // Action command
    }
}

/// Pan tilt reset command.
struct PanTiltResetCommand([u8; 9]);

impl PanTiltResetCommand {
    fn new() -> Self {
        let mut cmd = CommandBuilder::<9>::new();
        cmd.append(&[0x81, 0x01, 0x06, 0x05, 0xFF]); // RESET is not in commands.rs
        Self(cmd.build())
    }
}

impl Command for PanTiltResetCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<crate::command::ResponseType> {
        None // Action command
    }
}

/// Pan tilt absolute position command.
struct PanTiltAbsoluteCommand([u8; 15]);

impl PanTiltAbsoluteCommand {
    fn new(pan: i16, tilt: i16, pan_speed: u8, tilt_speed: u8) -> Self {
        Self(encode_pan_tilt_absolute(pan, tilt, pan_speed, tilt_speed))
    }
}

impl Command for PanTiltAbsoluteCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<crate::command::ResponseType> {
        None // Action command
    }
}

/// Pan tilt relative position command.
struct PanTiltRelativeCommand([u8; 15]);

impl PanTiltRelativeCommand {
    fn new(pan: i16, tilt: i16, pan_speed: u8, tilt_speed: u8) -> Self {
        Self(encode_pan_tilt_relative(pan, tilt, pan_speed, tilt_speed))
    }
}

impl Command for PanTiltRelativeCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<crate::command::ResponseType> {
        None // Action command
    }
}

/// Extension trait for CameraCore that adds pan/tilt methods.
pub trait PanTiltCoreExt<P: ProfileMetadata + PanTilt> {
    /// Stop all pan/tilt movement.
    fn pan_tilt_stop(&self) -> impl Future<Output = Result<(), Error>>;

    /// Move to home position (0, 0).
    fn pan_tilt_home(&self) -> impl Future<Output = Result<(), Error>>;

    /// Move to absolute pan/tilt position in degrees.
    fn pan_tilt_absolute(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&self) -> impl Future<Output = Result<(), Error>>;
}

impl<P, T> PanTiltCoreExt<P> for CameraCore<P, T>
where
    P: ProfileMetadata + PanTilt + Default,
    T: Transport,
{
    fn pan_tilt_stop(&self) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = PanTiltStopCommand::new();
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn pan_tilt_home(&self) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = PanTiltHomeCommand::new();
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn pan_tilt_absolute(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            // Create a dummy profile instance to use extension trait methods
            let profile = P::default();

            // Use the extension trait to convert degrees to units
            let pan = profile.degrees_to_pan_units(pan_degrees);
            let tilt = profile.degrees_to_tilt_units(tilt_degrees);

            // Validate using profile constants
            let pan = profile.validate_pan(pan)?;
            let tilt = profile.validate_tilt(tilt)?;
            let pan_speed = profile.validate_pan_speed(speed);
            let tilt_speed = profile.validate_tilt_speed(speed);

            let cmd = PanTiltAbsoluteCommand::new(pan, tilt, pan_speed, tilt_speed);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn pan_tilt_relative(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            let profile = P::default();
            let pan = profile.degrees_to_pan_units(pan_degrees);
            let tilt = profile.degrees_to_tilt_units(tilt_degrees);
            let pan_speed = profile.validate_pan_speed(speed);
            let tilt_speed = profile.validate_tilt_speed(speed);

            let cmd = PanTiltRelativeCommand::new(pan, tilt, pan_speed, tilt_speed);
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            let profile = P::default();
            let pan_speed = profile.validate_pan_speed(pan_speed);
            let tilt_speed = profile.validate_tilt_speed(tilt_speed);

            let cmd = PanTiltCommand::Move {
                direction,
                pan_speed: PanSpeed::new(pan_speed)?,
                tilt_speed: TiltSpeed::new(tilt_speed)?,
            };

            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn pan_tilt_reset(&self) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = PanTiltResetCommand::new();
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for CameraAsync that adds pan/tilt methods.
pub trait PanTiltAsyncExt<P: ProfileMetadata + PanTilt>: Sized {
    /// Stop all pan/tilt movement.
    async fn pan_tilt_stop(&self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    async fn pan_tilt_home(&self) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    async fn pan_tilt_absolute(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    async fn pan_tilt_relative(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    async fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    async fn pan_tilt_reset(&self) -> Result<(), Error>;
}

impl<P, T> PanTiltAsyncExt<P> for CameraAsync<P, T>
where
    P: ProfileMetadata + PanTilt + Default,
    T: Transport,
{
    async fn pan_tilt_stop(&self) -> Result<(), Error> {
        self.core().pan_tilt_stop().await
    }

    async fn pan_tilt_home(&self) -> Result<(), Error> {
        self.core().pan_tilt_home().await
    }

    async fn pan_tilt_absolute(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error> {
        self.core()
            .pan_tilt_absolute(pan_degrees, tilt_degrees, speed)
            .await
    }

    async fn pan_tilt_relative(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error> {
        self.core()
            .pan_tilt_relative(pan_degrees, tilt_degrees, speed)
            .await
    }

    async fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error> {
        self.core()
            .pan_tilt_move(direction, pan_speed, tilt_speed)
            .await
    }

    async fn pan_tilt_reset(&self) -> Result<(), Error> {
        self.core().pan_tilt_reset().await
    }
}

/// Extension trait for CameraBlocking that adds pan/tilt methods.
pub trait PanTiltBlockingExt<P: ProfileMetadata + PanTilt>: Sized {
    /// Stop all pan/tilt movement.
    fn pan_tilt_stop(&mut self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    fn pan_tilt_home(&mut self) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    fn pan_tilt_absolute(
        &mut self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &mut self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&mut self) -> Result<(), Error>;
}

impl<P, T> PanTiltBlockingExt<P> for CameraBlocking<P, T>
where
    P: ProfileMetadata + PanTilt + Default,
    T: Transport,
{
    fn pan_tilt_stop(&mut self) -> Result<(), Error> {
        block_on(self.core().pan_tilt_stop())
    }

    fn pan_tilt_home(&mut self) -> Result<(), Error> {
        block_on(self.core().pan_tilt_home())
    }

    fn pan_tilt_absolute(
        &mut self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error> {
        block_on(
            self.core()
                .pan_tilt_absolute(pan_degrees, tilt_degrees, speed),
        )
    }

    fn pan_tilt_relative(
        &mut self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error> {
        block_on(
            self.core()
                .pan_tilt_relative(pan_degrees, tilt_degrees, speed),
        )
    }

    fn pan_tilt_move(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error> {
        block_on(self.core().pan_tilt_move(direction, pan_speed, tilt_speed))
    }

    fn pan_tilt_reset(&mut self) -> Result<(), Error> {
        block_on(self.core().pan_tilt_reset())
    }
}
