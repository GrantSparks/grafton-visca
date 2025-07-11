//! Preset methods for cameras using the new GAT architecture.

use crate::{
    blocking::block_on,
    camera::{async_facade::CameraAsync, blocking_facade::CameraBlocking, core::CameraCore},
    capabilities::{Presets, ProfileMetadata, ValidationError},
    command::{
        const_encoding::{commands, CommandBuilder},
        Command, Response, ResponseType,
    },
    transport::gat_transport::Transport,
    Error,
};
use core::future::Future;

/// Preset recall command.
struct PresetRecallCommand([u8; 7]);

impl PresetRecallCommand {
    fn new(preset: u8) -> Self {
        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(commands::PRESET_RECALL_PREFIX).push(preset);
        Self(cmd.build())
    }
}

impl Command for PresetRecallCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Preset set command.
struct PresetSetCommand([u8; 7]);

impl PresetSetCommand {
    fn new(preset: u8) -> Self {
        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(commands::PRESET_SET_PREFIX).push(preset);
        Self(cmd.build())
    }
}

impl Command for PresetSetCommand {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        Ok(self.0.to_vec())
    }

    fn response_type(&self) -> Option<ResponseType> {
        None // Action command
    }
}

/// Extension trait for CameraCore - provides future-returning methods.
pub trait PresetsCoreExt<P, T>
where
    P: ProfileMetadata + Presets,
    T: Transport,
{
    /// Recall a preset position - returns a future.
    fn preset_recall(&self, preset: u8) -> impl Future<Output = Result<(), Error>> + '_;

    /// Set current position as a preset - returns a future.
    fn preset_set(&self, preset: u8) -> impl Future<Output = Result<(), Error>> + '_;
}

impl<P, T> PresetsCoreExt<P, T> for CameraCore<P, T>
where
    P: ProfileMetadata + Presets,
    T: Transport,
{
    fn preset_recall(&self, preset: u8) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            // Validate preset number (0 is valid - it's the home position)
            if preset > P::MAX_PRESETS {
                return Err(Error::ValidationError(ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!("Preset {} is invalid, must be 0-{}", preset, P::MAX_PRESETS),
                }));
            }

            let command = PresetRecallCommand::new(preset);
            let response = self.send_command(&command).await?;
            match response {
                Response::Completion => Ok(()),
                Response::Error(e) => Err(e),
                _ => Err(Error::UnexpectedResponseType),
            }
        }
    }

    fn preset_set(&self, preset: u8) -> impl Future<Output = Result<(), Error>> + '_ {
        async move {
            // Validate preset number (0 is valid - it's the home position)
            if preset > P::MAX_PRESETS {
                return Err(Error::ValidationError(ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!("Preset {} is invalid, must be 0-{}", preset, P::MAX_PRESETS),
                }));
            }

            let command = PresetSetCommand::new(preset);
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
    fn preset_recall(&self, preset: u8) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set current position as a preset.
    fn preset_set(&self, preset: u8) -> impl Future<Output = Result<(), Error>> + Send;
}

impl<P, T> PresetsAsyncExt<P, T> for CameraAsync<P, T>
where
    P: ProfileMetadata + Presets,
    T: Transport,
{
    fn preset_recall(&self, preset: u8) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().preset_recall(preset).await }
    }

    fn preset_set(&self, preset: u8) -> impl Future<Output = Result<(), Error>> + Send {
        async move { self.core().preset_set(preset).await }
    }
}

/// Extension trait for blocking Camera facade.
pub trait PresetsBlockingExt<P, T>
where
    P: ProfileMetadata + Presets,
    T: Transport,
{
    /// Recall a preset position.
    fn preset_recall(&self, preset: u8) -> Result<(), Error>;

    /// Set current position as a preset.
    fn preset_set(&self, preset: u8) -> Result<(), Error>;
}

impl<P, T> PresetsBlockingExt<P, T> for CameraBlocking<P, T>
where
    P: ProfileMetadata + Presets,
    T: Transport,
{
    fn preset_recall(&self, preset: u8) -> Result<(), Error> {
        block_on(self.core().preset_recall(preset))
    }

    fn preset_set(&self, preset: u8) -> Result<(), Error> {
        block_on(self.core().preset_set(preset))
    }
}
