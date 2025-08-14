//! Pan/Tilt methods for cameras using the new GAT architecture.

use crate::{
    command::pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
    types::{PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed},
    units::Degrees,
    Error,
};

/// Pan/Tilt operations (async).
#[cfg(feature = "async")]
pub trait PanTiltOps: Sized {
    /// Stop all pan/tilt movement.
    async fn pan_tilt_stop(&self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    async fn pan_tilt_home(&self) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    async fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    async fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    async fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    async fn pan_tilt_reset(&self) -> Result<(), Error>;

    /// Set pan/tilt movement limit for a specific corner.
    async fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error>;

    /// Clear pan/tilt movement limit for a specific corner.
    async fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> Result<(), Error>;
}

/// Pan/Tilt operations (blocking).
#[cfg(not(feature = "async"))]
pub trait PanTiltOpsBlocking: Sized {
    /// Stop all pan/tilt movement.
    fn pan_tilt_stop(&self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    fn pan_tilt_home(&self) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&self) -> Result<(), Error>;

    /// Set pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error>;

    /// Clear pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> PanTiltOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn pan_tilt_stop(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::from(SpeedLevel::Medium),
            tilt_speed: TiltSpeed::from(SpeedLevel::Medium),
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_home(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Home;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::AbsolutePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed,
            tilt_speed,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::RelativePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed,
            tilt_speed,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_reset(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Reset;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::LimitSet { corner, pan, tilt };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::LimitClear { corner };
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> PanTiltOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn pan_tilt_stop(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::from(SpeedLevel::Medium),
            tilt_speed: TiltSpeed::from(SpeedLevel::Medium),
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_home(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Home;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::AbsolutePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed,
            tilt_speed,
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = PanPosition::from_degrees(pan.0)?;
        let tilt_pos = TiltPosition::from_degrees(tilt.0)?;
        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::RelativePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed,
            tilt_speed,
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_reset(&self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::LimitSet { corner, pan, tilt };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::LimitClear { corner };
        self.send_command(&cmd)?;
        Ok(())
    }
}
