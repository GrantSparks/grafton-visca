//! power control implementation using Mode trait.

use crate::{camera::CommandClient, mode::Mode, Error};

/// power operations for cameras.
///
/// This trait provides power control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait PowerControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Power on the camera.
    fn power_on(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Power off the camera.
    fn power_off(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> PowerControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CommandClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn power_on(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::power::PowerOn;
        self.send_and_complete(PowerOn::new())
    }

    fn power_off(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::power::PowerStandby;
        self.send_and_complete(PowerStandby::new())
    }
}
