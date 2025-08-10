//! State management implementation for Camera

use std::time::Duration;

use crate::{
    camera::generic::Camera,
    capabilities::Profile,
    types::SpeedLevel,
    units::{Degrees, Normalized},
    Error, Result,
};

/// Camera state for saving and restoring position
#[derive(Debug, Clone, Copy)]
pub struct CameraState {
    /// Pan position in degrees
    pub pan: f32,
    /// Tilt position in degrees
    pub tilt: f32,
    /// Raw zoom value (0-16384)
    pub zoom: u16,
}

impl CameraState {
    /// Get normalized zoom value (0.0 to 1.0)
    pub fn zoom_normalized(&self) -> f32 {
        self.zoom as f32 / 16384.0
    }
}

impl<P, T> Camera<P, T>
where
    P: Profile,
    T: crate::transport::Transport + Send + Sync + 'static,
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    /// Save the current camera state
    pub fn save_state(&self) -> Result<CameraState>
    where
        Self: crate::camera::methods::InquiryOpsBlocking
            + crate::camera::methods::PanTiltInquiryOpsBlocking,
    {
        use crate::camera::methods::{InquiryOpsBlocking, PanTiltInquiryOpsBlocking};

        let (pan, tilt) = self.get_pan_tilt_degrees()?;
        let zoom = self.get_zoom_position()?;

        Ok(CameraState {
            pan: pan.0,
            tilt: tilt.0,
            zoom,
        })
    }

    /// Restore camera to a previously saved state
    pub fn restore_state(&self, state: &CameraState) -> Result<()>
    where
        Self: crate::camera::methods::PanTiltOpsBlocking
            + crate::camera::methods::ZoomOpsBlocking
            + crate::camera::helpers::MovementOps,
    {
        self.restore_state_with_speed(state, SpeedLevel::Fast)
    }

    /// Restore camera to a previously saved state with custom speed
    pub fn restore_state_with_speed(&self, state: &CameraState, speed: SpeedLevel) -> Result<()>
    where
        Self: crate::camera::methods::PanTiltOpsBlocking
            + crate::camera::methods::ZoomOpsBlocking
            + crate::camera::helpers::MovementOps,
    {
        use crate::camera::helpers::MovementOps;
        use crate::camera::methods::{PanTiltOpsBlocking, ZoomOpsBlocking};

        self.pan_tilt_absolute(Degrees(state.pan), Degrees(state.tilt), speed)?;
        MovementOps::await_idle(self, Duration::from_secs(30))?;

        let normalized_zoom = state.zoom_normalized();
        self.zoom_absolute(Normalized(normalized_zoom))?;
        MovementOps::await_idle(self, Duration::from_secs(10))?;

        Ok(())
    }
}

#[cfg(feature = "async")]
impl<P: Profile, T: crate::transport::Transport + Send + Sync> Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    /// Save the current camera state (async)
    #[cfg(feature = "tokio")]
    pub async fn save_state_async(&self) -> Result<CameraState>
    where
        Self: crate::camera::methods::InquiryOps + crate::camera::methods::PanTiltInquiryOps,
    {
        use crate::camera::methods::{InquiryOps, PanTiltInquiryOps};

        tokio::time::sleep(Duration::from_millis(500)).await;

        let (pan, tilt) = self.get_pan_tilt_degrees().await?;

        let mut zoom = self.get_zoom_position().await?;
        if zoom == 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            zoom = self.get_zoom_position().await?;
        }

        Ok(CameraState {
            pan: pan.0,
            tilt: tilt.0,
            zoom,
        })
    }

    /// Save the current camera state (async without tokio)
    #[cfg(all(feature = "async", not(feature = "tokio")))]
    pub async fn save_state_async(&self) -> Result<CameraState>
    where
        Self: crate::camera::methods::InquiryOps + crate::camera::methods::PanTiltInquiryOps,
    {
        use crate::camera::methods::{InquiryOps, PanTiltInquiryOps};

        let (pan, tilt) = self.get_pan_tilt_degrees().await?;
        let zoom = self.get_zoom_position().await?;

        Ok(CameraState {
            pan: pan.0,
            tilt: tilt.0,
            zoom,
        })
    }

    /// Restore camera to a previously saved state (async)
    pub async fn restore_state_async(&self, state: &CameraState) -> Result<()>
    where
        Self: crate::camera::methods::PanTiltOps
            + crate::camera::methods::ZoomOps
            + crate::camera::helpers::MovementOpsAsync,
    {
        self.restore_state_with_speed_async(state, SpeedLevel::Fast)
            .await
    }

    /// Restore camera to a previously saved state with custom speed (async)
    pub async fn restore_state_with_speed_async(
        &self,
        state: &CameraState,
        speed: SpeedLevel,
    ) -> Result<()>
    where
        Self: crate::camera::methods::PanTiltOps
            + crate::camera::methods::ZoomOps
            + crate::camera::helpers::MovementOpsAsync,
    {
        use crate::camera::helpers::MovementOpsAsync;
        use crate::camera::methods::{PanTiltOps, ZoomOps};

        self.pan_tilt_absolute(Degrees(state.pan), Degrees(state.tilt), speed)
            .await?;
        MovementOpsAsync::await_idle(self, Duration::from_secs(30)).await?;

        let normalized_zoom = state.zoom_normalized();
        self.zoom_absolute(Normalized(normalized_zoom)).await?;
        MovementOpsAsync::await_idle(self, Duration::from_secs(10)).await?;

        Ok(())
    }
}
