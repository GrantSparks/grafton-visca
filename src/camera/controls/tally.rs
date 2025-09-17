//! tally light control implementation using Mode trait.

use crate::{
    camera::ViscaClient,
    command::{
        inquiry_structs::{TallyAutoAdjustInquiry, TallyGreenInquiry, TallyStatusInquiry},
        tally::{
            TallyBrightHi, TallyBrightLo, TallyFlash, TallyGreenOff, TallyGreenOn, TallyOff,
            TallyOn, TallyRedOff, TallyRedOn,
        },
    },
    mode::Mode,
    Error,
};

/// tally light control operations for cameras.
///
/// This trait provides tally light control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait TallyControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Turn red tally light on.
    fn tally_red_on(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn red tally light off.
    fn tally_red_off(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set tally brightness to low.
    fn tally_bright_lo(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set tally brightness to high.
    fn tally_bright_hi(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn green tally light on.
    fn tally_green_on(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn green tally light off.
    fn tally_green_off(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Flash tally light.
    fn tally_flash(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn tally light on.
    fn tally_on(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn tally light off.
    fn tally_off(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Get tally light status (red and green states).
    ///
    /// Returns a TallyStatusState struct containing both red and green tally light states.
    fn tally_status(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::typed::TallyStatusState, Error>>;

    /// Query green tally light state (FR7 specific).
    ///
    /// Returns true if the green tally light is on, false otherwise.
    /// Note: This uses a special extended inquiry format (0x7E 0x04 0x1A 0x00) and is only
    /// supported on Sony FR7 cameras with dual tally lights.
    fn green_tally_status(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Check if tally auto adjust is enabled.
    ///
    /// Returns true if tally auto adjust is enabled, false otherwise.
    fn tally_auto_adjust_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> TallyControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn tally_red_on(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyRedOn::new())
    }

    fn tally_red_off(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyRedOff::new())
    }

    fn tally_bright_lo(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyBrightLo::new())
    }

    fn tally_bright_hi(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyBrightHi::new())
    }

    fn tally_green_on(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyGreenOn::new())
    }

    fn tally_green_off(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyGreenOff::new())
    }

    fn tally_flash(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyFlash::new())
    }

    fn tally_on(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyOn::new())
    }

    fn tally_off(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyOff::new())
    }

    fn tally_status(&self) -> M::Fut<'_, Result<crate::command::typed::TallyStatusState, Error>> {
        self.query(TallyStatusInquiry)
    }

    fn green_tally_status(&self) -> M::Fut<'_, Result<bool, Error>> {
        self.query(TallyGreenInquiry)
    }

    fn tally_auto_adjust_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        self.query(TallyAutoAdjustInquiry)
    }
}
