//! Mode-parametrized focus control trait using the Mode trait system.

use crate::{
    command::focus::{
        AutoFocusSensitivity, AutoFocusSensitivityCommand, Focus, FocusLock, FocusNearLimitCommand,
        FocusSpeed, FocusZone, FocusZoneCommand, PushAF,
    },
    mode::Mode,
    types::{FocusPosition, SpeedLevel},
    Error,
};

/// Unified focus control trait that works with both blocking and async modes.
///
/// This trait uses the Mode trait system to provide a single API surface
/// that works correctly in both blocking and async contexts. The return
/// types adapt automatically based on the Mode parameter.
///
/// # Examples
///
/// ```rust,ignore
/// use grafton_visca::{Camera, mode::{Async, Blocking}};
/// use grafton_visca::camera::controls::unified_focus::UnifiedFocusControl;
/// use grafton_visca::types::SpeedLevel;
///
/// // Async usage
/// let async_camera: Camera<Async, Profile, Transport, Executor> = ...;
/// async_camera.focus_auto().await?; // Returns a future
/// async_camera.focus_near(SpeedLevel::Mid).await?;
///
/// // Blocking usage  
/// let blocking_camera: Camera<Blocking, Profile, Transport, ()> = ...;
/// blocking_camera.focus_auto().await?; // Returns immediately via Ready<T>
/// blocking_camera.focus_near(SpeedLevel::Mid).await?;
/// ```
pub trait UnifiedFocusControl<M>
where
    M: Mode,
{
    /// Set auto focus mode.
    fn focus_auto(&self) -> M::Ret<Result<(), Error>>;

    /// Set manual focus mode.
    fn focus_manual(&self) -> M::Ret<Result<(), Error>>;

    /// Focus near at specified speed.
    fn focus_near(&self, speed: SpeedLevel) -> M::Ret<Result<(), Error>>;

    /// Focus far at specified speed.
    fn focus_far(&self, speed: SpeedLevel) -> M::Ret<Result<(), Error>>;

    /// Stop focus movement.
    fn focus_stop(&self) -> M::Ret<Result<(), Error>>;

    /// Trigger one-push auto focus.
    fn focus_one_push(&self) -> M::Ret<Result<(), Error>>;

    /// Set focus to a specific position.
    fn set_focus(&self, position: FocusPosition) -> M::Ret<Result<(), Error>>;

    /// Set focus to infinity.
    fn focus_infinity(&self) -> M::Ret<Result<(), Error>>;

    /// Enable focus lock.
    /// Locks the current focus position to prevent changes.
    fn enable_focus_lock(&self) -> M::Ret<Result<(), Error>>;

    /// Disable focus lock.
    /// Allows focus to be adjusted again.
    fn disable_focus_lock(&self) -> M::Ret<Result<(), Error>>;

    /// Press Push AF button.
    /// Temporarily activates auto focus while pressed.
    fn push_af_press(&self) -> M::Ret<Result<(), Error>>;

    /// Release Push AF button.
    /// Returns to previous focus mode after temporary auto focus.
    fn push_af_release(&self) -> M::Ret<Result<(), Error>>;

    /// Set the focus zone.
    /// Determines which area of the image the camera uses for auto focus.
    fn set_focus_zone(&self, zone: FocusZone) -> M::Ret<Result<(), Error>>;

    /// Set auto focus sensitivity.
    /// Controls how responsive the auto focus system is to changes in the scene.
    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> M::Ret<Result<(), Error>>;

    /// Set the focus near limit.
    /// Sets the minimum focus distance to prevent the camera from focusing on objects too close to the lens.
    fn set_focus_near_limit(&self, position: FocusPosition) -> M::Ret<Result<(), Error>>;
}

// Implementation for the unified camera type
impl<M, P, Tr, Exec> UnifiedFocusControl<M> for crate::camera::unified::Camera<M, P, Tr, Exec>
where
    M: Mode + 'static,
    P: crate::capabilities::Profile + Default,
    Tr: Send + Sync,
{
    fn focus_auto(&self) -> M::Ret<Result<(), Error>> {
        let cmd = Focus::Auto;
        self.send_command(&cmd)
    }

    fn focus_manual(&self) -> M::Ret<Result<(), Error>> {
        let cmd = Focus::Manual;
        self.send_command(&cmd)
    }

    fn focus_near(&self, speed: SpeedLevel) -> M::Ret<Result<(), Error>> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Near
        } else {
            match FocusSpeed::new(focus_speed_val.min(7)) {
                Ok(focus_speed) => Focus::NearWithSpeed(focus_speed),
                Err(e) => return M::ret(Err(e)),
            }
        };
        self.send_command(&cmd)
    }

    fn focus_far(&self, speed: SpeedLevel) -> M::Ret<Result<(), Error>> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Far
        } else {
            match FocusSpeed::new(focus_speed_val.min(7)) {
                Ok(focus_speed) => Focus::FarWithSpeed(focus_speed),
                Err(e) => return M::ret(Err(e)),
            }
        };
        self.send_command(&cmd)
    }

    fn focus_stop(&self) -> M::Ret<Result<(), Error>> {
        let cmd = Focus::Stop;
        self.send_command(&cmd)
    }

    fn focus_one_push(&self) -> M::Ret<Result<(), Error>> {
        let cmd = Focus::OnePushTrigger;
        self.send_command(&cmd)
    }

    fn set_focus(&self, position: FocusPosition) -> M::Ret<Result<(), Error>> {
        let cmd = Focus::Position(position);
        self.send_command(&cmd)
    }

    fn focus_infinity(&self) -> M::Ret<Result<(), Error>> {
        let cmd = Focus::Infinity;
        self.send_command(&cmd)
    }

    fn enable_focus_lock(&self) -> M::Ret<Result<(), Error>> {
        let cmd = FocusLock::On;
        self.send_command(&cmd)
    }

    fn disable_focus_lock(&self) -> M::Ret<Result<(), Error>> {
        let cmd = FocusLock::Off;
        self.send_command(&cmd)
    }

    fn push_af_press(&self) -> M::Ret<Result<(), Error>> {
        let cmd = PushAF::Press;
        self.send_command(&cmd)
    }

    fn push_af_release(&self) -> M::Ret<Result<(), Error>> {
        let cmd = PushAF::Release;
        self.send_command(&cmd)
    }

    fn set_focus_zone(&self, zone: FocusZone) -> M::Ret<Result<(), Error>> {
        let cmd = FocusZoneCommand { zone };
        self.send_command(&cmd)
    }

    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> M::Ret<Result<(), Error>> {
        let cmd = AutoFocusSensitivityCommand { sensitivity };
        self.send_command(&cmd)
    }

    fn set_focus_near_limit(&self, position: FocusPosition) -> M::Ret<Result<(), Error>> {
        let cmd = FocusNearLimitCommand { position };
        self.send_command(&cmd)
    }
}

#[cfg(test)]
mod tests {
    use crate::mode::{Async, Blocking};

    #[tokio::test]
    async fn test_unified_focus_control_concept() {
        // This test demonstrates the concept - actual implementation would need
        // real camera instances

        // The key insight is that both async and blocking modes can be awaited:
        // - Async returns actual futures
        // - Blocking returns Ready<T> which immediately resolves

        // Example usage (conceptual):
        // async_camera.focus_auto().await?;
        // blocking_camera.focus_auto().await?;
        // Both work with the same method signature!
    }

    #[test]
    fn test_mode_markers_are_zero_sized() {
        use std::mem::size_of;
        assert_eq!(size_of::<Async>(), 0);
        assert_eq!(size_of::<Blocking>(), 0);
    }
}
