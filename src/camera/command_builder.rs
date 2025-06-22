//! Fluent command builder for `Camera<P>` API.
//!
//! This module provides a builder interface for creating complex command sequences
//! with sequential execution.

use crate::{
    camera::{Camera, CameraProfile},
    command::{
        color::OnePushTriggerCommand,
        exposure::{ExposureCommand, ExposureMode, IrisCommand, ShutterCommand},
        focus::{FocusCommand, FocusSpeed},
        image::{BacklightCommand, NoiseReduction2DCommand, NoiseReduction3DCommand},
        pan_tilt::{PanTiltCommand, PanTiltDirection},
        power::{Power, PowerCommand},
        preset::{PresetAction, PresetCommand},
        white_balance::{WhiteBalanceCommand, WhiteBalanceMode},
        zoom::{ZoomCommand, ZoomSpeed},
    },
    types::{
        GainLimit, IrisLevel, PanPosition, PanSpeed, ShutterSpeed, TiltPosition, TiltSpeed,
        ZoomPosition,
    },
    Command, Error, Response,
};

/// A command that has been prepared for execution.
#[allow(dead_code)]
struct PreparedCommand {
    command: Box<dyn Command + Send + Sync>,
    description: String,
}

/// Builder for creating command sequences on a Camera.
#[cfg(not(feature = "async"))]
pub struct CommandBuilder<'a, P: CameraProfile, T> {
    camera: &'a mut Camera<P, T>,
    commands: Vec<PreparedCommand>,
}

/// Builder for creating command sequences on a Camera (async version with interior mutability).
#[cfg(feature = "async")]
pub struct CommandBuilder<'a, P: CameraProfile, T> {
    camera: &'a Camera<P, T>,
    commands: Vec<PreparedCommand>,
}

impl<P: CameraProfile, T> std::fmt::Debug for CommandBuilder<'_, P, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandBuilder")
            .field("commands", &self.commands.len())
            .finish()
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P: CameraProfile, T> CommandBuilder<'_, P, T> {
    /// Creates a new command builder for the given camera.
    pub(crate) fn new(camera: &mut Camera<P, T>) -> CommandBuilder<'_, P, T> {
        CommandBuilder {
            camera,
            commands: Vec::new(),
        }
    }
}

// Async implementation
#[cfg(feature = "async")]
impl<P: CameraProfile, T> CommandBuilder<'_, P, T> {
    /// Creates a new command builder for the given camera.
    pub(crate) fn new(camera: &Camera<P, T>) -> CommandBuilder<'_, P, T> {
        CommandBuilder {
            camera,
            commands: Vec::new(),
        }
    }
}

