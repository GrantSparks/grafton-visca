//! Unified streaming control implementation using Mode trait.

use crate::{camera::CameraSend, mode::Mode, types::NdiQuality, Error};

/// Unified streaming operations for cameras.
///
/// This trait provides streaming control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::forward_control_to_session]
pub trait StreamingControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable multicast streaming for Ndi cameras.
    fn enable_multicast(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Disable multicast streaming for Ndi cameras.
    fn disable_multicast(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set the Ndi streaming quality.
    fn set_ndi_quality(
        &self,
        quality: NdiQuality,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> StreamingControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CameraSend<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_multicast(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::streaming::MulticastStreaming;
        self.send_and_complete(MulticastStreaming::On)
    }

    fn disable_multicast(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::streaming::MulticastStreaming;
        self.send_and_complete(MulticastStreaming::Off)
    }

    fn set_ndi_quality(&self, quality: NdiQuality) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::streaming::NdiQualityCommand;
        self.send_and_complete(NdiQualityCommand::new(quality))
    }
}
