//! Preset methods for cameras using the new GAT architecture.

use crate::{command::preset::PresetNumber, Error};

/// Presets operations (async).
#[cfg(feature = "async")]
pub trait PresetsOps: Sized {
    /// Recall a preset position.
    async fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    async fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Reset/clear a preset.
    async fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error>;
}

/// Presets operations (blocking).
#[cfg(not(feature = "async"))]
pub trait PresetsOpsBlocking: Sized {
    /// Recall a preset position.
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Reset/clear a preset.
    fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> PresetsOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
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
impl<P, T> PresetsOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let cmd = PresetCommand {
            action: PresetAction::Recall,
            preset_number: preset,
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let cmd = PresetCommand {
            action: PresetAction::Set,
            preset_number: preset,
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn preset_reset(&self, preset: PresetNumber) -> Result<(), Error> {
        use crate::command::preset::{PresetAction, PresetCommand};
        let cmd = PresetCommand {
            action: PresetAction::Reset,
            preset_number: preset,
        };
        self.send_command(&cmd)?;
        Ok(())
    }
}
