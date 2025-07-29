//! Motion Sync control methods for PTZOptics cameras.

use crate::{
    camera::Camera,
    command::motion_sync::{MotionSyncModeCommand, MotionSyncSpeedCommand},
    error::Error,
    MotionSyncMode, MotionSyncSpeed,
};

/// Motion Sync control methods for cameras that support this feature.
#[cfg(feature = "async")]
pub trait MotionSyncControl {
    /// Sets the motion sync mode (on/off).
    ///
    /// This PTZOptics-specific feature coordinates pan, tilt, and zoom movements
    /// for smoother preset recalls.
    ///
    /// # Arguments
    /// * `mode` - The motion sync mode to set
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    async fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error>;

    /// Sets the motion sync speed.
    ///
    /// # Arguments
    /// * `speed` - Speed value from 1 to 24
    ///
    /// # Errors
    /// Returns an error if:
    /// - The camera doesn't support motion sync
    /// - The speed is outside the valid range (1-24)
    async fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error>;

    /// Sets the motion sync speed using a preset value.
    ///
    /// # Arguments
    /// * `speed` - Preset speed (Slow, Normal, Fast)
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    async fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error>;

    /// Gets the current motion sync mode.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    async fn get_motion_sync_mode(&self) -> Result<MotionSyncMode, Error>;

    /// Gets the current motion sync speed.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    async fn get_motion_sync_speed(&self) -> Result<MotionSyncSpeed, Error>;
}

/// Blocking version of motion sync control methods.
pub trait MotionSyncControlBlocking {
    /// Sets the motion sync mode (on/off).
    fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error>;

    /// Sets the motion sync speed.
    fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error>;

    /// Sets the motion sync speed using a preset value.
    fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error>;

    /// Gets the current motion sync mode.
    fn get_motion_sync_mode(&self) -> Result<MotionSyncMode, Error>;

    /// Gets the current motion sync speed.
    fn get_motion_sync_speed(&self) -> Result<MotionSyncSpeed, Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl MotionSyncControl for Camera {
    async fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        let cmd = MotionSyncModeCommand::new(mode);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        let cmd = MotionSyncSpeedCommand::new(speed)?;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        let cmd = MotionSyncSpeedCommand::from_preset(speed);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn get_motion_sync_mode(&self) -> Result<MotionSyncMode, Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        use crate::command::inquiry::MotionSyncModeInquiry;
        use crate::command::{InquiryResponse, Response};

        let response = self.send_command(&MotionSyncModeInquiry).await?;
        match response {
            Response::Inquiry(InquiryResponse::MotionSyncMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_motion_sync_speed(&self) -> Result<MotionSyncSpeed, Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        use crate::command::inquiry::MotionSyncSpeedInquiry;
        use crate::command::{InquiryResponse, Response};

        let response = self.send_command(&MotionSyncSpeedInquiry).await?;
        match response {
            Response::Inquiry(InquiryResponse::MotionSyncSpeed { speed }) => Ok(speed),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation
impl MotionSyncControlBlocking for Camera {
    fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        let cmd = MotionSyncModeCommand::new(mode);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        let cmd = MotionSyncSpeedCommand::new(speed)?;
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        let cmd = MotionSyncSpeedCommand::from_preset(speed);
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn get_motion_sync_mode(&self) -> Result<MotionSyncMode, Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        use crate::command::inquiry::MotionSyncModeInquiry;
        use crate::command::{InquiryResponse, Response};

        let response = self.send_command_blocking(&MotionSyncModeInquiry)?;
        match response {
            Response::Inquiry(InquiryResponse::MotionSyncMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_motion_sync_speed(&self) -> Result<MotionSyncSpeed, Error> {
        if !self.supports_motion_sync() {
            return Err(Error::FeatureNotSupported {
                feature: "Motion Sync",
            });
        }

        use crate::command::inquiry::MotionSyncSpeedInquiry;
        use crate::command::{InquiryResponse, Response};

        let response = self.send_command_blocking(&MotionSyncSpeedInquiry)?;
        match response {
            Response::Inquiry(InquiryResponse::MotionSyncSpeed { speed }) => Ok(speed),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
