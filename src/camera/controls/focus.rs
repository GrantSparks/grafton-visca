//! focus control implementation using Mode trait.

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

/// focus operations for cameras.
///
/// This trait provides focus control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait FocusControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set auto focus mode.
    fn focus_auto(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set manual focus mode.
    fn focus_manual(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Focus near at specified speed.
    fn focus_near(&self, speed: SpeedLevel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Focus far at specified speed.
    fn focus_far(&self, speed: SpeedLevel) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Stop focus movement.
    fn focus_stop(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Trigger one-push auto focus.
    fn focus_one_push(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set focus to a specific position.
    fn set_focus(
        &self,
        position: FocusPosition,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set focus to infinity.
    fn focus_infinity(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Enable focus lock.
    /// Locks the current focus position to prevent changes.
    fn enable_focus_lock(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable focus lock.
    /// Allows focus to be adjusted again.
    fn disable_focus_lock(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Press Push AF button.
    /// Temporarily activates auto focus while pressed.
    fn push_af_press(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Release Push AF button.
    /// Returns to previous focus mode after temporary auto focus.
    fn push_af_release(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set the focus zone.
    /// Determines which area of the image the camera uses for auto focus.
    fn set_focus_zone(&self, zone: FocusZone) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set auto focus sensitivity.
    /// Controls how responsive the auto focus system is to changes in the scene.
    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set the focus near limit.
    /// Sets the minimum focus distance to prevent the camera from focusing on objects too close to the lens.
    fn set_focus_near_limit(
        &self,
        position: FocusPosition,
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

    fn set_focus(&self, position: FocusPosition) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::Position(position))
    }

    fn focus_infinity(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(Focus::Infinity)
    }

    fn enable_focus_lock(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(FocusLock::On)
    }

    fn disable_focus_lock(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(FocusLock::Off)
    }

    fn push_af_press(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(PushAF::Press)
    }

    fn push_af_release(&self) -> M::Fut<'_, Result<(), Error>> {
        self.execute(PushAF::Release)
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

    fn set_focus_near_limit(&self, position: FocusPosition) -> M::Fut<'_, Result<(), Error>> {
        self.execute(FocusNearLimitCommand { position })
    }
}
