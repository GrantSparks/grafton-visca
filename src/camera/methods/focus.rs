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
        let focus_speed_val = speed.to_focus_speed();
        if focus_speed_val == 0 {
            // Use standard speed command
            self.send_command(&FocusCommand::Near).await?;
        } else {
            // Use variable speed command
            let focus_speed = FocusSpeed::new(focus_speed_val.min(7))?;
            self.send_command(&FocusCommand::NearWithSpeed(focus_speed))
                .await?;
        }
        Ok(())
    }

    async fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed();
        if focus_speed_val == 0 {
            // Use standard speed command
            self.send_command(&FocusCommand::Far).await?;
        } else {
            // Use variable speed command
            let focus_speed = FocusSpeed::new(focus_speed_val.min(7))?;
            self.send_command(&FocusCommand::FarWithSpeed(focus_speed))
                .await?;
        }
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

    async fn focus_infinity(&self) -> Result<(), Error> {
        self.send_command(&FocusCommand::Infinity).await?;
        Ok(())
    }

    async fn enable_focus_lock(&self) -> Result<(), Error> {
        use crate::command::focus::FocusLock;
        self.send_command(&FocusLock::On).await?;
        Ok(())
    }

    async fn disable_focus_lock(&self) -> Result<(), Error> {
        use crate::command::focus::FocusLock;
        self.send_command(&FocusLock::Off).await?;
        Ok(())
    }

    async fn push_af_press(&self) -> Result<(), Error> {
        use crate::command::focus::PushAF;
        self.send_command(&PushAF::Press).await?;
        Ok(())
    }

    async fn push_af_release(&self) -> Result<(), Error> {
        use crate::command::focus::PushAF;
        self.send_command(&PushAF::Release).await?;
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
        let focus_speed_val = speed.to_focus_speed();
        if focus_speed_val == 0 {
            // Use standard speed command
            self.send_command_blocking(&FocusCommand::Near)?;
        } else {
            // Use variable speed command
            let focus_speed = FocusSpeed::new(focus_speed_val.min(7))?;
            self.send_command_blocking(&FocusCommand::NearWithSpeed(focus_speed))?;
        }
        Ok(())
    }

    fn focus_far(&self, speed: SpeedLevel) -> Result<(), Error> {
        let focus_speed_val = speed.to_focus_speed();
        if focus_speed_val == 0 {
            // Use standard speed command
            self.send_command_blocking(&FocusCommand::Far)?;
        } else {
            // Use variable speed command
            let focus_speed = FocusSpeed::new(focus_speed_val.min(7))?;
            self.send_command_blocking(&FocusCommand::FarWithSpeed(focus_speed))?;
        }
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

    fn focus_infinity(&self) -> Result<(), Error> {
        self.send_command_blocking(&FocusCommand::Infinity)?;
        Ok(())
    }

    fn enable_focus_lock(&self) -> Result<(), Error> {
        use crate::command::focus::FocusLock;
        self.send_command_blocking(&FocusLock::On)?;
        Ok(())
    }

    fn disable_focus_lock(&self) -> Result<(), Error> {
        use crate::command::focus::FocusLock;
        self.send_command_blocking(&FocusLock::Off)?;
        Ok(())
    }

    fn push_af_press(&self) -> Result<(), Error> {
        use crate::command::focus::PushAF;
        self.send_command_blocking(&PushAF::Press)?;
        Ok(())
    }

    fn push_af_release(&self) -> Result<(), Error> {
        use crate::command::focus::PushAF;
        self.send_command_blocking(&PushAF::Release)?;
        Ok(())
    }
}
