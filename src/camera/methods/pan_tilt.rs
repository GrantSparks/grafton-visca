//! Pan/Tilt methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{pan_tilt::PanTiltExt, PanTilt, ProfileMetadata},
    command::{
        pan_tilt::{PanTiltCommand, PanTiltDirection},
        Response,
    },
    transport::core::{BlockingTransport, Transport},
    types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed, SpeedLevel},
    units::Degrees,
    Error,
};
use core::future::Future;


/// Extension trait for CameraCore that adds pan/tilt methods.
pub trait PanTiltCoreExt<P: ProfileMetadata + PanTilt> {
    /// Stop all pan/tilt movement.
    fn pan_tilt_stop(&self) -> impl Future<Output = Result<(), Error>>;

    /// Move to home position (0, 0).
    fn pan_tilt_home(&self) -> impl Future<Output = Result<(), Error>>;

    /// Move to absolute pan/tilt position in degrees.
    fn pan_tilt_absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> impl Future<Output = Result<(), Error>>;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&self) -> impl Future<Output = Result<(), Error>>;
}

#[allow(clippy::manual_async_fn)]
impl<P, T> PanTiltCoreExt<P> for CameraCore<P, T>
where
    P: ProfileMetadata + PanTilt + Default,
    T: Transport,
{
    fn pan_tilt_stop(&self) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0).unwrap(),
                tilt_speed: TiltSpeed::new(0).unwrap(),
            };
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn pan_tilt_home(&self) -> impl Future<Output = Result<(), Error>> {
        async move {
            let cmd = PanTiltCommand::Home;
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn pan_tilt_absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            // Use inputs directly
            let pan_degrees = pan;
            let tilt_degrees = tilt;
            let speed_level = speed;

            // Create a dummy profile instance to use extension trait methods
            let profile = P::default();

            // Use the extension trait to convert degrees to units
            let pan = profile.degrees_to_pan_units(pan_degrees.0);
            let tilt = profile.degrees_to_tilt_units(tilt_degrees.0);

            // Validate using profile constants
            let pan = profile.validate_pan(pan)?;
            let tilt = profile.validate_tilt(tilt)?;
            
            // Convert SpeedLevel to specific pan/tilt speeds
            let pan_speed_val = speed_level.to_pan_speed();
            let tilt_speed_val = speed_level.to_tilt_speed();

            let pan_pos = PanPosition::new(pan)?;
            let tilt_pos = TiltPosition::new(tilt)?;
            let pan_spd = PanSpeed::new(pan_speed_val)?;
            let tilt_spd = TiltSpeed::new(tilt_speed_val)?;
            
            let cmd = PanTiltCommand::AbsolutePosition {
                pan: pan_pos,
                tilt: tilt_pos,
                pan_speed: pan_spd,
                tilt_speed: tilt_spd,
            };
            match self.send_command(&cmd).await? {
                Response::Ack => Ok(()),
                Response::Error(e) => Err(e.into()),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn pan_tilt_relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            // Use inputs directly
            let pan_degrees = pan;
            let tilt_degrees = tilt;
            let speed_level = speed;

            let profile = P::default();
            let pan = profile.degrees_to_pan_units(pan_degrees.0);
            let tilt = profile.degrees_to_tilt_units(tilt_degrees.0);
            
            // Convert SpeedLevel to specific pan/tilt speeds
            let pan_speed_val = speed_level.to_pan_speed();
            let tilt_speed_val = speed_level.to_tilt_speed();

            let pan_pos = PanPosition::new(pan)?;
            let tilt_pos = TiltPosition::new(tilt)?;
            let pan_spd = PanSpeed::new(pan_speed_val)?;
            let tilt_spd = TiltSpeed::new(tilt_speed_val)?;
            
            let cmd = PanTiltCommand::RelativePosition {
                pan: pan_pos,
                tilt: tilt_pos,
                pan_speed: pan_spd,
                tilt_speed: tilt_spd,
            };
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
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> impl Future<Output = Result<(), Error>> {
        async move {
            let pan_spd = pan_speed;
            let tilt_spd = tilt_speed;
            
            let cmd = PanTiltCommand::Move {
                direction,
                pan_speed: pan_spd,
                tilt_speed: tilt_spd,
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
            let cmd = PanTiltCommand::Reset;
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
    fn pan_tilt_stop(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Move to home position (0, 0).
    fn pan_tilt_home(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Move to absolute pan/tilt position in degrees.
    fn pan_tilt_absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> impl Future<Output = Result<(), Error>> + Send;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&self) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> PanTiltAsyncExt<P> for CameraAsync<P, T>
where
    P: ProfileMetadata + PanTilt + Default + Sync + Send,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn pan_tilt_stop(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().pan_tilt_stop().await }
    }

    fn pan_tilt_home(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().pan_tilt_home().await }
    }

    fn pan_tilt_absolute(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            self.core()
                .pan_tilt_absolute(pan, tilt, speed)
                .await
        }
    }

    fn pan_tilt_relative(
        &self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            self.core()
                .pan_tilt_relative(pan, tilt, speed)
                .await
        }
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> impl Future<Output = Result<(), Error>> + Send {
        async move {
            self.core()
                .pan_tilt_move(direction, pan_speed, tilt_speed)
                .await
        }
    }

    fn pan_tilt_reset(&self) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().pan_tilt_reset().await }
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
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &mut self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&mut self) -> Result<(), Error>;
}

impl<P, T> PanTiltBlockingExt<P> for CameraBlocking<P, T>
where
    P: ProfileMetadata + PanTilt + Default,
    T: BlockingTransport,
{
    fn pan_tilt_stop(&mut self) -> Result<(), Error> {
        block_on(self.core().pan_tilt_stop())
    }

    fn pan_tilt_home(&mut self) -> Result<(), Error> {
        block_on(self.core().pan_tilt_home())
    }

    fn pan_tilt_absolute(
        &mut self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        block_on(
            self.core()
                .pan_tilt_absolute(pan, tilt, speed),
        )
    }

    fn pan_tilt_relative(
        &mut self,
        pan: Degrees<f32>,
        tilt: Degrees<f32>,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        block_on(
            self.core()
                .pan_tilt_relative(pan, tilt, speed),
        )
    }

    fn pan_tilt_move(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        block_on(self.core().pan_tilt_move(direction, pan_speed, tilt_speed))
    }

    fn pan_tilt_reset(&mut self) -> Result<(), Error> {
        block_on(self.core().pan_tilt_reset())
    }
}
