//! Preset methods for cameras that support preset positions.

use crate::camera::Camera;
use crate::capabilities::{Presets, ProfileMetadata};
use crate::Error;
use grafton_visca_macros::dual_native_method;

/// Extension trait that adds preset methods to cameras.
#[allow(async_fn_in_trait)]
pub trait PresetMethodsExt {
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

// Blanket implementation for cameras with preset support - blocking
#[cfg(not(feature = "async"))]
impl<P, T> PresetMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + Presets,
    T: crate::transport::blocking::BlockingTransport,
{
    #[dual_native_method]
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
        cmd.append(commands::PRESET_RECALL_PREFIX).push(preset);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
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
        cmd.append(commands::PRESET_SET_PREFIX).push(preset);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

// Blanket implementation for cameras with preset support - async
#[cfg(feature = "async")]
impl<P, T> PresetMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + Presets,
    T: crate::transport::AsyncTransport,
{
    #[dual_native_method]
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
        cmd.append(commands::PRESET_RECALL_PREFIX).push(preset);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    #[dual_native_method]
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
        cmd.append(commands::PRESET_SET_PREFIX).push(preset);

        let response_bytes = self.send_raw(&cmd.build())?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}
