//! Unified zoom control implementation using Mode trait.

use crate::{camera::CameraSend, command::zoom::ZoomSpeed, mode::Mode, units::Normalized, Error};

/// Unified zoom operations for cameras.
///
/// This trait provides zoom control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::forward_control_to_session]
pub trait ZoomControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Stop zooming.
    fn zoom_stop(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Start zooming in at standard speed.
    fn zoom_tele_std(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Start zooming out at standard speed.
    fn zoom_wide_std(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Start zooming in at variable speed.
    fn zoom_tele_variable(
        &self,
        speed: ZoomSpeed,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Start zooming out at variable speed.
    fn zoom_wide_variable(
        &self,
        speed: ZoomSpeed,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set zoom to absolute position (0.0 = wide, 1.0 = full tele).
    fn zoom_absolute(
        &self,
        position: Normalized,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set zoom to a specific position value.
    fn zoom_position(
        &self,
        position: crate::types::ZoomPosition,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set digital zoom on or off.
    ///
    /// When enabled, zoom can continue past the optical zoom limit using digital processing.
    ///
    /// # Arguments
    /// * `enabled` - true to enable digital zoom, false to disable
    fn set_digital_zoom(&self, enabled: bool) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> ZoomControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::zoom::Zoom,
    Self: CameraSend<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn zoom_stop(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.send_and_complete(Zoom::Stop)
    }

    fn zoom_tele_std(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.send_and_complete(Zoom::TeleStd)
    }

    fn zoom_wide_std(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.send_and_complete(Zoom::WideStd)
    }

    fn zoom_tele_variable(&self, speed: ZoomSpeed) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.send_and_complete(Zoom::TeleVariable(speed))
    }

    fn zoom_wide_variable(&self, speed: ZoomSpeed) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.send_and_complete(Zoom::WideVariable(speed))
    }

    fn zoom_absolute(&self, position: Normalized) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        match crate::types::ZoomPosition::try_from(*position.value()) {
            Ok(zoom_pos) => self.send_and_complete(Zoom::Position(zoom_pos)),
            Err(e) => self.error(e),
        }
    }

    fn zoom_position(&self, position: crate::types::ZoomPosition) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::zoom::Zoom;
        self.send_and_complete(Zoom::Position(position))
    }

    fn set_digital_zoom(&self, enabled: bool) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::zoom::DigitalZoom;
        self.send_and_complete(DigitalZoom::new(enabled))
    }
}
