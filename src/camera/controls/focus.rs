//! Focus control implementation for PTZ cameras.
//!
//! This module provides comprehensive focus control functionality including:
//! - Automatic and manual focus modes
//! - Variable speed focus adjustment (near/far)
//! - Absolute position control and infinity focus
//! - Focus lock to prevent unwanted changes (vendor-specific: PtzOptics)
//! - One-push auto focus for quick adjustment
//! - Push AF for temporary auto focus (vendor-specific: Sony FR7)
//! - Focus zone configuration for area-specific focusing
//! - Auto focus sensitivity adjustment
//! - Near limit setting to prevent close-object focus
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.
//!
//! # Capability-Gated Traits
//!
//! Vendor-specific features are gated by marker traits to ensure compile-time safety:
//! - `FocusLockControl` requires `HasFocusLock` (PtzOptics cameras)
//! - `PushAFControl` requires `HasPushAutoFocus` (Sony FR7)

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
    fn focus_auto(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set manual focus mode.
    ///
    /// In manual focus mode, focus must be adjusted manually using focus_near() or focus_far().
    /// Auto focus operations will not function in this mode.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_manual(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

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
    fn focus_near(&self, speed: SpeedLevel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

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
    fn focus_far(&self, speed: SpeedLevel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Stop focus movement.
    ///
    /// Immediately stops any ongoing focus adjustment operation.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_stop(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Trigger one-push auto focus.
    ///
    /// Performs a single auto focus operation to quickly achieve sharp focus,
    /// then returns to the previous focus mode (typically manual).
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_one_push(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set focus to a specific position.
    ///
    /// Moves focus directly to the specified absolute position.
    /// The camera must be in manual focus mode for this to work.
    ///
    /// This method accepts any type that can be converted to `FocusPosition`, providing
    /// a flexible API for setting focus using different units:
    ///
    /// - `Percentage(50.0)` - Set focus to 50% of range
    /// - `Normalized(0.5)` - Set focus to 0.5 (equivalent to 50%)
    /// - `Raw(0x8000)` - Set focus to raw VISCA value
    /// - `FocusPosition` - Set focus to specific position directly
    ///
    /// # Arguments
    /// * `position` - Target focus position (accepts multiple types via `TryInto<FocusPosition>`)
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Percentage, Normalized, Raw};
    ///
    /// // Using percentage
    /// camera.set_focus(Percentage(50.0))?;
    ///
    /// // Using normalized value
    /// camera.set_focus(Normalized(0.5))?;
    ///
    /// // Using raw value
    /// camera.set_focus(Raw(0x8000_u16))?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - The conversion to `FocusPosition` fails (e.g., value out of range)
    /// - The command fails to send or receive a response
    fn set_focus<T>(&self, position: T) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>
    where
        T: TryInto<FocusPosition>,
        T::Error: Into<Error>;

    /// Set focus to infinity.
    ///
    /// Sets focus to the maximum distance position for capturing distant objects.
    /// This is equivalent to the far limit of the focus range.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_infinity(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

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
    fn set_focus_zone(&self, zone: FocusZone) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

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
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set the focus near limit.
    ///
    /// Sets the minimum focus distance to prevent the camera from focusing on objects
    /// too close to the lens. This is useful to avoid focusing on dust or scratches
    /// on the lens surface.
    ///
    /// This method accepts any type that can be converted to `FocusPosition`.
    ///
    /// # Parameters
    /// - `position`: The near limit focus position (accepts multiple types via `TryInto<FocusPosition>`)
    ///
    /// # Errors
    /// Returns an error if:
    /// - The conversion to `FocusPosition` fails (e.g., value out of range)
    /// - The command fails to send or receive a response
    fn set_focus_near_limit<T>(
        &self,
        position: T,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>
    where
        T: TryInto<FocusPosition>,
        T::Error: Into<Error>;

    /// Toggle between auto and manual focus modes.
    ///
    /// Switches the focus mode from auto to manual or vice versa.
    ///
    /// **Vendor-Specific**: PTZOptics cameras only.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_toggle(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Trigger snap focus (one-push AF in manual mode).
    ///
    /// Performs a single autofocus operation then returns to manual focus mode.
    /// This is similar to `focus_one_push` but uses the PTZOptics-specific
    /// snap focus implementation.
    ///
    /// **Vendor-Specific**: PTZOptics cameras only.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn focus_snap(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
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

    fn focus_auto(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::Auto)
    }

    fn focus_manual(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::Manual)
    }

    fn focus_near(&self, speed: SpeedLevel) -> M::Fut<'_, Result<(), Error>> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Near
        } else {
            match FocusSpeed::new(focus_speed_val.min(7)) {
                Ok(focus_speed) => Focus::NearWithSpeed(focus_speed),
                Err(e) => return self.error(e),
            }
        };
        self.execute(cmd)
    }

    fn focus_far(&self, speed: SpeedLevel) -> M::Fut<'_, Result<(), Error>> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Far
        } else {
            match FocusSpeed::new(focus_speed_val.min(7)) {
                Ok(focus_speed) => Focus::FarWithSpeed(focus_speed),
                Err(e) => return self.error(e),
            }
        };
        self.execute(cmd)
    }

    fn focus_stop(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::Stop)
    }

    fn focus_one_push(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::OnePushTrigger)
    }

    fn set_focus<T>(&self, position: T) -> M::Fut<'_, Result<(), Error>>
    where
        T: TryInto<FocusPosition>,
        T::Error: Into<Error>,
    {
        match position.try_into() {
            Ok(focus_pos) => self.execute(Focus::Position(focus_pos)),
            Err(e) => self.error(e.into()),
        }
    }

    fn focus_infinity(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::Infinity)
    }

    fn set_focus_zone(&self, zone: FocusZone) -> M::Fut<'_, Result<(), Error>> {
        self.execute(FocusZoneCommand { zone })
    }

    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> M::Fut<'_, Result<(), Error>> {
        self.execute(AutoFocusSensitivityCommand { sensitivity })
    }

    fn set_focus_near_limit<T>(&self, position: T) -> M::Fut<'_, Result<(), Error>>
    where
        T: TryInto<FocusPosition>,
        T::Error: Into<Error>,
    {
        match position.try_into() {
            Ok(focus_pos) => self.execute(FocusNearLimitCommand {
                position: focus_pos,
            }),
            Err(e) => self.error(e.into()),
        }
    }

    fn focus_toggle(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::Toggle)
    }

    fn focus_snap(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::Snap)
    }
}

