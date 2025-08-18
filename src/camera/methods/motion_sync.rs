//! Motion Sync control methods for PtzOptics cameras.

use crate::{error::Error, MotionSyncMode, MotionSyncSpeed};

/// Motion Sync control methods for cameras that support this feature.
#[cfg(feature = "async")]
pub trait MotionSyncControl {
    /// Sets the motion sync mode (on/off).
    ///
    /// This PtzOptics-specific feature coordinates pan, tilt, and zoom movements
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

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> MotionSyncControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile + crate::capabilities::motion_sync::MotionSync,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncModeCmd;

        let cmd = MotionSyncModeCmd::new(mode);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncSpeedCmd;

        let cmd = MotionSyncSpeedCmd::new(speed).map_err(|_| Error::InvalidParameter {
            parameter: "speed",
            value: speed.to_string().into(),
            reason: "must be between 1 and 24".into(),
        })?;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncSpeedCmd;

        let cmd = MotionSyncSpeedCmd::from_preset(speed);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn get_motion_sync_mode(&self) -> Result<MotionSyncMode, Error> {
        use crate::command::{
            inquiry::MotionSyncModeInquiry, response::ViscaResponse, InquiryResponse,
        };
        let inquiry = MotionSyncModeInquiry;
        let response = self.send_command(&inquiry).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::MotionSyncMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    async fn get_motion_sync_speed(&self) -> Result<MotionSyncSpeed, Error> {
        use crate::command::{
            inquiry::MotionSyncSpeedInquiry, response::ViscaResponse, InquiryResponse,
        };
        let inquiry = MotionSyncSpeedInquiry;
        let response = self.send_command(&inquiry).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::MotionSyncSpeed { speed }) => Ok(speed),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> MotionSyncControlBlocking
    for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::motion_sync::MotionSync,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncModeCmd;

        let cmd = MotionSyncModeCmd::new(mode);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncSpeedCmd;

        let cmd = MotionSyncSpeedCmd::new(speed).map_err(|_| Error::InvalidParameter {
            parameter: "speed",
            value: speed.to_string().into(),
            reason: "must be between 1 and 24".into(),
        })?;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncSpeedCmd;

        let cmd = MotionSyncSpeedCmd::from_preset(speed);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn get_motion_sync_mode(&self) -> Result<MotionSyncMode, Error> {
        use crate::command::{
            inquiry::MotionSyncModeInquiry, response::ViscaResponse, InquiryResponse,
        };
        let inquiry = MotionSyncModeInquiry;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::MotionSyncMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_motion_sync_speed(&self) -> Result<MotionSyncSpeed, Error> {
        use crate::command::{
            inquiry::MotionSyncSpeedInquiry, response::ViscaResponse, InquiryResponse,
        };
        let inquiry = MotionSyncSpeedInquiry;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::MotionSyncSpeed { speed }) => Ok(speed),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
