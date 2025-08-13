//! State management implementation for Camera

use std::time::Duration;

use crate::{
    camera::generic::Camera, capabilities::Profile, types::SpeedLevel, units::Normalized, Result,
};

#[cfg(feature = "async")]
use crate::Error;

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

#[cfg(not(feature = "async"))]
impl<P, T> Camera<crate::camera::BlockingMode, P, T>
where
    P: Profile,
    T: crate::transport::BlockingTransport + Send + Sync + 'static,
{
    /// Save the current camera state
    pub fn save_state(&self) -> Result<CameraState> {
        let (pan, tilt) = self.get_pan_tilt_degrees()?;
        let zoom = self.get_zoom_position()?;

        Ok(CameraState {
            pan: pan.0,
            tilt: tilt.0,
            zoom,
        })
    }

    /// Restore camera to a previously saved state
    pub fn restore_state(&self, state: &CameraState) -> Result<()> {
        self.restore_state_with_speed(state, SpeedLevel::Fast)
    }

    /// Restore camera to a previously saved state with custom speed
    #[allow(clippy::expect_used)]
    pub fn restore_state_with_speed(&self, state: &CameraState, speed: SpeedLevel) -> Result<()> {
        use crate::camera::helpers::MovementOpsBlocking;
        use crate::types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed};

        let pan_pos = PanPosition::from_degrees(state.pan)?;
        let tilt_pos = TiltPosition::from_degrees(state.tilt)?;
        let speed_val = match speed {
            SpeedLevel::Slowest => 1,
            SpeedLevel::Slow => 5,
            SpeedLevel::Medium => 12,
            SpeedLevel::Fast => 18,
            SpeedLevel::Fastest => 24,
        };
        let pan_speed = PanSpeed::new(speed_val).expect("speed_val is valid");
        let tilt_speed = TiltSpeed::new(speed_val).expect("speed_val is valid");

        self.pan_tilt_absolute(pan_pos, tilt_pos, pan_speed, tilt_speed)?;
        MovementOpsBlocking::await_idle(self, Duration::from_secs(30))?;

        let normalized_zoom = state.zoom_normalized();
        self.zoom_absolute(Normalized(normalized_zoom))?;
        MovementOpsBlocking::await_idle(self, Duration::from_secs(10))?;

        Ok(())
    }
}

#[cfg(feature = "async")]
impl<P, T> Camera<crate::camera::AsyncMode, P, T>
where
    P: Profile,
    T: crate::transport::AsyncTransport + 'static,
{
    /// Save the current camera state (async)
    #[cfg(feature = "async")]
    pub async fn save_state_async(&self) -> Result<CameraState> {
        // Use runtime for sleep
        let runtime = if let Some(runtime) = self.runtime() {
            std::sync::Arc::clone(runtime)
        } else {
            return Err(Error::MissingRuntime);
        };

        runtime.sleep(Duration::from_millis(500)).await;

        let (pan, tilt) = self.get_pan_tilt_degrees().await?;

        let mut zoom = self.get_zoom_position().await?;
        if zoom == 0 {
            runtime.sleep(Duration::from_millis(500)).await;
            zoom = self.get_zoom_position().await?;
        }

        Ok(CameraState {
            pan: pan.0,
            tilt: tilt.0,
            zoom,
        })
    }

    /// Restore camera to a previously saved state (async)
    pub async fn restore_state_async(&self, state: &CameraState) -> Result<()> {
        self.restore_state_with_speed_async(state, SpeedLevel::Fast)
            .await
    }

    /// Restore camera to a previously saved state with custom speed (async)
    #[allow(clippy::expect_used)]
    pub async fn restore_state_with_speed_async(
        &self,
        state: &CameraState,
        speed: SpeedLevel,
    ) -> Result<()> {
        use crate::camera::helpers::MovementOps;

        use crate::types::{PanPosition, PanSpeed, TiltPosition, TiltSpeed};
        let pan_pos = PanPosition::from_degrees(state.pan)?;
        let tilt_pos = TiltPosition::from_degrees(state.tilt)?;
        let speed_val = match speed {
            SpeedLevel::Slowest => 1,
            SpeedLevel::Slow => 5,
            SpeedLevel::Medium => 12,
            SpeedLevel::Fast => 18,
            SpeedLevel::Fastest => 24,
        };
        let pan_speed = PanSpeed::new(speed_val).expect("speed_val is valid");
        let tilt_speed = TiltSpeed::new(speed_val).expect("speed_val is valid");
        self.pan_tilt_absolute(pan_pos, tilt_pos, pan_speed, tilt_speed)
            .await?;
        MovementOps::await_idle(self, Duration::from_secs(30)).await?;

        let normalized_zoom = state.zoom_normalized();
        self.zoom_absolute(Normalized(normalized_zoom)).await?;
        MovementOps::await_idle(self, Duration::from_secs(10)).await?;

        Ok(())
    }
}
