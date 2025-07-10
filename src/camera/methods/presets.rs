//! Preset methods for cameras that support preset positions.

use crate::camera::Camera;
use crate::capabilities::{ProfileMetadata, SupportsPresets};
use crate::Error;

/// Extension trait that adds preset methods to cameras.
#[allow(async_fn_in_trait)]
pub trait PresetMethods {
    /// Recall a preset position.
    #[cfg(not(feature = "async"))]
    fn preset_recall(&mut self, preset: u8) -> Result<(), Error>;

    /// Recall a preset position.
    #[cfg(feature = "async")]
    async fn preset_recall(&self, preset: u8) -> Result<(), Error>;

    /// Set current position as a preset.
    #[cfg(not(feature = "async"))]
    fn preset_set(&mut self, preset: u8) -> Result<(), Error>;

    /// Set current position as a preset.
    #[cfg(feature = "async")]
    async fn preset_set(&self, preset: u8) -> Result<(), Error>;
}

// Blanket implementation for cameras with preset support
#[cfg(not(feature = "async"))]
impl<P, T> PresetMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsPresets,
    T: crate::transport::blocking::BlockingTransport,
{
    fn preset_recall(&mut self, preset: u8) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        // Validate preset number
        if preset == 0 || preset > P::MAX_PRESETS {
            return Err(Error::ValidationError(
                crate::capabilities::ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!("Preset {} is invalid, must be 1-{}", preset, P::MAX_PRESETS),
                },
            ));
        }

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::PRESET_RECALL_PREFIX).push(preset);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }

    fn preset_set(&mut self, preset: u8) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        // Validate preset number
        if preset == 0 || preset > P::MAX_PRESETS {
            return Err(Error::ValidationError(
                crate::capabilities::ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!("Preset {} is invalid, must be 1-{}", preset, P::MAX_PRESETS),
                },
            ));
        }

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::PRESET_SET_PREFIX).push(preset);

        let response = self.transport.send_command(&cmd.build())?;
        response.into_result()
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P, T> PresetMethods for Camera<P, T>
where
    P: ProfileMetadata + SupportsPresets,
    T: crate::transport::AsyncTransport,
{
    async fn preset_recall(&self, preset: u8) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        // Validate preset number
        if preset == 0 || preset > P::MAX_PRESETS {
            return Err(Error::ValidationError(
                crate::capabilities::ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!("Preset {} is invalid, must be 1-{}", preset, P::MAX_PRESETS),
                },
            ));
        }

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::PRESET_RECALL_PREFIX).push(preset);

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }

    async fn preset_set(&self, preset: u8) -> Result<(), Error> {
        use crate::command::const_encoding::{commands, CommandBuilder};

        // Validate preset number
        if preset == 0 || preset > P::MAX_PRESETS {
            return Err(Error::ValidationError(
                crate::capabilities::ValidationError::InvalidValue {
                    parameter: "preset",
                    message: format!("Preset {} is invalid, must be 1-{}", preset, P::MAX_PRESETS),
                },
            ));
        }

        let mut cmd = CommandBuilder::<7>::new();
        cmd.append(&commands::PRESET_SET_PREFIX).push(preset);

        let response = self.transport.send_command(&cmd.build()).await?;
        response.into_result()
    }
}
