//! High-level extension trait for focus control operations.

use crate::command::focus::FocusCommand;
use crate::error::ViscaError;
use crate::transport_ext::ViscaTransportExt;

/// Extension trait providing high-level focus control methods.
pub trait ViscaFocusExt: ViscaTransportExt {
    /// Enable or disable auto-focus mode.
    ///
    /// # Arguments
    /// * `enabled` - true to enable auto-focus, false for manual focus
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaFocusExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Enable auto-focus
    /// transport.set_auto_focus(true)?;
    ///
    /// // Switch to manual focus
    /// transport.set_auto_focus(false)?;
    /// # Ok(())
    /// # }
    /// ```
    fn set_auto_focus(&mut self, enabled: bool) -> Result<(), ViscaError> {
        let command = if enabled {
            FocusCommand::Auto
        } else {
            FocusCommand::Manual
        };
        self.send_command(&command)?;
        Ok(())
    }

    /// Move focus to an absolute position.
    ///
    /// # Arguments
    /// * `position` - Target focus position (0x1000 to 0xF000)
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaFocusExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Focus to near position
    /// transport.focus_to(0x1000)?;
    ///
    /// // Focus to far position
    /// transport.focus_to(0xF000)?;
    ///
    /// // Focus to mid-range
    /// transport.focus_to(0x8000)?;
    /// # Ok(())
    /// # }
    /// ```
    fn focus_to(&mut self, position: u16) -> Result<(), ViscaError> {
        let command = FocusCommand::Direct(position);
        self.send_command(&command)?;
        Ok(())
    }

    /// Start focusing near (closer to camera).
    ///
    /// # Arguments
    /// * `speed` - Optional focus speed (0-7). If None, uses standard speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaFocusExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Focus near at standard speed
    /// transport.focus_near(None)?;
    ///
    /// // Focus near at maximum speed
    /// transport.focus_near(Some(7))?;
    /// # Ok(())
    /// # }
    /// ```
    fn focus_near(&mut self, speed: Option<u8>) -> Result<(), ViscaError> {
        let command = if let Some(s) = speed {
            if s > 7 {
                return Err(ViscaError::InvalidParameter(
                    "Focus speed must be 0-7".into(),
                ));
            }
            FocusCommand::NearVariable(s)
        } else {
            FocusCommand::NearStandard
        };
        self.send_command(&command)?;
        Ok(())
    }

    /// Start focusing far (farther from camera).
    ///
    /// # Arguments
    /// * `speed` - Optional focus speed (0-7). If None, uses standard speed.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaFocusExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Focus far at standard speed
    /// transport.focus_far(None)?;
    ///
    /// // Focus far at slow speed
    /// transport.focus_far(Some(2))?;
    /// # Ok(())
    /// # }
    /// ```
    fn focus_far(&mut self, speed: Option<u8>) -> Result<(), ViscaError> {
        let command = if let Some(s) = speed {
            if s > 7 {
                return Err(ViscaError::InvalidParameter(
                    "Focus speed must be 0-7".into(),
                ));
            }
            FocusCommand::FarVariable(s)
        } else {
            FocusCommand::FarStandard
        };
        self.send_command(&command)?;
        Ok(())
    }

    /// Stop focus movement.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaFocusExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Start focusing near
    /// transport.focus_near(None)?;
    ///
    /// // ... wait some time ...
    ///
    /// // Stop focusing
    /// transport.stop_focus()?;
    /// # Ok(())
    /// # }
    /// ```
    fn stop_focus(&mut self) -> Result<(), ViscaError> {
        let command = FocusCommand::Stop;
        self.send_command(&command)?;
        Ok(())
    }

    /// Trigger one-push auto-focus.
    ///
    /// This performs a single auto-focus operation, even when in manual focus mode.
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaFocusExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Set manual focus mode
    /// transport.set_auto_focus(false)?;
    ///
    /// // Trigger one-time auto-focus
    /// transport.trigger_one_push_focus()?;
    /// # Ok(())
    /// # }
    /// ```
    fn trigger_one_push_focus(&mut self) -> Result<(), ViscaError> {
        let command = FocusCommand::OnePushTrigger;
        self.send_command(&command)?;
        Ok(())
    }
}

/// Implement the trait for all types that implement ViscaTransportExt
impl<T: ViscaTransportExt> ViscaFocusExt for T {}
