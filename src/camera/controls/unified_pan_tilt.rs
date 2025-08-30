//! Mode-parametrized pan/tilt control trait using the Mode trait system.

use crate::{
    command::pan_tilt::{PanTilt, PanTiltDirection, PanTiltLimitCorner},
    mode::Mode,
    types::{PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed},
    units::Degrees,
    Error,
};

/// Unified pan/tilt control trait that works with both blocking and async modes.
///
/// This trait uses the Mode trait system to provide a single API surface
/// that works correctly in both blocking and async contexts. The return
/// types adapt automatically based on the Mode parameter.
///
/// # Examples
///
/// ```rust,ignore
/// use grafton_visca::{Camera, mode::{Async, Blocking}};
/// use grafton_visca::camera::controls::unified_pan_tilt::UnifiedPanTiltControl;
/// use grafton_visca::command::pan_tilt::PanTiltDirection;
/// use grafton_visca::types::{PanSpeed, TiltSpeed, SpeedLevel};
/// use grafton_visca::units::Degrees;
///
/// // Async usage
/// let async_camera: Camera<Async, Profile, Transport, Executor> = ...;
/// async_camera.pan_tilt_stop().await?; // Returns a future
/// async_camera.pan_tilt_home().await?;
///
/// // Blocking usage  
/// let blocking_camera: Camera<Blocking, Profile, Transport, ()> = ...;
/// blocking_camera.pan_tilt_stop().await?; // Returns immediately via Ready<T>
/// blocking_camera.pan_tilt_home().await?;
/// ```
pub trait UnifiedPanTiltControl<M>
where
    M: Mode,
{
    /// Stop all pan/tilt movement.
    fn pan_tilt_stop(&self) -> M::Ret<Result<(), Error>>;

    /// Move to home position (0, 0).
    fn pan_tilt_home(&self) -> M::Ret<Result<(), Error>>;

    /// Move to absolute pan/tilt position in degrees.
    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> M::Ret<Result<(), Error>>;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> M::Ret<Result<(), Error>>;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> M::Ret<Result<(), Error>>;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&self) -> M::Ret<Result<(), Error>>;

    /// Set pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> M::Ret<Result<(), Error>>;

    /// Clear pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> M::Ret<Result<(), Error>>;
}

// Implementation for the unified camera type
impl<M, P, Tr, Exec> UnifiedPanTiltControl<M> for crate::camera::unified::Camera<M, P, Tr, Exec>
where
    M: Mode + 'static,
    P: crate::capabilities::Profile + Default,
    Tr: Send + Sync,
{
    fn pan_tilt_stop(&self) -> M::Ret<Result<(), Error>> {
        let cmd = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::from(SpeedLevel::Medium),
            tilt_speed: TiltSpeed::from(SpeedLevel::Medium),
        };
        self.send_command(&cmd)
    }

    fn pan_tilt_home(&self) -> M::Ret<Result<(), Error>> {
        let cmd = PanTilt::Home;
        self.send_command(&cmd)
    }

    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> M::Ret<Result<(), Error>> {
        // Convert Degrees to Position and SpeedLevel to individual speeds
        match (
            PanPosition::from_degrees(pan.0),
            TiltPosition::from_degrees(tilt.0),
        ) {
            (Ok(pan_pos), Ok(tilt_pos)) => {
                let pan_speed = PanSpeed::from(speed);
                let tilt_speed = TiltSpeed::from(speed);
                let cmd = PanTilt::AbsolutePosition {
                    pan: pan_pos,
                    tilt: tilt_pos,
                    pan_speed,
                    tilt_speed,
                };
                self.send_command(&cmd)
            }
            (Err(e), _) | (_, Err(e)) => M::ret(Err(e)),
        }
    }

    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> M::Ret<Result<(), Error>> {
        // Convert Degrees to Position and SpeedLevel to individual speeds
        match (
            PanPosition::from_degrees(pan.0),
            TiltPosition::from_degrees(tilt.0),
        ) {
            (Ok(pan_pos), Ok(tilt_pos)) => {
                let pan_speed = PanSpeed::from(speed);
                let tilt_speed = TiltSpeed::from(speed);
                let cmd = PanTilt::RelativePosition {
                    pan: pan_pos,
                    tilt: tilt_pos,
                    pan_speed,
                    tilt_speed,
                };
                self.send_command(&cmd)
            }
            (Err(e), _) | (_, Err(e)) => M::ret(Err(e)),
        }
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> M::Ret<Result<(), Error>> {
        let cmd = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send_command(&cmd)
    }

    fn pan_tilt_reset(&self) -> M::Ret<Result<(), Error>> {
        let cmd = PanTilt::Reset;
        self.send_command(&cmd)
    }

    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> M::Ret<Result<(), Error>> {
        let cmd = PanTilt::LimitSet { corner, pan, tilt };
        self.send_command(&cmd)
    }

    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> M::Ret<Result<(), Error>> {
        let cmd = PanTilt::LimitClear { corner };
        self.send_command(&cmd)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::{Async, Blocking};

    #[tokio::test]
    async fn test_unified_pan_tilt_control_concept() {
        // This test demonstrates the concept - actual implementation would need
        // real camera instances

        // The key insight is that both async and blocking modes can be awaited:
        // - Async returns actual futures
        // - Blocking returns Ready<T> which immediately resolves

        // Example usage (conceptual):
        // async_camera.pan_tilt_stop().await?;
        // blocking_camera.pan_tilt_stop().await?;
        // Both work with the same method signature!
    }

    #[test]
    fn test_speed_level_conversion() {
        // Test that SpeedLevel can be converted to PanSpeed and TiltSpeed
        let speed = SpeedLevel::Medium;
        let pan_speed = PanSpeed::from(speed);
        let tilt_speed = TiltSpeed::from(speed);

        // These should succeed without error
        assert!(pan_speed.value() > 0);
        assert!(tilt_speed.value() > 0);
    }

    #[test]
    fn test_mode_markers_are_zero_sized() {
        use std::mem::size_of;
        assert_eq!(size_of::<Async>(), 0);
        assert_eq!(size_of::<Blocking>(), 0);
    }
}
