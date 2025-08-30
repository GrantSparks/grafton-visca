//! Mode-parametrized zoom control trait using the Mode trait system.

use crate::{
    command::zoom::{Zoom, ZoomSpeed},
    mode::Mode,
    types::ZoomPosition,
    units::Normalized,
    Error,
};

/// Unified zoom control trait that works with both blocking and async modes.
///
/// This trait uses the Mode trait system to provide a single API surface
/// that works correctly in both blocking and async contexts. The return
/// types adapt automatically based on the Mode parameter.
///
/// # Examples
///
/// ```rust,ignore
/// use grafton_visca::{Camera, mode::{Async, Blocking}};
/// use grafton_visca::camera::controls::unified_zoom::UnifiedZoomControl;
/// use grafton_visca::command::zoom::ZoomSpeed;
/// use grafton_visca::units::Normalized;
///
/// // Async usage
/// let async_camera: Camera<Async, Profile, Transport, Executor> = ...;
/// async_camera.zoom_stop().await?; // Returns a future
/// async_camera.zoom_tele_variable(ZoomSpeed::new(3)?).await?;
///
/// // Blocking usage  
/// let blocking_camera: Camera<Blocking, Profile, Transport, ()> = ...;
/// blocking_camera.zoom_stop().await?; // Returns immediately via Ready<T>
/// blocking_camera.zoom_tele_variable(ZoomSpeed::new(3)?).await?;
/// ```
pub trait UnifiedZoomControl<M>
where
    M: Mode,
{
    /// Stop zooming.
    fn zoom_stop(&self) -> M::Ret<Result<(), Error>>;

    /// Start zooming in at standard speed.
    fn zoom_tele_std(&self) -> M::Ret<Result<(), Error>>;

    /// Start zooming out at standard speed.
    fn zoom_wide_std(&self) -> M::Ret<Result<(), Error>>;

    /// Start zooming in at variable speed.
    fn zoom_tele_variable(&self, speed: ZoomSpeed) -> M::Ret<Result<(), Error>>;

    /// Start zooming out at variable speed.
    fn zoom_wide_variable(&self, speed: ZoomSpeed) -> M::Ret<Result<(), Error>>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    fn zoom_absolute(&self, position: Normalized) -> M::Ret<Result<(), Error>>;

    /// Set zoom to a specific position value.
    fn zoom_position(&self, position: ZoomPosition) -> M::Ret<Result<(), Error>>;

    /// Query the current zoom position.
    fn zoom_position_inquiry(&self) -> M::Ret<Result<ZoomPosition, Error>>;
}

// Implementation for the unified camera type
impl<M, P, Tr, Exec> UnifiedZoomControl<M> for crate::camera::unified::Camera<M, P, Tr, Exec>
where
    M: Mode + 'static,
    P: crate::capabilities::Profile + Default,
    Tr: Send + Sync,
{
    fn zoom_stop(&self) -> M::Ret<Result<(), Error>> {
        let cmd = Zoom::Stop;
        self.send_command(&cmd)
    }

    fn zoom_tele_std(&self) -> M::Ret<Result<(), Error>> {
        let cmd = Zoom::TeleStd;
        self.send_command(&cmd)
    }

    fn zoom_wide_std(&self) -> M::Ret<Result<(), Error>> {
        let cmd = Zoom::WideStd;
        self.send_command(&cmd)
    }

    fn zoom_tele_variable(&self, speed: ZoomSpeed) -> M::Ret<Result<(), Error>> {
        let cmd = Zoom::TeleVariable(speed);
        self.send_command(&cmd)
    }

    fn zoom_wide_variable(&self, speed: ZoomSpeed) -> M::Ret<Result<(), Error>> {
        let cmd = Zoom::WideVariable(speed);
        self.send_command(&cmd)
    }

    fn zoom_absolute(&self, position: Normalized) -> M::Ret<Result<(), Error>> {
        // Convert normalized position to zoom position value
        match ZoomPosition::try_from(*position.value()) {
            Ok(zoom_pos) => {
                let cmd = Zoom::Position(zoom_pos);
                self.send_command(&cmd)
            }
            Err(e) => M::ret(Err(e)),
        }
    }

    fn zoom_position(&self, position: ZoomPosition) -> M::Ret<Result<(), Error>> {
        let cmd = Zoom::Position(position);
        self.send_command(&cmd)
    }

    fn zoom_position_inquiry(&self) -> M::Ret<Result<ZoomPosition, Error>> {
        // TODO: Connect to actual command execution system with inquiry commands
        // For now, return a default zoom position
        match ZoomPosition::try_from(0x0000) {
            Ok(pos) => M::ret(Ok(pos)),
            Err(e) => M::ret(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::{Async, Blocking};

    #[tokio::test]
    async fn test_unified_zoom_control_concept() {
        // This test demonstrates the concept - actual implementation would need
        // real camera instances

        // The key insight is that both async and blocking modes can be awaited:
        // - Async returns actual futures
        // - Blocking returns Ready<T> which immediately resolves

        // Example usage (conceptual):
        // async_camera.zoom_stop().await?;
        // blocking_camera.zoom_stop().await?;
        // Both work with the same method signature!
    }

    #[test]
    fn test_zoom_speed_construction() {
        // Test that ZoomSpeed can be constructed properly
        let speed = ZoomSpeed::new(3);
        assert!(speed.is_ok());

        let speed = ZoomSpeed::new(8); // Should fail - max is 7
        assert!(speed.is_err());
    }

    #[test]
    fn test_mode_markers_are_zero_sized() {
        use std::mem::size_of;
        assert_eq!(size_of::<Async>(), 0);
        assert_eq!(size_of::<Blocking>(), 0);
    }
}
