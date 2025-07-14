//! Focus methods for cameras using the new GAT architecture.

use crate::{
    camera::Camera,
    command::focus::{Focus as FocusCommand, FocusSpeed},
    types::{FocusPosition, SpeedLevel},
    Error,
};

/// Focus operations (async).
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
}

// Async implementation
impl FocusOps for Camera {
    async fn focus_auto(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::Auto).await?;
        Ok(())
    }

    async fn focus_manual(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::Manual).await?;
        Ok(())
    }

    async fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed().min(7);
        let focus_speed = FocusSpeed::new(focus_speed_val)?;
        self.send_command(&FocusCommand::NearWithSpeed(focus_speed))
            .await?;
        Ok(())
    }

    async fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed().min(7);
        let focus_speed = FocusSpeed::new(focus_speed_val)?;
        self.send_command(&FocusCommand::FarWithSpeed(focus_speed))
            .await?;
        Ok(())
    }

    async fn focus_stop(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::Stop).await?;
        Ok(())
    }

    async fn focus_one_push(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::OnePushTrigger).await?;
        Ok(())
    }

    async fn set_focus(&self, position: FocusPosition) -> Result<(), Error> {
        self.send_command(&FocusCommand::Position(position)).await?;
        Ok(())
    }
}

// Blocking implementation
impl FocusOpsBlocking for Camera {
    fn focus_auto(&self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Auto)?;
        Ok(())
    }

    fn focus_manual(&self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Manual)?;
        Ok(())
    }

    fn focus_near(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed().min(7);
        let focus_speed = FocusSpeed::new(focus_speed_val)?;
        self.send_command_blocking(&FocusCommand::NearWithSpeed(focus_speed))?;
        Ok(())
    }

    fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed().min(7);
        let focus_speed = FocusSpeed::new(focus_speed_val)?;
        self.send_command_blocking(&FocusCommand::FarWithSpeed(focus_speed))?;
        Ok(())
    }

    fn focus_stop(&self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Stop)?;
        Ok(())
    }

    fn focus_one_push(&self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::OnePushTrigger)?;
        Ok(())
    }

    fn set_focus(&self, position: FocusPosition) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Position(position))?;
        Ok(())
    }
}
