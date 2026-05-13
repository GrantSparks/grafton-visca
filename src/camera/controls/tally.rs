//! Tally light control implementation for PTZ cameras.
//!
//! This module provides tally light control functionality including:
//! - Red and green tally light control (on/off)
//! - Brightness adjustment (high/low settings)
//! - Flash functionality for attention-getting
//! - Combined tally operations (all lights)
//! - Status inquiry for current tally states
//! - Auto-adjust feature status checking
//!
//! Tally lights are indicator LEDs on cameras that show recording or live status.
//! Red typically indicates recording or live output, while green may indicate
//! preview or standby states. These are essential for studio and live production
//! environments where camera operators need visual feedback about camera status.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::{
        inquiry_structs::{TallyAutoAdjustInquiry, TallyGreenInquiry, TallyStatusInquiry},
        tally::{
            TallyBrightHi, TallyBrightLo, TallyFlash, TallyGreenOff, TallyGreenOn, TallyOff,
            TallyOn, TallyRedOff, TallyRedOn,
        },
    },
    mode::Mode,
    Error,
};

/// Tally light control operations for cameras.
///
/// This trait provides tally light control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Tally Light Standards
///
/// - **Red Light**: Typically indicates "on air" or recording status
/// - **Green Light**: Often used for preview, standby, or operator communication
/// - **Brightness**: Adjustable to suit different environments (studio vs outdoor)
/// - **Flash**: Attention-getting mode for operator alerts
///
/// # Multi-Color Support
///
/// Some cameras (like Sony FR7) support both red and green tally lights
/// simultaneously, allowing for more complex status indication schemes.
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.tally_red_on()?;  // Indicate recording
/// camera.tally_bright_hi()?;  // Increase brightness
/// camera.tally_flash()?;  // Flash for attention
/// camera.tally_off()?;  // Turn off all tally lights
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.tally_red_on().await?;  // Indicate recording
/// camera.tally_bright_hi().await?;  // Increase brightness
/// camera.tally_flash().await?;  // Flash for attention
/// camera.tally_off().await?;  // Turn off all tally lights
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait TallyControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Turn red tally light on.
    ///
    /// Activates the red tally light, typically used to indicate
    /// recording or "on air" status.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_red_on(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn red tally light off.
    ///
    /// Deactivates the red tally light.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_red_off(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set tally brightness to low.
    ///
    /// Reduces the brightness of tally lights for situations where
    /// bright lights might be distracting or inappropriate.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_bright_lo(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set tally brightness to high.
    ///
    /// Increases the brightness of tally lights for better visibility
    /// in bright environments or when greater visibility is needed.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_bright_hi(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn green tally light on.
    ///
    /// Activates the green tally light, often used for preview
    /// or standby status indication.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_green_on(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn green tally light off.
    ///
    /// Deactivates the green tally light.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_green_off(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Flash tally light.
    ///
    /// Causes the tally light to flash, typically used to get
    /// the operator's attention or indicate an alert condition.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_flash(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn tally light on.
    ///
    /// Activates the tally light system. This may affect all tally colors
    /// depending on the camera's implementation.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_on(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Turn tally light off.
    ///
    /// Deactivates the tally light system, turning off all tally indicators.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn tally_off(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Get tally light status (red and green states).
    ///
    /// Returns the current state of both red and green tally lights
    /// in a structured format.
    ///
    /// # Returns
    /// A `TallyStatusState` struct containing both red and green tally light states.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn tally_status(
        &self,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<crate::command::TallyStatusState, Error>>;

    /// Query green tally light state (FR7 specific).
    ///
    /// Returns the current state of the green tally light specifically.
    /// This uses a special extended inquiry format and is primarily
    /// supported on Sony FR7 cameras with dual tally lights.
    ///
    /// # Returns
    /// - `true` if the green tally light is on
    /// - `false` if the green tally light is off
    ///
    /// # Note
    /// This uses a special extended inquiry format (0x7E 0x04 0x1A 0x00) and may
    /// not be supported on all camera models.
    ///
    /// # Errors
    /// Returns an error if the inquiry fails, times out, or is not supported.
    fn green_tally_status(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Check if tally auto adjust is enabled.
    ///
    /// Returns whether automatic tally brightness adjustment is enabled.
    /// When enabled, the camera may automatically adjust tally brightness
    /// based on ambient conditions or other factors.
    ///
    /// # Returns
    /// - `true` if tally auto adjust is enabled
    /// - `false` if tally auto adjust is disabled
    ///
    /// # Errors
    /// Returns an error if the inquiry fails or times out.
    fn tally_auto_adjust_enabled(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> TallyControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn tally_red_on(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyRedOn::new())
    }

    fn tally_red_off(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyRedOff::new())
    }

    fn tally_bright_lo(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyBrightLo::new())
    }

    fn tally_bright_hi(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyBrightHi::new())
    }

    fn tally_green_on(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyGreenOn::new())
    }

    fn tally_green_off(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyGreenOff::new())
    }

    fn tally_flash(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyFlash::new())
    }

    fn tally_on(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyOn::new())
    }

    fn tally_off(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(TallyOff::new())
    }

    fn tally_status(&self) -> M::Fut<'_, Result<crate::command::TallyStatusState, Error>> {
        self.query(TallyStatusInquiry)
    }

    fn green_tally_status(&self) -> M::Fut<'_, Result<bool, Error>> {
        self.query(TallyGreenInquiry)
    }

    fn tally_auto_adjust_enabled(&self) -> M::Fut<'_, Result<bool, Error>> {
        self.query(TallyAutoAdjustInquiry)
    }
}
