//! Focus methods for cameras using the new GAT architecture.

use crate::{
    camera::unified::Camera,
    command::focus::{Focus as FocusCommand, FocusSpeed},
    types::{FocusPosition, SpeedLevel},
    Error,
};

#[cfg(feature = "tokio")]
use core::future::Future;
/// Extension trait for Camera that adds focus methods.
pub trait FocusOps: Sized {
    /// Set auto focus mode.
    #[cfg(feature = "tokio")]
    fn focus_auto(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set auto focus mode (blocking).
    #[cfg(not(feature = "tokio"))]
    fn focus_auto_blocking(&mut self) -> Result<(), Error>;

    /// Set manual focus mode.
    #[cfg(feature = "tokio")]
    fn focus_manual(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set manual focus mode (blocking).
    #[cfg(not(feature = "tokio"))]
    fn focus_manual_blocking(&mut self) -> Result<(), Error>;

    /// Focus near at specified speed.
    #[cfg(feature = "tokio")]
    fn focus_near(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + Send;

    /// Focus near at specified speed (blocking).
    #[cfg(not(feature = "tokio"))]
    fn focus_near_blocking(&mut self, speed: SpeedLevel) -> Result<(), Error>;

    /// Focus far at specified speed.
    #[cfg(feature = "tokio")]
    fn focus_far(&self, speed: SpeedLevel) -> impl Future<Output = Result<(), Error>> + Send;

    /// Focus far at specified speed (blocking).
    #[cfg(not(feature = "tokio"))]
    fn focus_far_blocking(&mut self, speed: SpeedLevel) -> Result<(), Error>;

    /// Stop focus movement.
    #[cfg(feature = "tokio")]
    fn focus_stop(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Stop focus movement (blocking).
    #[cfg(not(feature = "tokio"))]
    fn focus_stop_blocking(&mut self) -> Result<(), Error>;

    /// Trigger one-push auto focus.
    #[cfg(feature = "tokio")]
    fn focus_one_push(&self) -> impl Future<Output = Result<(), Error>> + Send;

    /// Trigger one-push auto focus (blocking).
    #[cfg(not(feature = "tokio"))]
    fn focus_one_push_blocking(&mut self) -> Result<(), Error>;

    /// Set focus to a specific position.
    #[cfg(feature = "tokio")]
    fn set_focus(&self, position: FocusPosition) -> impl Future<Output = Result<(), Error>> + Send;

    /// Set focus to a specific position (blocking).
    #[cfg(not(feature = "tokio"))]
    fn set_focus_blocking(&mut self, position: FocusPosition) -> Result<(), Error>;
}

impl FocusOps for Camera {
    #[cfg(feature = "tokio")]
    async fn focus_auto(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::Auto).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn focus_auto_blocking(&mut self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Auto)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn focus_manual(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::Manual).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn focus_manual_blocking(&mut self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Manual)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed().min(7);
        let focus_speed = FocusSpeed::new(focus_speed_val)?;
        self.send_command(&FocusCommand::NearWithSpeed(focus_speed))
            .await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn focus_near_blocking(&mut self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed().min(7);
        let focus_speed = FocusSpeed::new(focus_speed_val)?;
        self.send_command_blocking(&FocusCommand::NearWithSpeed(focus_speed))?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed().min(7);
        let focus_speed = FocusSpeed::new(focus_speed_val)?;
        self.send_command(&FocusCommand::FarWithSpeed(focus_speed))
            .await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn focus_far_blocking(&mut self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed().min(7);
        let focus_speed = FocusSpeed::new(focus_speed_val)?;
        self.send_command_blocking(&FocusCommand::FarWithSpeed(focus_speed))?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn focus_stop(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::Stop).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn focus_stop_blocking(&mut self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Stop)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn focus_one_push(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::OnePushTrigger).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn focus_one_push_blocking(&mut self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::OnePushTrigger)?;
        Ok(())
    }
    #[cfg(feature = "tokio")]
    async fn set_focus(&self, position: FocusPosition) -> Result<(), Error> {
        self.send_command(&FocusCommand::Position(position)).await?;
        Ok(())
    }

    #[cfg(not(feature = "tokio"))]
    fn set_focus_blocking(&mut self, position: FocusPosition) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Position(position))?;
        Ok(())
    }
}
