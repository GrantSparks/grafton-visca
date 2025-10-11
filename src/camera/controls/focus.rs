//! Focus control implementation for PTZ cameras.
//!
//! This module provides comprehensive focus control functionality including:
//! - Automatic and manual focus modes
//! - Variable speed focus adjustment (near/far)
//! - Absolute position control and infinity focus
//! - Focus lock to prevent unwanted changes
//! - One-push auto focus for quick adjustment
//! - Push AF for temporary auto focus
//! - Focus zone configuration for area-specific focusing
//! - Auto focus sensitivity adjustment
//! - Near limit setting to prevent close-object focus
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{
    camera::ViscaClient,
    command::focus::{
        AutoFocusSensitivity, AutoFocusSensitivityCommand, Focus, FocusLock, FocusNearLimitCommand,
        FocusSpeed, FocusZone, FocusZoneCommand, PushAF,
    },
    mode::Mode,
    types::{FocusPosition, SpeedLevel},
    Error,
};

/// Focus operations for PTZ cameras.
///
/// This trait provides comprehensive focus control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Focus Modes
///
/// - **Auto Focus**: Camera automatically adjusts focus based on scene content
/// - **Manual Focus**: User has direct control over focus position
/// - **One-Push AF**: Single auto focus operation then returns to manual
/// - **Push AF**: Temporary auto focus while button is held
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.focus_auto()?;  // Enable auto focus
/// camera.focus_one_push()?;  // Quick auto focus adjustment
/// camera.focus_manual()?;  // Switch to manual mode
/// camera.focus_far(SpeedLevel::Medium)?;  // Adjust focus manually
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.focus_auto().await?;  // Enable auto focus
/// camera.focus_one_push().await?;  // Quick auto focus adjustment
/// camera.focus_manual().await?;  // Switch to manual mode
/// camera.focus_far(SpeedLevel::Medium).await?;  // Adjust focus manually
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait FocusControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set auto focus mode.
    ///
    /// In auto focus mode, the camera automatically adjusts focus based on the scene content
    /// within the configured focus zone.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_auto(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set manual focus mode.
    ///
    /// In manual focus mode, focus must be adjusted manually using focus_near() or focus_far().
    /// Auto focus operations will not function in this mode.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_manual(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Focus near at specified speed.
    ///
    /// Moves focus toward closer objects. The focus will continue moving until
    /// focus_stop() is called or the near limit is reached.
    ///
    /// # Parameters
    /// - `speed`: Movement speed from slow to fast
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_near(
        &self,
        speed: SpeedLevel,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Focus far at specified speed.
    ///
    /// Moves focus toward distant objects. The focus will continue moving until
    /// focus_stop() is called or infinity is reached.
    ///
    /// # Parameters
    /// - `speed`: Movement speed from slow to fast
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_far(
        &self,
        speed: SpeedLevel,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Stop focus movement.
    ///
    /// Immediately stops any ongoing focus adjustment operation.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_stop(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Trigger one-push auto focus.
    ///
    /// Performs a single auto focus operation to quickly achieve sharp focus,
    /// then returns to the previous focus mode (typically manual).
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_one_push(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set focus to a specific position.
    ///
    /// Moves focus directly to the specified absolute position.
    /// The camera must be in manual focus mode for this to work.
    ///
    /// # Parameters
    /// - `position`: Target focus position
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_focus(
        &self,
        position: FocusPosition,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set focus to infinity.
    ///
    /// Sets focus to the maximum distance position for capturing distant objects.
    /// This is equivalent to the far limit of the focus range.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_infinity(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable focus lock.
    ///
    /// Locks the current focus position to prevent any changes from auto focus
    /// or manual adjustments. Useful for maintaining consistent focus during recording.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_focus_lock(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable focus lock.
    ///
    /// Allows focus to be adjusted again after being locked.
    /// Returns focus control to the previously selected mode.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_focus_lock(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Press Push AF button.
    ///
    /// Temporarily activates auto focus while the button is pressed.
    /// This allows quick focus adjustment without changing the focus mode.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn push_af_press(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Release Push AF button.
    ///
    /// Returns to the previous focus mode after temporary auto focus.
    /// Must be called after push_af_press() to end the temporary auto focus.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn push_af_release(
        &self,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set the focus zone.
    ///
    /// Determines which area of the image the camera uses for auto focus detection.
    /// Different zones allow focusing on different parts of the scene.
    ///
    /// # Parameters
    /// - `zone`: The focus detection zone to use
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_focus_zone(
        &self,
        zone: FocusZone,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto focus sensitivity.
    ///
    /// Controls how responsive the auto focus system is to changes in the scene.
    /// Higher sensitivity means faster response to scene changes but may cause hunting.
    ///
    /// # Parameters
    /// - `sensitivity`: Auto focus sensitivity level
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set the focus near limit.
    ///
    /// Sets the minimum focus distance to prevent the camera from focusing on objects
    /// too close to the lens. This is useful to avoid focusing on dust or scratches
    /// on the lens surface.
    ///
    /// # Parameters
    /// - `position`: The near limit focus position
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn set_focus_near_limit(
        &self,
        position: FocusPosition,
        opts: crate::CommandOptions<'_>,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> FocusControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn focus_auto(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(Focus::Auto, opts)
    }

    fn focus_manual(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(Focus::Manual, opts)
    }

    fn focus_near(
        &self,
        speed: SpeedLevel,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Near
        } else {
            match FocusSpeed::new(focus_speed_val.min(7)) {
                Ok(focus_speed) => Focus::NearWithSpeed(focus_speed),
                Err(e) => return self.error(e),
            }
        };
        self.execute_with_opts(cmd, opts)
    }

    fn focus_far(
        &self,
        speed: SpeedLevel,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Far
        } else {
            match FocusSpeed::new(focus_speed_val.min(7)) {
                Ok(focus_speed) => Focus::FarWithSpeed(focus_speed),
                Err(e) => return self.error(e),
            }
        };
        self.execute_with_opts(cmd, opts)
    }

    fn focus_stop(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(Focus::Stop, opts)
    }

    fn focus_one_push(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(Focus::OnePushTrigger, opts)
    }

    fn set_focus(
        &self,
        position: FocusPosition,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(Focus::Position(position), opts)
    }

    fn focus_infinity(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(Focus::Infinity, opts)
    }

    fn enable_focus_lock(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(FocusLock::On, opts)
    }

    fn disable_focus_lock(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(FocusLock::Off, opts)
    }

    fn push_af_press(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(PushAF::Press, opts)
    }

    fn push_af_release(&self, opts: crate::CommandOptions<'_>) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(PushAF::Release, opts)
    }

    fn set_focus_zone(
        &self,
        zone: FocusZone,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(FocusZoneCommand { zone }, opts)
    }

    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(AutoFocusSensitivityCommand { sensitivity }, opts)
    }

    fn set_focus_near_limit(
        &self,
        position: FocusPosition,
        opts: crate::CommandOptions<'_>,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute_with_opts(FocusNearLimitCommand { position }, opts)
    }
}
