//! Motion Sync control methods for PtzOptics cameras.

use crate::{error::Error, MotionSyncMode, MotionSyncSpeed};

/// Motion Sync control methods for cameras that support this feature.
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
    #[cfg(feature = "async")]
    fn set_motion_sync_mode(
        &self,
        mode: MotionSyncMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

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
    #[cfg(not(feature = "async"))]
    fn set_motion_sync_mode(&mut self, mode: MotionSyncMode) -> Result<(), Error>;

    /// Sets the motion sync speed.
    ///
    /// # Arguments
    /// * `speed` - Speed value from 1 to 24
    ///
    /// # Errors
    /// Returns an error if:
    /// - The camera doesn't support motion sync
    /// - The speed is outside the valid range (1-24)
    #[cfg(feature = "async")]
    fn set_motion_sync_speed(
        &self,
        speed: u8,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Sets the motion sync speed.
    ///
    /// # Arguments
    /// * `speed` - Speed value from 1 to 24
    ///
    /// # Errors
    /// Returns an error if:
    /// - The camera doesn't support motion sync
    /// - The speed is outside the valid range (1-24)
    #[cfg(not(feature = "async"))]
    fn set_motion_sync_speed(&mut self, speed: u8) -> Result<(), Error>;

    /// Sets the motion sync speed using a preset value.
    ///
    /// # Arguments
    /// * `speed` - Preset speed (Slow, Normal, Fast)
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    #[cfg(feature = "async")]
    fn set_motion_sync_preset_speed(
        &self,
        speed: MotionSyncSpeed,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Sets the motion sync speed using a preset value.
    ///
    /// # Arguments
    /// * `speed` - Preset speed (Slow, Normal, Fast)
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    #[cfg(not(feature = "async"))]
    fn set_motion_sync_preset_speed(&mut self, speed: MotionSyncSpeed) -> Result<(), Error>;

    /// Gets the current motion sync mode.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    #[cfg(feature = "async")]
    fn get_motion_sync_mode(
        &self,
    ) -> impl std::future::Future<Output = Result<MotionSyncMode, Error>> + Send + '_;

    /// Gets the current motion sync mode.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    #[cfg(not(feature = "async"))]
    fn get_motion_sync_mode(&mut self) -> Result<MotionSyncMode, Error>;

    /// Gets the current motion sync speed.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    #[cfg(feature = "async")]
    fn get_motion_sync_speed(
        &self,
    ) -> impl std::future::Future<Output = Result<MotionSyncSpeed, Error>> + Send + '_;

    /// Gets the current motion sync speed.
    ///
    /// # Errors
    /// Returns an error if the camera doesn't support motion sync.
    #[cfg(not(feature = "async"))]
    fn get_motion_sync_speed(&mut self) -> Result<MotionSyncSpeed, Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, Tr, Exec> MotionSyncControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + crate::capabilities::motion_sync::MotionSync + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn set_motion_sync_mode(&self, mode: MotionSyncMode) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncModeCommand;
        let cmd = MotionSyncModeCommand::new(mode);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_motion_sync_speed(&self, speed: u8) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncSpeedCommand;

        let cmd = MotionSyncSpeedCommand::new(speed).map_err(|_| Error::InvalidParameter {
            parameter: "speed",
            value: speed.to_string().into(),
            reason: "must be between 1 and 24".into(),
        })?;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_motion_sync_preset_speed(&self, speed: MotionSyncSpeed) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncSpeedCommand;

        let cmd = MotionSyncSpeedCommand::from_preset(speed);
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> MotionSyncControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::motion_sync::MotionSync + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn set_motion_sync_mode(&mut self, mode: MotionSyncMode) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncModeCommand;
        let cmd = MotionSyncModeCommand::new(mode);
        self.send_command(&cmd).into_inner()?;
        Ok(())
    }

    fn set_motion_sync_speed(&mut self, speed: u8) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncSpeedCommand;

        let cmd = MotionSyncSpeedCommand::new(speed).map_err(|_| Error::InvalidParameter {
            parameter: "speed",
            value: speed.to_string().into(),
            reason: "must be between 1 and 24".into(),
        })?;
        self.send_command(&cmd).into_inner()?;
        Ok(())
    }

    fn set_motion_sync_preset_speed(&mut self, speed: MotionSyncSpeed) -> Result<(), Error> {
        use crate::command::motion_sync::MotionSyncSpeedCommand;

        let cmd = MotionSyncSpeedCommand::from_preset(speed);
        self.send_command(&cmd).into_inner()?;
        Ok(())
    }

    fn get_motion_sync_mode(&mut self) -> Result<MotionSyncMode, Error> {
        use crate::command::{
            inquiry::MotionSyncModeInquiry, response::ViscaResponse, InquiryResponse,
        };
        let inquiry = MotionSyncModeInquiry;
        let response = self.send_command(&inquiry).into_inner()?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::MotionSyncMode { mode }) => Ok(mode),
            _ => Err(Error::UnexpectedResponseType),
        }
    }

    fn get_motion_sync_speed(&mut self) -> Result<MotionSyncSpeed, Error> {
        use crate::command::{
            inquiry::MotionSyncSpeedInquiry, response::ViscaResponse, InquiryResponse,
        };
        let inquiry = MotionSyncSpeedInquiry;
        let response = self.send_command(&inquiry).into_inner()?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::MotionSyncSpeed { speed }) => Ok(speed),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
