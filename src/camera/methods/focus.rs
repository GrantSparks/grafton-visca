//! Focus methods for cameras using mode markers.

use crate::{
    command::focus::{
        AutoFocusSensitivity, AutoFocusSensitivityCommand, Focus, FocusLock, FocusNearLimitCommand,
        FocusSpeed, FocusZone, FocusZoneCommand, PushAF,
    },
    types::{FocusPosition, SpeedLevel},
    Error,
};

/// Focus operations (async).
#[cfg(feature = "async")]
pub trait FocusOps: Sized {
    /// Set auto focus mode.
    async fn focus_auto(&self) -> Result<(), Error>;

    /// Set manual focus mode.
    async fn focus_manual(&self) -> Result<(), Error>;

    /// Focus near at specified speed.
    async fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error>;

    /// Focus far at specified speed.
    async fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error>;

    /// Stop focus movement.
    async fn focus_stop(&self) -> Result<(), Error>;

    /// Trigger one-push auto focus.
    async fn focus_one_push(&self) -> Result<(), Error>;

    /// Set focus to a specific position.
    async fn set_focus(&self, position: FocusPosition) -> Result<(), Error>;

    /// Set focus to infinity.
    async fn focus_infinity(&self) -> Result<(), Error>;

    /// Enable focus lock.
    /// Locks the current focus position to prevent changes.
    async fn enable_focus_lock(&self) -> Result<(), Error>;

    /// Disable focus lock.
    /// Allows focus to be adjusted again.
    async fn disable_focus_lock(&self) -> Result<(), Error>;

    /// Press Push AF button.
    /// Temporarily activates auto focus while pressed.
    async fn push_af_press(&self) -> Result<(), Error>;

    /// Release Push AF button.
    /// Returns to previous focus mode after temporary auto focus.
    async fn push_af_release(&self) -> Result<(), Error>;

    /// Set the focus zone.
    /// Determines which area of the image the camera uses for auto focus.
    async fn set_focus_zone(&self, zone: FocusZone) -> Result<(), Error>;

    /// Set auto focus sensitivity.
    /// Controls how responsive the auto focus system is to changes in the scene.
    async fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> Result<(), Error>;

    /// Set the focus near limit.
    /// Sets the minimum focus distance to prevent the camera from focusing on objects too close to the lens.
    async fn set_focus_near_limit(&self, position: FocusPosition) -> Result<(), Error>;
}

/// Focus operations (blocking).
pub trait FocusOpsBlocking: Sized {
    /// Set auto focus mode.
    fn focus_auto(&self) -> Result<(), Error>;

    /// Set manual focus mode.
    fn focus_manual(&self) -> Result<(), Error>;

    /// Focus near at specified speed.
    fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error>;

    /// Focus far at specified speed.
    fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error>;

    /// Stop focus movement.
    fn focus_stop(&self) -> Result<(), Error>;

    /// Trigger one-push auto focus.
    fn focus_one_push(&self) -> Result<(), Error>;

    /// Set focus to a specific position.
    fn set_focus(&self, position: FocusPosition) -> Result<(), Error>;

    /// Set focus to infinity.
    fn focus_infinity(&self) -> Result<(), Error>;

    /// Enable focus lock.
    /// Locks the current focus position to prevent changes.
    fn enable_focus_lock(&self) -> Result<(), Error>;

    /// Disable focus lock.
    /// Allows focus to be adjusted again.
    fn disable_focus_lock(&self) -> Result<(), Error>;

    /// Press Push AF button.
    /// Temporarily activates auto focus while pressed.
    fn push_af_press(&self) -> Result<(), Error>;

    /// Release Push AF button.
    /// Returns to previous focus mode after temporary auto focus.
    fn push_af_release(&self) -> Result<(), Error>;

    /// Set the focus zone.
    /// Determines which area of the image the camera uses for auto focus.
    fn set_focus_zone(&self, zone: FocusZone) -> Result<(), Error>;

    /// Set auto focus sensitivity.
    /// Controls how responsive the auto focus system is to changes in the scene.
    fn set_auto_focus_sensitivity(&self, sensitivity: AutoFocusSensitivity) -> Result<(), Error>;

