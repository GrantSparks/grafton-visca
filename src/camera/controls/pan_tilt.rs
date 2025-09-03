//! Unified pan/tilt control implementation using Mode trait.

use crate::{
    camera::CameraSend,
    command::pan_tilt::{PanTiltDirection, PanTiltLimitCorner},
    mode::Mode,
    types::{PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed},
    units::Degrees,
    Error,
};

/// Unified pan/tilt operations for cameras.
///
/// This trait provides pan/tilt control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
pub trait PanTiltControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Stop all pan/tilt movement.
    fn pan_tilt_stop(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Move to home position (0, 0).
    fn pan_tilt_home(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Move to absolute pan/tilt position in degrees.
    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Clear pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_clear(
        &self,
        corner: PanTiltLimitCorner,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> PanTiltControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CameraSend<M>,
{
    type Mode = M;

    fn pan_tilt_stop(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::from(SpeedLevel::Medium),
            tilt_speed: TiltSpeed::from(SpeedLevel::Medium),
        };
        self.send_and_complete(cmd)
    }

    fn pan_tilt_home(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        self.send_and_complete(PanTilt::Home)
    }

    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;

        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = match PanPosition::from_degrees(pan.0) {
            Ok(pos) => pos,
            Err(e) => return self.error(e),
        };
        let tilt_pos = match TiltPosition::from_degrees(tilt.0) {
            Ok(pos) => pos,
            Err(e) => return self.error(e),
        };
        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::AbsolutePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed,
            tilt_speed,
        };
        self.send_and_complete(cmd)
    }

    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;

        // Convert Degrees to Position and SpeedLevel to individual speeds
        let pan_pos = match PanPosition::from_degrees(pan.0) {
            Ok(pos) => pos,
            Err(e) => return self.error(e),
        };
        let tilt_pos = match TiltPosition::from_degrees(tilt.0) {
            Ok(pos) => pos,
            Err(e) => return self.error(e),
        };
        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);
        let cmd = PanTilt::RelativePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed,
            tilt_speed,
        };
        self.send_and_complete(cmd)
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send_and_complete(cmd)
    }

    fn pan_tilt_reset(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        self.send_and_complete(PanTilt::Reset)
    }

    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::LimitSet { corner, pan, tilt };
        self.send_and_complete(cmd)
    }

    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::pan_tilt::PanTilt;
        let cmd = PanTilt::LimitClear { corner };
        self.send_and_complete(cmd)
    }
}
