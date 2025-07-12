//! Preset methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{Presets, ProfileMetadata, ValidationError},
    command::{
        preset::{PresetAction, PresetCommand, PresetNumber},
        Response,
    },
    transport::core::{BlockingTransport, Transport},
    Error,
};
use core::future::Future;

/// Extension trait for CameraCore - provides future-returning methods.
pub trait PresetsCoreExt<P, T>
where
    P: ProfileMetadata + Presets,
    T: Transport,
{
    /// Recall a preset position - returns a future.
    fn preset_recall(&self, preset: PresetNumber) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set current position as a preset - returns a future.
    fn preset_set(&self, preset: PresetNumber) -> impl Future<Output = Result<(), Error>> + '_;
}

#[allow(clippy::manual_async_fn)]
impl<P, T> PresetsCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + Presets,
    T: Transport,
{
    fn preset_recall(&self, preset: PresetNumber) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let preset_number = preset;
            
            // Validate preset number (0 is valid - it's the home position)
            if preset_number.value() > P::MAX_PRESETS {
                return Err(Error::ValidationError(ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!("Preset {} is invalid, must be 0-{}", preset_number.value(), P::MAX_PRESETS),
                }));
            }

            let command = PresetCommand {
                action: PresetAction::Recall,
                preset_number,
            };
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn preset_set(&self, preset: PresetNumber) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            let preset_number = preset;
            
            // Validate preset number (0 is valid - it's the home position)
            if preset_number.value() > P::MAX_PRESETS {
                return Err(Error::ValidationError(ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!("Preset {} is invalid, must be 0-{}", preset_number.value(), P::MAX_PRESETS),
                }));
            }

            let command = PresetCommand {
                action: PresetAction::Set,
                preset_number,
            };
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }
}

/// Extension trait for async Camera facade.
pub trait PresetsAsyncExt<P, T>
where
    P: ProfileMetadata + Presets,
    T: Transport,
{
    /// Recall a preset position.
    fn preset_recall(&self, preset: PresetNumber) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set current position as a preset.
    fn preset_set(&self, preset: PresetNumber) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> PresetsAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + Presets + Sync,
    T: Transport + Sync,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn preset_recall(&self, preset: PresetNumber) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().preset_recall(preset).await }
    }

    fn preset_set(&self, preset: PresetNumber) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().preset_set(preset).await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait PresetsBlockingExt<P, T>
where
    P: ProfileMetadata + Presets,
    T: BlockingTransport,
{
    /// Recall a preset position.
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error>;

    /// Set current position as a preset.
    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error>;
}

impl<P, T> PresetsBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + Presets,
    T: BlockingTransport,
{
    fn preset_recall(&self, preset: PresetNumber) -> Result<(), Error> {
        block_on(self.core().preset_recall(preset))
    }

    fn preset_set(&self, preset: PresetNumber) -> Result<(), Error> {
        block_on(self.core().preset_set(preset))
    }
}
