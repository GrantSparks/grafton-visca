//! power control implementation using Mode trait.

use crate::{camera::ViscaClient, mode::Mode, Error};

/// power operations for cameras.
///
/// This trait provides power control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait PowerControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Power on the camera.
    fn power_on(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Power off the camera.
    fn power_off(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> PowerControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn power_on(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::power::PowerOn;
        self.execute(PowerOn::new())
    }

    fn power_off(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::power::PowerStandby;
        self.execute(PowerStandby::new())
    }
}
