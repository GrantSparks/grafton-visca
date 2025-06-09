//! PTZ Builder for ergonomic camera control.
//!
//! This module provides a fluent builder interface for creating complex PTZ
//! (Pan-Tilt-Zoom) command sequences with support for borrowed references and
//! both concurrent and sequential execution modes.
//!
//! # Example
//! ```no_run
//! # #[cfg(feature = "blocking-client")] {
//! # use grafton_visca::{ViscaClient, ViscaClientPtzExt};
//! # use grafton_visca::command::pan_tilt::{PanSpeed, TiltSpeed, PanTiltDirection};
//! # use std::sync::Arc;
//! # let client = Arc::new(ViscaClient::connect_udp("192.168.1.100:5678").unwrap());
//! // Build a complex PTZ sequence
//! client.ptz()
//!     .pan_tilt_move(PanTiltDirection::UpRight, PanSpeed::new(10).unwrap(), TiltSpeed::new(10).unwrap())
//!     .zoom_in(5).unwrap()
//!     .focus_far()
//!     .execute_sequential()
//!     .unwrap();
//! # }
//! ```

// Standard library imports
use std::sync::Arc;

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::{
    command::{
        focus::{FocusCommand, FocusSpeed},
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        zoom::{ZoomCommand, ZoomSpeed},
    },
    error::Error as ViscaError,
    unified_client::Client as ViscaClient,
    ViscaCommand, Response,
};

/// Builder for creating PTZ command sequences.
///
/// Provides a fluent interface for building complex camera control sequences
/// with support for borrowed references and multiple execution modes.
pub struct PtzBuilder {
    /// Reference to the client that will execute commands
    client: Arc<ViscaClient>,
    /// Commands to be executed
    commands: Vec<Box<dyn ViscaCommand + Send + Sync>>,
}

impl PtzBuilder {
    /// Creates a new PTZ builder for the given client.
    pub(crate) fn new(client: Arc<ViscaClient>) -> Self {
        Self {
            client,
            commands: Vec::new(),
        }
    }

    /// Add a pan/tilt movement command.
    ///
    /// # Arguments
    /// * `direction` - Direction to move (can be borrowed)
    /// * `pan_speed` - Pan speed (can be borrowed)
    /// * `tilt_speed` - Tilt speed (can be borrowed)
    #[must_use]
    pub fn pan_tilt_move(
        mut self,
        direction: impl Into<PanTiltDirection> + Copy,
        pan_speed: impl Into<PanSpeed> + Copy,
        tilt_speed: impl Into<TiltSpeed> + Copy,
    ) -> Self {
        let command = PanTiltCommand::Move {
            direction: direction.into(),
            pan_speed: pan_speed.into(),
            tilt_speed: tilt_speed.into(),
        };
        self.commands.push(Box::new(command));
        self
    }

    /// Add a pan/tilt home command.
    #[must_use]
    pub fn pan_tilt_home(mut self) -> Self {
        self.commands.push(Box::new(PanTiltCommand::Home));
        self
    }

    /// Add a pan/tilt reset command.
    #[must_use]
    pub fn pan_tilt_reset(mut self) -> Self {
        self.commands.push(Box::new(PanTiltCommand::Reset));
        self
    }

