//! Focus methods for unified camera API.

use crate::{
    command::focus::{
        AutoFocusSensitivity, AutoFocusSensitivityCommand, Focus, FocusLock, FocusNearLimitCommand,
        FocusSpeed, FocusZone, FocusZoneCommand, PushAF,
    },
    types::{FocusPosition, SpeedLevel},
    Error,
};

/// Focus operations for cameras.
///
/// This trait provides focus control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait FocusControl {
    /// Set auto focus mode.
    #[cfg(feature = "async")]
    fn focus_auto(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set auto focus mode.
    #[cfg(not(feature = "async"))]
    fn focus_auto(&mut self) -> Result<(), Error>;

    /// Set manual focus mode.
    #[cfg(feature = "async")]
    fn focus_manual(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set manual focus mode.
    #[cfg(not(feature = "async"))]
    fn focus_manual(&mut self) -> Result<(), Error>;

    /// Focus near at specified speed.
    #[cfg(feature = "async")]
    fn focus_near(
        &self,
        speed: SpeedLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Focus near at specified speed.
    #[cfg(not(feature = "async"))]
    fn focus_near(&mut self, speed: SpeedLevel) -> Result<(), Error>;

    /// Focus far at specified speed.
    #[cfg(feature = "async")]
    fn focus_far(
        &self,
        speed: SpeedLevel,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Focus far at specified speed.
    #[cfg(not(feature = "async"))]
    fn focus_far(&mut self, speed: SpeedLevel) -> Result<(), Error>;

    /// Stop focus movement.
    #[cfg(feature = "async")]
    fn focus_stop(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Stop focus movement.
    #[cfg(not(feature = "async"))]
    fn focus_stop(&mut self) -> Result<(), Error>;

    /// Trigger one-push auto focus.
    #[cfg(feature = "async")]
    fn focus_one_push(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Trigger one-push auto focus.
    #[cfg(not(feature = "async"))]
    fn focus_one_push(&mut self) -> Result<(), Error>;

    /// Set focus to a specific position.
    #[cfg(feature = "async")]
    fn set_focus(
        &self,
        position: FocusPosition,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set focus to a specific position.
    #[cfg(not(feature = "async"))]
    fn set_focus(&mut self, position: FocusPosition) -> Result<(), Error>;

    /// Set focus to infinity.
    #[cfg(feature = "async")]
    fn focus_infinity(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set focus to infinity.
    #[cfg(not(feature = "async"))]
    fn focus_infinity(&mut self) -> Result<(), Error>;

    /// Enable focus lock.
    /// Locks the current focus position to prevent changes.
    #[cfg(feature = "async")]
    fn enable_focus_lock(&self)
        -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable focus lock.
    /// Locks the current focus position to prevent changes.
    #[cfg(not(feature = "async"))]
    fn enable_focus_lock(&mut self) -> Result<(), Error>;

    /// Disable focus lock.
    /// Allows focus to be adjusted again.
    #[cfg(feature = "async")]
    fn disable_focus_lock(
        &self,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Disable focus lock.
    /// Allows focus to be adjusted again.
    #[cfg(not(feature = "async"))]
    fn disable_focus_lock(&mut self) -> Result<(), Error>;

    /// Press Push AF button.
    /// Temporarily activates auto focus while pressed.
    #[cfg(feature = "async")]
    fn push_af_press(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Press Push AF button.
    /// Temporarily activates auto focus while pressed.
    #[cfg(not(feature = "async"))]
    fn push_af_press(&mut self) -> Result<(), Error>;

    /// Release Push AF button.
    /// Returns to previous focus mode after temporary auto focus.
    #[cfg(feature = "async")]
    fn push_af_release(&self) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Release Push AF button.
    /// Returns to previous focus mode after temporary auto focus.
    #[cfg(not(feature = "async"))]
    fn push_af_release(&mut self) -> Result<(), Error>;

    /// Set the focus zone.
    /// Determines which area of the image the camera uses for auto focus.
    #[cfg(feature = "async")]
    fn set_focus_zone(
        &self,
        zone: FocusZone,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set the focus zone.
    /// Determines which area of the image the camera uses for auto focus.
    #[cfg(not(feature = "async"))]
    fn set_focus_zone(&mut self, zone: FocusZone) -> Result<(), Error>;

    /// Set auto focus sensitivity.
    /// Controls how responsive the auto focus system is to changes in the scene.
    #[cfg(feature = "async")]
    fn set_auto_focus_sensitivity(
        &self,
        sensitivity: AutoFocusSensitivity,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set auto focus sensitivity.
    /// Controls how responsive the auto focus system is to changes in the scene.
    #[cfg(not(feature = "async"))]
    fn set_auto_focus_sensitivity(
        &mut self,
        sensitivity: AutoFocusSensitivity,
    ) -> Result<(), Error>;

    /// Set the focus near limit.
    /// Sets the minimum focus distance to prevent the camera from focusing on objects too close to the lens.
    #[cfg(feature = "async")]
    fn set_focus_near_limit(
        &self,
        position: FocusPosition,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set the focus near limit.
    /// Sets the minimum focus distance to prevent the camera from focusing on objects too close to the lens.
    #[cfg(not(feature = "async"))]
    fn set_focus_near_limit(&mut self, position: FocusPosition) -> Result<(), Error>;
}

// Keep the old trait names for backward compatibility during transition
#[cfg(feature = "async")]
/// Async focus control trait (deprecated, use FocusControl instead).
pub trait FocusControlAsync: FocusControl {}

#[cfg(not(feature = "async"))]
/// Blocking focus control trait (deprecated, use FocusControl instead).
pub trait FocusControlBlocking: FocusControl {}

// Async implementation for AsyncCamera
#[cfg(feature = "async")]
impl<P, Tr, Exec> FocusControl for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> FocusControl for crate::camera::BlockingCamera<P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::BlockingTransport + Send + 'static,
{
    fn focus_auto(&mut self) -> Result<(), Error> {
        let cmd = Focus::Auto;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_manual(&mut self) -> Result<(), Error> {
        let cmd = Focus::Manual;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_near(&mut self, speed: SpeedLevel) -> Result<(), Error> {
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

    fn focus_far(&mut self, speed: SpeedLevel) -> Result<(), Error> {
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

    fn focus_stop(&mut self) -> Result<(), Error> {
        let cmd = Focus::Stop;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_one_push(&mut self) -> Result<(), Error> {
        let cmd = Focus::OnePushTrigger;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_focus(&mut self, position: FocusPosition) -> Result<(), Error> {
        let cmd = Focus::Position(position);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn focus_infinity(&mut self) -> Result<(), Error> {
        let cmd = Focus::Infinity;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn enable_focus_lock(&mut self) -> Result<(), Error> {
        self.send_command(&FocusLock::On)?;
        Ok(())
    }

    fn disable_focus_lock(&mut self) -> Result<(), Error> {
        self.send_command(&FocusLock::Off)?;
        Ok(())
    }

    fn push_af_press(&mut self) -> Result<(), Error> {
        self.send_command(&PushAF::Press)?;
        Ok(())
    }

    fn push_af_release(&mut self) -> Result<(), Error> {
        self.send_command(&PushAF::Release)?;
        Ok(())
    }

    fn set_focus_zone(&mut self, zone: FocusZone) -> Result<(), Error> {
        self.send_command(&FocusZoneCommand { zone })?;
        Ok(())
    }

    fn set_auto_focus_sensitivity(
        &mut self,
        sensitivity: AutoFocusSensitivity,
    ) -> Result<(), Error> {
        self.send_command(&AutoFocusSensitivityCommand { sensitivity })?;
        Ok(())
    }

    fn set_focus_near_limit(&mut self, position: FocusPosition) -> Result<(), Error> {
        self.send_command(&FocusNearLimitCommand { position })?;
        Ok(())
    }
}

// Backward compatibility implementations
#[cfg(feature = "async")]
impl<P, Tr, Exec> FocusControlAsync for crate::camera::AsyncCamera<P, Tr, Exec>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
}

#[cfg(not(feature = "async"))]
impl<P, Tr> FocusControlBlocking for crate::camera::BlockingCamera<P, Tr>
where
    P: crate::capabilities::Profile + Default,
    Tr: crate::transport::BlockingTransport + Send + 'static,
{
}
