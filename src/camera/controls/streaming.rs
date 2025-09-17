//! streaming control implementation using Mode trait.

use crate::{camera::ViscaClient, mode::Mode, types::NdiQuality, Error};

/// streaming operations for cameras.
///
/// This trait provides streaming control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait StreamingControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable multicast streaming for Ndi cameras.
    fn enable_multicast(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable multicast streaming for Ndi cameras.
    fn disable_multicast(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set the Ndi streaming quality.
    fn set_ndi_quality(
        &self,
        quality: NdiQuality,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> StreamingControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_multicast(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::streaming::MulticastStreaming;
        self.execute(MulticastStreaming::On)
    }

    fn disable_multicast(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::streaming::MulticastStreaming;
        self.execute(MulticastStreaming::Off)
    }

    fn set_ndi_quality(&self, quality: NdiQuality) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::streaming::SetNdiQuality;
        self.execute(SetNdiQuality::new(quality))
    }
}