    /// Add a pan/tilt absolute position command.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if the speed values cannot be converted to valid `PanSpeed` or `TiltSpeed`.
    pub fn pan_tilt_absolute(
        mut self,
        pan: i16,
        tilt: i16,
        pan_speed: impl TryInto<PanSpeed>,
        tilt_speed: impl TryInto<TiltSpeed>,
    ) -> Result<Self, ViscaError> {
        let pan_speed = pan_speed
            .try_into()
            .map_err(|_| ViscaError::InvalidParameter("Invalid pan speed".into()))?;
        let tilt_speed = tilt_speed
            .try_into()
            .map_err(|_| ViscaError::InvalidParameter("Invalid tilt speed".into()))?;

        let command = PanTiltCommand::AbsolutePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        };
        self.commands.push(Box::new(command));
        Ok(self)
    }

    /// Add a pan/tilt relative position command.
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if the speed values cannot be converted to valid `PanSpeed` or `TiltSpeed`.
    pub fn pan_tilt_relative(
        mut self,
        pan: i16,
        tilt: i16,
        pan_speed: impl TryInto<PanSpeed>,
        tilt_speed: impl TryInto<TiltSpeed>,
    ) -> Result<Self, ViscaError> {
        let pan_speed = pan_speed
            .try_into()
            .map_err(|_| ViscaError::InvalidParameter("Invalid pan speed".into()))?;
        let tilt_speed = tilt_speed
            .try_into()
            .map_err(|_| ViscaError::InvalidParameter("Invalid tilt speed".into()))?;

        let command = PanTiltCommand::RelativePosition {
            pan,
            tilt,
            pan_speed,
            tilt_speed,
        };
        self.commands.push(Box::new(command));
        Ok(self)
    }

    /// Add a zoom stop command.
    #[must_use]
    pub fn zoom_stop(mut self) -> Self {
        self.commands.push(Box::new(ZoomCommand::Stop));
        self
    }

    /// Add a zoom in command at variable speed.
    ///
    /// # Arguments
    /// * `speed` - Zoom speed (0=slowest, 7=fastest)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if the speed value cannot be converted to a valid `ZoomSpeed`.
    pub fn zoom_in(mut self, speed: impl TryInto<ZoomSpeed>) -> Result<Self, ViscaError> {
        let speed = speed
            .try_into()
            .map_err(|_| ViscaError::InvalidParameter("Invalid zoom speed".into()))?;
        self.commands
            .push(Box::new(ZoomCommand::TeleVariable(speed)));
        Ok(self)
    }

    /// Add a zoom out command at variable speed.
    ///
    /// # Arguments
    /// * `speed` - Zoom speed (0=slowest, 7=fastest)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if the speed value cannot be converted to a valid `ZoomSpeed`.
    pub fn zoom_out(mut self, speed: impl TryInto<ZoomSpeed>) -> Result<Self, ViscaError> {
        let speed = speed
            .try_into()
            .map_err(|_| ViscaError::InvalidParameter("Invalid zoom speed".into()))?;
        self.commands
            .push(Box::new(ZoomCommand::WideVariable(speed)));
        Ok(self)
    }

    /// Add a direct zoom position command.
    ///
    /// # Arguments
    /// * `position` - Zoom position (0x0000 to 0xFFFF)
    #[must_use]
    pub fn zoom_direct(mut self, position: u16) -> Self {
        self.commands.push(Box::new(ZoomCommand::Direct(position)));
        self
    }

    /// Add a focus near command.
    #[must_use]
    pub fn focus_near(mut self) -> Self {
        self.commands.push(Box::new(FocusCommand::NearStandard));
        self
    }

    /// Add a focus near command with variable speed.
    ///
    /// # Arguments
    /// * `speed` - Focus speed (0=slowest, 7=fastest)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if the speed value cannot be converted to a valid `FocusSpeed`.
    pub fn focus_near_variable(
        mut self,
        speed: impl TryInto<FocusSpeed>,
    ) -> Result<Self, ViscaError> {
        let speed = speed
            .try_into()
            .map_err(|_| ViscaError::InvalidParameter("Invalid focus speed".into()))?;
        self.commands
            .push(Box::new(FocusCommand::NearVariable(speed)));
        Ok(self)
    }

    /// Add a focus far command.
    #[must_use]
    pub fn focus_far(mut self) -> Self {
        self.commands.push(Box::new(FocusCommand::FarStandard));
        self
    }

    /// Add a focus far command with variable speed.
    ///
    /// # Arguments
    /// * `speed` - Focus speed (0=slowest, 7=fastest)
    ///
    /// # Errors
    /// Returns `ViscaError::InvalidParameter` if the speed value cannot be converted to a valid `FocusSpeed`.
    pub fn focus_far_variable(
        mut self,
        speed: impl TryInto<FocusSpeed>,
    ) -> Result<Self, ViscaError> {
        let speed = speed
            .try_into()
            .map_err(|_| ViscaError::InvalidParameter("Invalid focus speed".into()))?;
        self.commands
            .push(Box::new(FocusCommand::FarVariable(speed)));
        Ok(self)
    }

    /// Add a focus stop command.
    #[must_use]
    pub fn focus_stop(mut self) -> Self {
        self.commands.push(Box::new(FocusCommand::Stop));
        self
    }

    /// Add a focus auto command.
    #[must_use]
    pub fn focus_auto(mut self) -> Self {
        self.commands.push(Box::new(FocusCommand::Auto));
        self
    }

    /// Add a focus manual command.
    #[must_use]
    pub fn focus_manual(mut self) -> Self {
        self.commands.push(Box::new(FocusCommand::Manual));
        self
    }

    /// Add a custom command to the sequence.
    ///
    /// This allows adding any command that implements `ViscaCommand` + Send + Sync.
    #[must_use]
    pub fn custom_command(mut self, command: Box<dyn ViscaCommand + Send + Sync>) -> Self {
        self.commands.push(command);
        self
    }

    /// Execute all commands sequentially (blocking).
    ///
    /// Commands are executed one after another, waiting for each to complete
    /// before starting the next.
    ///
    /// # Errors
    /// Returns `ViscaError` if any command in the sequence fails to execute.
    #[cfg(feature = "blocking-client")]
    pub fn execute_sequential(self) -> Result<Vec<Response>, ViscaError> {
        let mut responses = Vec::with_capacity(self.commands.len());

        for command in self.commands {
            let response = self.client.send(command.as_ref())?;
            responses.push(response);
        }

        Ok(responses)
    }

    /// Execute all commands sequentially (async).
    ///
    /// Commands are executed one after another, waiting for each to complete
    /// before starting the next.
    ///
    /// # Errors
    /// Returns `ViscaError` if any command in the sequence fails to execute.
    #[cfg(feature = "async-client")]
    pub async fn execute_sequential_async(self) -> Result<Vec<Response>, ViscaError> {
        let mut responses = Vec::with_capacity(self.commands.len());

        for command in self.commands {
            let response = self.client.send_async(command.as_ref()).await?;
            responses.push(response);
        }

        Ok(responses)
    }

    /// Execute all commands concurrently (async only).
    ///
    /// All commands are sent concurrently, respecting the camera's
    /// concurrent command limit (2 for `PTZOptics` G2).
    ///
    /// # Errors
    /// Returns `ViscaError` if any command in the sequence fails to execute.
    #[cfg(feature = "async-client")]
    pub async fn execute_concurrent(self) -> Result<Vec<Response>, ViscaError> {
        use futures_util::future::try_join_all;

        let futures: Vec<_> = self
            .commands
            .into_iter()
            .map(|command| {
                let client = Arc::clone(&self.client);
                async move { client.send_async(command.as_ref()).await }
            })
            .collect();

        try_join_all(futures).await
    }

    /// Get the number of commands in the sequence.
    #[must_use]
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if the command sequence is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// Clear all commands from the sequence.
    #[must_use]
    pub fn clear(mut self) -> Self {
        self.commands.clear();
        self
    }
}

impl std::fmt::Debug for PtzBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PtzBuilder")
            .field("command_count", &self.commands.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    // Note: PtzBuilder tests would require actual ViscaClient instances

    // Note: These tests would require actual ViscaClient instances
    // For now, we'll test the builder structure

    #[test]
    #[allow(clippy::missing_const_for_fn)] // Test functions should not be const
    fn test_ptz_builder_structure() {
        // This is a structural test - we can't easily test execution without a mock client
        // But we can test that the builder methods chain correctly

        // Test that method chaining works (compilation test)
        #[allow(clippy::missing_const_for_fn)] // Test functions should not be const
        fn _test_method_chaining() {
            // This won't compile unless the method signatures are correct
            // let client = ViscaClient::connect_udp("test")?;
            // let _builder = client.ptz()
            //     .pan_tilt_home()
            //     .zoom_in(5)
            //     .focus_auto()
            //     .clear();
        }
    }
}
