//! Preset methods for cameras using the new GAT architecture.

use crate::{command::preset::PresetNumber, Error};

/// Presets operations (async).
#[cfg(feature = "async")]
pub trait PresetsControl: Send + Sync + 'static + Sized {
    /// Recall a preset position.
    fn preset_recall(
        &self,
        preset: PresetNumber,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set current position as a preset.
    fn preset_set(
        &self,
        preset: PresetNumber,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Reset/clear a preset.
    fn preset_reset(
        &self,
        preset: PresetNumber,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;
}

/// Presets operations (blocking).
pub trait PresetsControlBlocking: Sized {
    /// Recall a preset position.
    fn preset_recall(&mut self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    fn preset_set(&mut self, preset: PresetNumber) -> Result<(), Error>;

    /// Reset/clear a preset.
    fn preset_reset(&mut self, preset: PresetNumber) -> Result<(), Error>;
}

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

// Blocking implementation for Camera with BlockingMode
#[cfg(not(feature = "async"))]
impl<P, T> PresetsControlBlocking for crate::camera::BlockingCamera<P, T>
where
    P: crate::capabilities::Profile + Default,
    T: crate::transport::BlockingTransport + Send + 'static,
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