    /// Set the focus near limit.
    /// Sets the minimum focus distance to prevent the camera from focusing on objects too close to the lens.
    fn set_focus_near_limit(&self, position: FocusPosition) -> Result<(), Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> FocusOps for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor_unified::Executor,
{
    async fn focus_auto(&self) -> Result<(), Error> {
        let cmd = Focus::Auto;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn focus_manual(&self) -> Result<(), Error> {
        let cmd = Focus::Manual;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Near
        } else {
            let focus_speed = FocusSpeed::new(focus_speed_val.min(7))?;
            Focus::NearWithSpeed(focus_speed)
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Far
        } else {
            let focus_speed = FocusSpeed::new(focus_speed_val.min(7))?;
            Focus::FarWithSpeed(focus_speed)
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn focus_stop(&self) -> Result<(), Error> {
        let cmd = Focus::Stop;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn focus_one_push(&self) -> Result<(), Error> {
        let cmd = Focus::OnePushTrigger;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_focus(&self, position: FocusPosition) -> Result<(), Error> {
        let cmd = Focus::Position(position);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn focus_infinity(&self) -> Result<(), Error> {
        let cmd = Focus::Infinity;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn enable_focus_lock(&self) -> Result<(), Error> {
        self.send_command(&FocusLock::On).await?;
        Ok(())
    }

    async fn disable_focus_lock(&self) -> Result<(), Error> {
        self.send_command(&FocusLock::Off).await?;
        Ok(())
    }

    async fn push_af_press(&self) -> Result<(), Error> {
        self.send_command(&PushAF::Press).await?;
        Ok(())
    }

    async fn push_af_release(&self) -> Result<(), Error> {
        self.send_command(&PushAF::Release).await?;
        Ok(())
    }

    async fn set_focus_zone(&self, zone: FocusZone) -> Result<(), Error> {
        self.send_command(&FocusZoneCommand { zone }).await?;
        Ok(())
    }

    async fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> Result<(), Error> {
        self.send_command(&AutoFocusSensitivityCommand { sensitivity })
            .await?;
        Ok(())
    }

    async fn set_focus_near_limit(&self, position: FocusPosition) -> Result<(), Error> {
        self.send_command(&FocusNearLimitCommand { position })
            .await?;
        Ok(())
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> FocusOpsBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn focus_auto(&self) -> Result<(), Error> {
        let cmd = Focus::Auto;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_manual(&self) -> Result<(), Error> {
        let cmd = Focus::Manual;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Near
        } else {
            let focus_speed = FocusSpeed::new(focus_speed_val.min(7))?;
            Focus::NearWithSpeed(focus_speed)
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed();
        let cmd = if focus_speed_val == 0 {
            Focus::Far
        } else {
            let focus_speed = FocusSpeed::new(focus_speed_val.min(7))?;
            Focus::FarWithSpeed(focus_speed)
        };
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_stop(&self) -> Result<(), Error> {
        let cmd = Focus::Stop;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_one_push(&self) -> Result<(), Error> {
        let cmd = Focus::OnePushTrigger;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_focus(&self, position: FocusPosition) -> Result<(), Error> {
        let cmd = Focus::Position(position);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_infinity(&self) -> Result<(), Error> {
        let cmd = Focus::Infinity;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn enable_focus_lock(&self) -> Result<(), Error> {
        self.send_command(&FocusLock::On)?;
        Ok(())
    }

    fn disable_focus_lock(&self) -> Result<(), Error> {
        self.send_command(&FocusLock::Off)?;
        Ok(())
    }

    fn push_af_press(&self) -> Result<(), Error> {
        self.send_command(&PushAF::Press)?;
        Ok(())
    }

    fn push_af_release(&self) -> Result<(), Error> {
        self.send_command(&PushAF::Release)?;
        Ok(())
    }

    fn set_focus_zone(&self, zone: FocusZone) -> Result<(), Error> {
        self.send_command(&FocusZoneCommand { zone })?;
        Ok(())
    }

    fn set_auto_focus_sensitivity(&self, sensitivity: AutoFocusSensitivity) -> Result<(), Error> {
        self.send_command(&AutoFocusSensitivityCommand { sensitivity })?;
        Ok(())
    }

    fn set_focus_near_limit(&self, position: FocusPosition) -> Result<(), Error> {
        self.send_command(&FocusNearLimitCommand { position })?;
        Ok(())
    }
}
