//! Unified tally light control implementation using Mode trait.

use crate::{camera::CameraSend, mode::Mode, Error};

/// Unified tally light control operations for cameras.
///
/// This trait provides tally light control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
pub trait TallyControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Turn red tally light on.
    fn tally_red_on(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Turn red tally light off.
    fn tally_red_off(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set tally brightness to low.
    fn tally_bright_lo(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set tally brightness to high.
    fn tally_bright_hi(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Turn green tally light on.
    fn tally_green_on(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Turn green tally light off.
    fn tally_green_off(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Flash tally light.
    fn tally_flash(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Turn tally light on.
    fn tally_on(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Turn tally light off.
    fn tally_off(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Get tally light status.
    fn get_tally_status(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Query red tally light state.
    fn get_red_tally_status(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;

    /// Query green tally light state (FR7 specific).
    fn get_green_tally_status(&self) -> <Self::Mode as Mode>::Ret<'_, Result<bool, Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> TallyControl for crate::camera::UnifiedCamera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: CameraSend<M>,
{
    type Mode = M;

    fn tally_red_on(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::RedOn)
    }

    fn tally_red_off(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::RedOff)
    }

    fn tally_bright_lo(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::BrightLo)
    }

    fn tally_bright_hi(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::BrightHi)
    }

    fn tally_green_on(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::GreenOn)
    }

    fn tally_green_off(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::GreenOff)
    }

    fn tally_flash(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::Flash)
    }

    fn tally_on(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::On)
    }

    fn tally_off(&self) -> M::Ret<'_, Result<(), Error>> {
        use crate::command::tally::Tally;
        self.send_and_complete(Tally::Off)
    }

    fn get_tally_status(&self) -> M::Ret<'_, Result<bool, Error>> {
        // For now, return an error since proper inquiry implementation needs more work
        self.error(Error::Unsupported)
    }

    fn get_red_tally_status(&self) -> M::Ret<'_, Result<bool, Error>> {
        // For now, return an error since proper inquiry implementation needs more work
        self.error(Error::Unsupported)
    }

    fn get_green_tally_status(&self) -> M::Ret<'_, Result<bool, Error>> {
        // For now, return an error since proper inquiry implementation needs more work
        self.error(Error::Unsupported)
    }
}
