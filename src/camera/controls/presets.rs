//! preset control implementation using Mode trait.

use crate::{camera::ViscaClient, command::preset::PresetNumber, mode::Mode, Error};

/// presets operations for cameras.
///
/// This trait provides preset control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait PresetsControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Recall a preset position.
    fn preset_recall(
        &self,
        preset: PresetNumber,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set current position as a preset.
    fn preset_set(&self, preset: PresetNumber) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Reset/clear a preset.
    fn preset_reset(
        &self,
        preset: PresetNumber,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> PresetsControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn preset_recall(&self, preset: PresetNumber) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset,
        };
        self.execute(cmd)
    }

    fn preset_set(&self, preset: PresetNumber) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: preset,
        };
        self.execute(cmd)
    }

    fn preset_reset(&self, preset: PresetNumber) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let cmd = PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset,
        };
        self.execute(cmd)
    }
}