// Separate implementation for async-mode _op methods on async Camera
#[cfg(feature = "mode-async")]
impl<P, Tr, Exec> crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    /// Set focus to a specific position and return an operation handle.
    ///
    /// This method accepts any type that can be converted to `FocusPosition`, providing
    /// a flexible API for setting focus using different units. Returns an `InFlight`
    /// handle for fine-grained control over timeouts and cancellation.
    ///
    /// # Arguments
    /// * `position` - Target focus position (accepts multiple types via `TryInto<FocusPosition>`)
    ///
    /// # Examples
    /// ```ignore
    /// use grafton_visca::units::{Percentage, Normalized};
    /// use std::time::Duration;
    ///
    /// // Using percentage
    /// let handle = camera.set_focus_op(Percentage(50.0)).await?;
    /// handle.await_completion(Duration::from_secs(5)).await?;
    ///
    /// // Using normalized value
    /// let handle = camera.set_focus_op(Normalized(0.5)).await?;
    /// handle.await_completion(Duration::from_secs(5)).await?;
    /// ```
    ///
    /// # Errors
    /// Returns an error if:
    /// - The conversion to `FocusPosition` fails (e.g., value out of range)
    /// - The command fails to send
    pub async fn set_focus_op<T>(
        &self,
        position: T,
    ) -> Result<crate::camera::inflight::InFlight<'_, crate::camera::inflight::Focus, Self>, Error>
    where
        T: TryInto<FocusPosition>,
        T::Error: Into<Error>,
    {
        use crate::command::focus::Focus;

        let focus_pos = position.try_into().map_err(Into::into)?;
        let cmd = Focus::Position(focus_pos);

        // Use start_command_with_id to get the response future without awaiting it
        let (id, response_future) = self.start_command_with_id(&cmd).await?;

        // Return InFlight handle with the response future
        Ok(crate::camera::inflight::InFlight::new(
            id,
            self.camera_id(),
            self,
            response_future,
        ))
    }
}

/// Focus lock control for cameras that support focus locking.
///
/// This trait provides focus lock functionality which prevents any changes to
/// focus position while enabled. This is a vendor-specific feature primarily
/// supported by PtzOptics cameras.
///
/// # Capability Gating
///
/// This trait is only available for camera profiles that implement the
/// `HasFocusLock` marker trait. Attempting to use these methods on unsupported
/// cameras will result in a compile-time error.
///
/// # Examples
///
/// ```ignore
/// // Only works with PtzOptics cameras that implement HasFocusLock
/// camera.enable_focus_lock()?;
/// // Focus is now locked at current position
/// // ... later ...
/// camera.disable_focus_lock()?;
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait FocusLockControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable focus lock.
    ///
    /// Locks the current focus position to prevent any changes from auto focus
    /// or manual adjustments. Useful for maintaining consistent focus during recording.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn enable_focus_lock(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable focus lock.
    ///
    /// Allows focus to be adjusted again after being locked.
    /// Returns focus control to the previously selected mode.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn disable_focus_lock(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Implementation of FocusLockControl for cameras with HasFocusLock capability
impl<M, P, Tr, Exec> FocusLockControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + crate::capabilities::HasFocusLock + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_focus_lock(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(FocusLock::On)
    }

    fn disable_focus_lock(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(FocusLock::Off)
    }
}

/// Push AF control for cameras that support temporary auto focus.
///
/// Push AF temporarily activates auto focus while the button is pressed,
/// then returns to the previous focus mode. This is a vendor-specific feature
/// primarily supported by Sony FR7 cameras.
///
/// # Capability Gating
///
/// This trait is only available for camera profiles that implement the
/// `HasPushAutoFocus` marker trait. Attempting to use these methods on unsupported
/// cameras will result in a compile-time error.
///
/// # Examples
///
/// ```ignore
/// // Only works with Sony FR7 cameras that implement HasPushAutoFocus
/// camera.push_af_press()?;  // Start temporary auto focus
/// // ... focus is adjusting ...
/// camera.push_af_release()?;  // Return to previous mode
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait PushAFControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Press Push AF button.
    ///
    /// Temporarily activates auto focus while the button is pressed.
    /// This allows quick focus adjustment without changing the focus mode.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn push_af_press(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Release Push AF button.
    ///
    /// Returns to the previous focus mode after temporary auto focus.
    /// Must be called after push_af_press() to end the temporary auto focus.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    fn push_af_release(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Implementation of PushAFControl for cameras with HasPushAutoFocus capability
impl<M, P, Tr, Exec> PushAFControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + crate::capabilities::HasPushAutoFocus + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn push_af_press(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(PushAF::Press)
    }

    fn push_af_release(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(PushAF::Release)
    }
}