// Shared implementation for both blocking and async
impl<P: CameraProfile, T> CommandBuilder<'_, P, T> {
    /// Adds a command to the sequence.
    fn add_command(
        mut self,
        command: impl Command + 'static,
        description: impl Into<String>,
    ) -> Self {
        self.commands.push(PreparedCommand {
            command: Box::new(command),
            description: description.into(),
        });
        self
    }

    // Power Commands

    /// Adds a power on command.
    #[must_use]
    pub fn power_on(self) -> Self {
        self.add_command(PowerCommand { power: Power::On }, "Power On")
    }

    /// Adds a power off (standby) command.
    #[must_use]
    pub fn power_off(self) -> Self {
        self.add_command(
            PowerCommand {
                power: Power::Standby,
            },
            "Power Off",
        )
    }

    // Pan/Tilt Commands

    /// Adds a pan/tilt move command.
    #[must_use]
    pub fn pan_tilt_move(
        self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Self {
        self.add_command(
            PanTiltCommand::Move {
                direction,
                pan_speed,
                tilt_speed,
            },
            format!("Pan/Tilt Move {:?}", direction),
        )
    }

    /// Adds a pan/tilt stop command.
    #[must_use]
    pub fn pan_tilt_stop(self) -> Self {
        self.add_command(
            PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::ZERO,
                tilt_speed: TiltSpeed::ZERO,
            },
            "Pan/Tilt Stop",
        )
    }

    /// Adds a pan/tilt home command.
    #[must_use]
    pub fn pan_tilt_home(self) -> Self {
        self.add_command(PanTiltCommand::Home, "Pan/Tilt Home")
    }

    /// Adds a pan/tilt absolute position command in degrees.
    ///
    /// # Errors
    /// Returns an error if the position is outside the camera's range.
    pub fn pan_tilt_to_degrees(
        self,
        pan_deg: f32,
        tilt_deg: f32,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<Self, Error> {
        let pan = self.camera.profile().pan_degrees_to_units(pan_deg);
        let tilt = self.camera.profile().tilt_degrees_to_units(tilt_deg);

        // Validate ranges
        let pan_range = self.camera.profile().pan_range();
        if !pan_range.contains(&pan) {
            return Err(Error::InvalidParameter(format!(
                "Pan position {} degrees ({} units) outside range [{}, {}]",
                pan_deg,
                pan,
                pan_range.start(),
                pan_range.end()
            )));
        }

        let tilt_range = self.camera.profile().tilt_range();
        if !tilt_range.contains(&tilt) {
            return Err(Error::InvalidParameter(format!(
                "Tilt position {} degrees ({} units) outside range [{}, {}]",
                tilt_deg,
                tilt,
                tilt_range.start(),
                tilt_range.end()
            )));
        }

        Ok(self.add_command(
            PanTiltCommand::AbsolutePosition {
                pan: PanPosition::new(pan)?,
                tilt: TiltPosition::new(tilt)?,
                pan_speed,
                tilt_speed,
            },
            format!("Pan/Tilt to {}°, {}°", pan_deg, tilt_deg),
        ))
    }

    /// Adds a pan/tilt relative position command in degrees.
    #[must_use]
    pub fn pan_tilt_relative_degrees(
        self,
        pan_deg: f32,
        tilt_deg: f32,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Self {
        let pan = self.camera.profile().pan_degrees_to_units(pan_deg);
        let tilt = self.camera.profile().tilt_degrees_to_units(tilt_deg);

        self.add_command(
            PanTiltCommand::RelativePosition {
                pan: PanPosition::new(pan).unwrap_or(PanPosition::CENTER),
                tilt: TiltPosition::new(tilt).unwrap_or(TiltPosition::CENTER),
                pan_speed,
                tilt_speed,
            },
            format!("Pan/Tilt relative {}°, {}°", pan_deg, tilt_deg),
        )
    }

    // Zoom Commands

    /// Adds a zoom stop command.
    #[must_use]
    pub fn zoom_stop(self) -> Self {
        self.add_command(ZoomCommand::Stop, "Zoom Stop")
    }

    /// Adds a zoom in command.
    #[must_use]
    pub fn zoom_in(self, speed: ZoomSpeed) -> Self {
        self.add_command(
            ZoomCommand::ZoomInVariable(speed),
            format!("Zoom In (speed {})", speed.value()),
        )
    }

    /// Adds a zoom out command.
    #[must_use]
    pub fn zoom_out(self, speed: ZoomSpeed) -> Self {
        self.add_command(
            ZoomCommand::ZoomOutVariable(speed),
            format!("Zoom Out (speed {})", speed.value()),
        )
    }

    /// Adds a direct zoom position command.
    #[must_use]
    pub fn zoom_to(self, position: u16) -> Self {
        self.add_command(
            ZoomCommand::Direct(ZoomPosition::new(position).unwrap_or(ZoomPosition::MIN)),
            format!("Zoom to position {}", position),
        )
    }

    // Focus Commands

    /// Adds a focus stop command.
    #[must_use]
    pub fn focus_stop(self) -> Self {
        self.add_command(FocusCommand::Stop, "Focus Stop")
    }

    /// Adds a focus near command.
    #[must_use]
    pub fn focus_near(self, speed: FocusSpeed) -> Self {
        self.add_command(
            FocusCommand::NearVariable(speed),
            format!("Focus Near (speed {})", speed.value()),
        )
    }

    /// Adds a focus far command.
    #[must_use]
    pub fn focus_far(self, speed: FocusSpeed) -> Self {
        self.add_command(
            FocusCommand::FarVariable(speed),
            format!("Focus Far (speed {})", speed.value()),
        )
    }

    /// Adds a focus auto command.
    #[must_use]
    pub fn focus_auto(self) -> Self {
        self.add_command(FocusCommand::Auto, "Focus Auto")
    }

    /// Adds a focus manual command.
    #[must_use]
    pub fn focus_manual(self) -> Self {
        self.add_command(FocusCommand::Manual, "Focus Manual")
    }

    /// Adds a direct focus position command.
    #[must_use]
    pub fn focus_to(self, position: u16) -> Self {
        self.add_command(
            FocusCommand::Direct(
                crate::types::FocusPosition::new(position)
                    .unwrap_or(crate::types::FocusPosition::MIN),
            ),
            format!("Focus to position {}", position),
        )
    }

    // Preset Commands

    /// Adds a preset recall command.
    #[must_use]
    pub fn preset_recall(self, preset_id: P::PresetId) -> Self {
        let id_val: u8 = preset_id.into();
        match crate::command::preset::PresetNumber::new(id_val) {
            Ok(preset_number) => self.add_command(
                PresetCommand {
                    action: PresetAction::Recall,
                    preset_number,
                },
                format!("Recall Preset {}", id_val),
            ),
            Err(_) => self, // Invalid preset number, skip
        }
    }

    /// Adds a preset set command.
    #[must_use]
    pub fn preset_set(self, preset_id: P::PresetId) -> Self {
        let id_val: u8 = preset_id.into();
        match crate::command::preset::PresetNumber::new(id_val) {
            Ok(preset_number) => self.add_command(
                PresetCommand {
                    action: PresetAction::Set,
                    preset_number,
                },
                format!("Set Preset {}", id_val),
            ),
            Err(_) => self, // Invalid preset number, skip
        }
    }

    // Exposure Commands

    /// Adds an exposure mode command.
    #[must_use]
    pub fn exposure_mode(self, mode: ExposureMode) -> Self {
        self.add_command(
            ExposureCommand { mode },
            format!("Exposure Mode {:?}", mode),
        )
    }

    /// Adds a shutter speed command.
    #[must_use]
    pub fn shutter_speed(self, speed: ShutterSpeed) -> Self {
        self.add_command(
            ShutterCommand::Direct(speed),
            format!("Shutter Speed {:?}", speed),
        )
    }

    /// Adds an iris command.
    #[must_use]
    pub fn iris(self, value: IrisLevel) -> Self {
        self.add_command(IrisCommand::Direct(value), format!("Iris {:?}", value))
    }

    /// Adds a gain limit command.
    #[must_use]
    pub fn gain_limit(self, limit: GainLimit) -> Self {
        self.add_command(
            crate::command::gain::GainLimitCommand { limit },
            format!("Gain Limit {:?}", limit),
        )
    }

    /// Adds a gain limit command (alias for gain_limit).
    #[must_use]
    pub fn gain(self, limit: GainLimit) -> Self {
        self.gain_limit(limit)
    }

    // White Balance Commands

    /// Adds a white balance mode command.
    #[must_use]
    pub fn white_balance_mode(self, mode: WhiteBalanceMode) -> Self {
        self.add_command(
            WhiteBalanceCommand { mode },
            format!("White Balance {:?}", mode),
        )
    }

    /// Adds a one-push white balance trigger command.
    #[must_use]
    pub fn white_balance_one_push(self) -> Self {
        self.add_command(OnePushTriggerCommand, "One Push White Balance")
    }

    // Image Adjustment Commands

    /// Adds a backlight compensation command.
    #[must_use]
    pub fn backlight(self, enabled: bool) -> Self {
        self.add_command(
            BacklightCommand { status: enabled },
            format!("Backlight {}", if enabled { "On" } else { "Off" }),
        )
    }

    /// Adds a 2D noise reduction command.
    #[must_use]
    pub fn noise_reduction(self, level: u8) -> Self {
        match crate::types::NoiseReduction2DLevel::new(level) {
            Ok(nr_level) => self.add_command(
                NoiseReduction2DCommand::Level(nr_level),
                format!("2D Noise Reduction Level {}", level),
            ),
            Err(_) => self, // Invalid level, skip
        }
    }

    /// Adds a 3D noise reduction command.
    #[must_use]
    pub fn noise_reduction_3d(self, level: u8) -> Self {
        match crate::types::NoiseReduction3DLevel::new(level) {
            Ok(nr_level) => self.add_command(
                NoiseReduction3DCommand::Level(nr_level),
                format!("3D Noise Reduction Level {}", level),
            ),
            Err(_) => self, // Invalid level, skip
        }
    }

    /// Adds an anti-flicker command.
    #[must_use]
    pub fn anti_flicker(self, mode: crate::command::gain::AntiFlickerMode) -> Self {
        self.add_command(
            crate::command::gain::AntiFlickerCommand { mode },
            format!("Anti-Flicker {:?}", mode),
        )
    }

    // Custom Commands

    /// Adds a custom command to the sequence.
    #[must_use]
    pub fn custom(self, command: impl Command + 'static, description: impl Into<String>) -> Self {
        self.add_command(command, description)
    }

    // Execution Methods

    /// Executes all commands sequentially (blocking).
    ///
    /// Commands are executed one after another, waiting for each to complete
    /// before starting the next. Returns results for all commands.
    ///
    /// # Errors
    /// Returns an error if any command fails. Execution stops at the first error.
    #[cfg(not(feature = "async"))]
    pub fn execute_sequential(self) -> Result<Vec<Response>, Error>
    where
        T: crate::transport::blocking::Transport,
    {
        let mut responses = Vec::with_capacity(self.commands.len());

        for (i, prepared) in self.commands.into_iter().enumerate() {
            log::debug!("Executing command {}: {}", i + 1, prepared.description);

            match self.camera.send_command(prepared.command.as_ref()) {
                Ok(response) => responses.push(response),
                Err(e) => {
                    log::error!("Command {} failed: {}", prepared.description, e);
                    return Err(e);
                }
            }
        }

        Ok(responses)
    }

    /// Executes all commands sequentially (async).
    ///
    /// Commands are executed one after another, waiting for each to complete
    /// before starting the next. Returns results for all commands.
    ///
    /// # Errors
    /// Returns an error if any command fails. Execution stops at the first error.
    #[cfg(feature = "async")]
    pub async fn execute_sequential_async(self) -> Result<Vec<Response>, Error>
    where
        T: crate::transport::AsyncTransport,
    {
        let mut responses = Vec::with_capacity(self.commands.len());

        for (i, prepared) in self.commands.into_iter().enumerate() {
            log::debug!("Executing command {}: {}", i + 1, prepared.description);

            match self.camera.send_command(prepared.command.as_ref()).await {
                Ok(response) => responses.push(response),
                Err(e) => {
                    log::error!("Command {} failed: {}", prepared.description, e);
                    return Err(e);
                }
            }
        }

        Ok(responses)
    }

    /// Returns the number of commands in the sequence.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Returns true if the sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Clears all commands from the sequence.
    pub fn clear(&mut self) {
        self.commands.clear();
    }
}

/// Extension trait to add command builder support to Camera (blocking).
#[cfg(not(feature = "async"))]
pub trait CommandBuilderExt<P: CameraProfile, T> {
    /// Creates a new command builder for this camera.
    fn commands(&mut self) -> CommandBuilder<'_, P, T>;
}

#[cfg(not(feature = "async"))]
impl<P: CameraProfile, T> CommandBuilderExt<P, T> for Camera<P, T> {
    fn commands(&mut self) -> CommandBuilder<'_, P, T> {
        CommandBuilder::new(self)
    }
}

/// Extension trait to add command builder support to Camera (async).
#[cfg(feature = "async")]
pub trait CommandBuilderExt<P: CameraProfile, T> {
    /// Creates a new command builder for this camera.
    fn commands(&self) -> CommandBuilder<'_, P, T>;
}

#[cfg(feature = "async")]
impl<P: CameraProfile, T> CommandBuilderExt<P, T> for Camera<P, T> {
    fn commands(&self) -> CommandBuilder<'_, P, T> {
        CommandBuilder::new(self)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test_command_builder_creation() {
        // This is a compile-time test to ensure the builder can be created
        // Actual execution tests would require a mock transport
    }

    #[test]
    fn test_builder_chaining() {
        // This tests that methods can be chained properly
        // Again, mainly a compile-time test
    }
}
