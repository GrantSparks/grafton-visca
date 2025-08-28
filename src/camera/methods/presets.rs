//! Preset methods for cameras.

use crate::{command::preset::PresetNumber, Error};

/// Presets operations for cameras.
///
/// This trait provides preset control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait PresetsControl {
    /// Recall a preset position.
    #[cfg(feature = "async")]
    fn preset_recall(
        &self,
        preset: PresetNumber,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Recall a preset position.
    #[cfg(not(feature = "async"))]
    fn preset_recall(&mut self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    #[cfg(feature = "async")]
    fn preset_set(
        &self,
        preset: PresetNumber,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set current position as a preset.
    #[cfg(not(feature = "async"))]
    fn preset_set(&mut self, preset: PresetNumber) -> Result<(), Error>;

    /// Reset/clear a preset.
    #[cfg(feature = "async")]
    fn preset_reset(
        &self,
        preset: PresetNumber,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset/clear a preset.
    #[cfg(not(feature = "async"))]
    fn preset_reset(&mut self, preset: PresetNumber) -> Result<(), Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async presets control trait (deprecated, use PresetsControl instead).
/// Blocking presets control trait (deprecated, use PresetsControl instead).
// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, Tr, Exec> PresetsControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn preset_set(&self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};

        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: preset,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};

        let cmd = PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> PresetsControl for crate::camera::BlockingCamera<P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::BlockingTransport + Send + 'static,
{
    fn preset_recall(&mut self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset,
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn preset_set(&mut self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};

        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: preset,
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn preset_reset(&mut self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};

        let cmd = PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset,
        };
        self.send_command(&cmd)?;
        Ok(())
    }
}
