//! Pan/Tilt methods for unified camera API.

// Local imports
use crate::{
    command::pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
    types::{PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed},
    units::Degrees,
    Error,
};

/// Pan/Tilt operations for cameras.
///
/// This trait provides pan/tilt control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait PanTiltControl {
    /// Stop all pan/tilt movement.
    #[cfg(feature = "async")]
    fn pan_tilt_stop(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Stop all pan/tilt movement.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_stop(&mut self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    #[cfg(feature = "async")]
    fn pan_tilt_home(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Move to home position (0, 0).
    #[cfg(not(feature = "async"))]
    fn pan_tilt_home(&mut self) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    #[cfg(feature = "async")]
    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Move to absolute pan/tilt position in degrees.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_absolute(
        &mut self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    #[cfg(feature = "async")]
    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Move relative to current position in degrees.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_relative(
        &mut self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    #[cfg(feature = "async")]
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Move pan/tilt in a specific direction.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_move(
        &mut self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    #[cfg(feature = "async")]
    fn pan_tilt_reset(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset pan/tilt to default position.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_reset(&mut self) -> Result<(), Error>;

    /// Set pan/tilt movement limit for a specific corner.
    #[cfg(feature = "async")]
    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set pan/tilt movement limit for a specific corner.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_limit_set(
        &mut self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error>;

    /// Clear pan/tilt movement limit for a specific corner.
    #[cfg(feature = "async")]
    fn pan_tilt_limit_clear(
        &self,
        corner: PanTiltLimitCorner,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Clear pan/tilt movement limit for a specific corner.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_limit_clear(&mut self, corner: PanTiltLimitCorner) -> Result<(), Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async pan/tilt control trait (deprecated, use PanTiltControl instead).
/// Blocking pan/tilt control trait (deprecated, use PanTiltControl instead).
// Async implementation for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> PanTiltControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> PanTiltControl for crate::camera::BlockingCamera<P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::BlockingTransport + Send + 'static,
{
    fn pan_tilt_stop(&mut self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::from(SpeedLevel::Medium),
            tilt_speed: TiltSpeed::from(SpeedLevel::Medium),
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_home(&mut self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;

        let cmd = PanTilt::Home;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_absolute(
        &mut self,
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
        &mut self,
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
        &mut self,
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

    fn pan_tilt_reset(&mut self) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;

        let cmd = PanTilt::Reset;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_limit_set(
        &mut self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;

        let cmd = PanTilt::LimitSet { corner, pan, tilt };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn pan_tilt_limit_clear(&mut self, corner: PanTiltLimitCorner) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTilt;

        let cmd = PanTilt::LimitClear { corner };
        self.send_command(&cmd)?;
        Ok(())
    }
}
