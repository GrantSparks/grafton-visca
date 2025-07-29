//! Profile-based ergonomic camera implementation.
//!
//! This demonstrates how to create a camera that only exposes methods
//! for commands supported by the specific camera profile, eliminating
//! dead code warnings.

use std::marker::PhantomData;
use crate::transport::core::UnifiedTransport;
use crate::command::ergonomic_pan_tilt::{PanTilt, PanTiltCommand};
use crate::command::ergonomic_power::{Power, PowerCommand};
use crate::command::ergonomic_zoom::{Zoom, ZoomCommand};
use crate::types::{Degrees, PanSpeed, TiltSpeed, ZoomSpeed, ZoomPercentage};
use crate::error::Error;

/// Trait that defines which commands a camera profile supports.
pub trait CameraProfile {
    /// Whether this profile supports pan/tilt commands.
    const SUPPORTS_PAN_TILT: bool = false;
    
    /// Whether this profile supports zoom commands.
    const SUPPORTS_ZOOM: bool = false;
    
    /// Whether this profile supports power commands.
    const SUPPORTS_POWER: bool = false;
    
    /// Whether this profile supports preset commands.
    const SUPPORTS_PRESETS: bool = false;
    
    /// Whether this profile supports focus commands.
    const SUPPORTS_FOCUS: bool = false;
    
    /// Whether this profile supports exposure commands.
    const SUPPORTS_EXPOSURE: bool = false;
    
    /// Whether this profile supports advanced color commands.
    const SUPPORTS_COLOR: bool = false;

    /// Profile name for identification.
    const NAME: &'static str;
}

/// PTZOptics G2 camera profile - supports basic PTZ operations.
pub struct PTZOpticsG2Profile;

impl CameraProfile for PTZOpticsG2Profile {
    const SUPPORTS_PAN_TILT: bool = true;
    const SUPPORTS_ZOOM: bool = true;
    const SUPPORTS_POWER: bool = true;
    const SUPPORTS_PRESETS: bool = true;
    const SUPPORTS_FOCUS: bool = true;
    const NAME: &'static str = "PTZOptics G2";
}

/// Sony FR7 camera profile - supports advanced features.
pub struct SonyFR7Profile;

impl CameraProfile for SonyFR7Profile {
    const SUPPORTS_PAN_TILT: bool = true;
    const SUPPORTS_ZOOM: bool = true;
    const SUPPORTS_POWER: bool = true;
    const SUPPORTS_PRESETS: bool = true;
    const SUPPORTS_FOCUS: bool = true;
    const SUPPORTS_EXPOSURE: bool = true;
    const SUPPORTS_COLOR: bool = true;
    const NAME: &'static str = "Sony FR7";
}

/// Basic camera profile - only pan/tilt and power.
pub struct BasicCameraProfile;

impl CameraProfile for BasicCameraProfile {
    const SUPPORTS_PAN_TILT: bool = true;
    const SUPPORTS_POWER: bool = true;
    const NAME: &'static str = "Basic PTZ";
}

/// Ergonomic camera that only exposes methods supported by its profile.
pub struct ErgonomicCamera<P: CameraProfile> {
    transport: std::sync::Arc<dyn UnifiedTransport>,
    _profile: PhantomData<P>,
}

impl<P: CameraProfile> ErgonomicCamera<P> {
    /// Create a new camera with the specified profile.
    pub fn new(transport: std::sync::Arc<dyn UnifiedTransport>) -> Self {
        Self {
            transport,
            _profile: PhantomData,
        }
    }

    /// Get the profile name.
    pub fn profile_name(&self) -> &'static str {
        P::NAME
    }

    /// Send a command to the camera.
    async fn send_command<C: crate::command::ViscaCommand>(&self, command: C) -> Result<(), Error> {
        // This would integrate with your existing transport layer
        // For now, just a placeholder implementation
        let _bytes = command.to_bytes();
        let _response_type = command.response_type();
        
        // In real implementation:
        // self.transport.send_command_and_wait(command).await
        Ok(())
    }
}

// Pan/Tilt methods - only available if profile supports pan/tilt
impl<P: CameraProfile> ErgonomicCamera<P>
where
    P: CameraProfile<{P::SUPPORTS_PAN_TILT = true}>, // This syntax doesn't work in current Rust, but shows intent
{
    // For now, we'll use a simpler approach with cfg attributes or trait bounds
}

// Let's use a simpler approach with separate impl blocks for each capability

/// Pan/Tilt operations for cameras that support them.
impl<P: CameraProfile> ErgonomicCamera<P> {
    /// Move camera to home position.
    /// 
    /// Only available for profiles that support pan/tilt.
    pub async fn pan_tilt_home(&self) -> Result<(), Error>
    where
        P: SupportsPanTilt,
    {
        self.send_command(PanTilt::home()).await
    }

    /// Stop all pan/tilt movement.
    /// 
    /// Only available for profiles that support pan/tilt.
    pub async fn pan_tilt_stop(&self) -> Result<(), Error>
    where
        P: SupportsPanTilt,
    {
        self.send_command(PanTilt::stop()).await
    }

    /// Move to absolute pan/tilt position.
    /// 
    /// Only available for profiles that support pan/tilt.
    pub async fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>
    where
        P: SupportsPanTilt,
    {
        let command = PanTilt::absolute(pan, tilt, pan_speed, tilt_speed)?;
        self.send_command(command).await
    }

    /// Move relatively from current position.
    /// 
    /// Only available for profiles that support pan/tilt.
    pub async fn pan_tilt_relative(
        &self,
        pan_delta: Degrees,
        tilt_delta: Degrees,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>
    where
        P: SupportsPanTilt,
    {
        let command = PanTilt::relative(pan_delta, tilt_delta, pan_speed, tilt_speed)?;
        self.send_command(command).await
    }

    /// Reset pan/tilt to default position.
    /// 
    /// Only available for profiles that support pan/tilt.
    pub async fn pan_tilt_reset(&self) -> Result<(), Error>
    where
        P: SupportsPanTilt,
    {
        self.send_command(PanTilt::reset()).await
    }
}

/// Power operations for cameras that support them.
impl<P: CameraProfile> ErgonomicCamera<P> {
    /// Turn camera power on.
    /// 
    /// Only available for profiles that support power control.
    pub async fn power_on(&self) -> Result<(), Error>
    where
        P: SupportsPower,
    {
        self.send_command(Power::on()).await
    }

    /// Turn camera power off (standby).
    /// 
    /// Only available for profiles that support power control.
    pub async fn power_off(&self) -> Result<(), Error>
    where
        P: SupportsPower,
    {
        self.send_command(Power::off()).await
    }

    /// Query current power state.
    /// 
    /// Only available for profiles that support power control.
    pub async fn power_inquiry(&self) -> Result<(), Error>
    where
        P: SupportsPower,
    {
        self.send_command(Power::inquiry()).await
    }
}

/// Zoom operations for cameras that support them.
impl<P: CameraProfile> ErgonomicCamera<P> {
    /// Stop zoom movement.
    /// 
    /// Only available for profiles that support zoom.
    pub async fn zoom_stop(&self) -> Result<(), Error>
    where
        P: SupportsZoom,
    {
        self.send_command(Zoom::stop()).await
    }

    /// Zoom in (telephoto) at specified speed.
    /// 
    /// Only available for profiles that support zoom.
    pub async fn zoom_tele(&self, speed: ZoomSpeed) -> Result<(), Error>
    where
        P: SupportsZoom,
    {
        let command = Zoom::tele(speed)?;
        self.send_command(command).await
    }

    /// Zoom out (wide) at specified speed.
    /// 
    /// Only available for profiles that support zoom.
    pub async fn zoom_wide(&self, speed: ZoomSpeed) -> Result<(), Error>
    where
        P: SupportsZoom,
    {
        let command = Zoom::wide(speed)?;
        self.send_command(command).await
    }

    /// Set absolute zoom position.
    /// 
    /// Only available for profiles that support zoom.
    pub async fn zoom_absolute(&self, position: ZoomPercentage) -> Result<(), Error>
    where
        P: SupportsZoom,
    {
        let command = Zoom::absolute(position)?;
        self.send_command(command).await
    }

    /// Query current zoom position.
    /// 
    /// Only available for profiles that support zoom.
    pub async fn zoom_inquiry(&self) -> Result<(), Error>
    where
        P: SupportsZoom,
    {
        self.send_command(Zoom::inquiry()).await
    }

    /// Enable digital zoom.
    /// 
    /// Only available for profiles that support zoom.
    pub async fn digital_zoom_on(&self) -> Result<(), Error>
    where
        P: SupportsZoom,
    {
        self.send_command(Zoom::digital_zoom_on()).await
    }

    /// Disable digital zoom.
    /// 
    /// Only available for profiles that support zoom.
    pub async fn digital_zoom_off(&self) -> Result<(), Error>
    where
        P: SupportsZoom,
    {
        self.send_command(Zoom::digital_zoom_off()).await
    }
}

// Marker traits for capabilities - these provide compile-time guarantees
pub trait SupportsPanTilt {}
pub trait SupportsPower {}
pub trait SupportsZoom {}
pub trait SupportsPresets {}
pub trait SupportsFocus {}
pub trait SupportsExposure {}
pub trait SupportsColor {}

// Implement marker traits for profiles that support each capability
impl SupportsPanTilt for PTZOpticsG2Profile {}
impl SupportsPower for PTZOpticsG2Profile {}
impl SupportsZoom for PTZOpticsG2Profile {}
impl SupportsPresets for PTZOpticsG2Profile {}
impl SupportsFocus for PTZOpticsG2Profile {}

impl SupportsPanTilt for SonyFR7Profile {}
impl SupportsPower for SonyFR7Profile {}
impl SupportsZoom for SonyFR7Profile {}
impl SupportsPresets for SonyFR7Profile {}
impl SupportsFocus for SonyFR7Profile {}
impl SupportsExposure for SonyFR7Profile {}
impl SupportsColor for SonyFR7Profile {}

impl SupportsPanTilt for BasicCameraProfile {}
impl SupportsPower for BasicCameraProfile {}

// Type aliases for convenience
pub type PTZOpticsG2Camera = ErgonomicCamera<PTZOpticsG2Profile>;
pub type SonyFR7Camera = ErgonomicCamera<SonyFR7Profile>;
pub type BasicCamera = ErgonomicCamera<BasicCameraProfile>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // Mock transport for testing
    struct MockTransport;
    
    #[async_trait::async_trait]
    impl UnifiedTransport for MockTransport {
        async fn send_command_and_wait(
            &self, 
            _command: Box<dyn crate::command::ViscaCommand + Send>
        ) -> Result<crate::command::Response, Error> {
            Ok(crate::command::Response::Ack { socket: 1 })
        }
    }

    #[tokio::test]
    async fn test_ptzoptics_g2_supports_pan_tilt() {
        let transport = Arc::new(MockTransport);
        let camera = PTZOpticsG2Camera::new(transport);
        
        // These should compile and work
        assert!(camera.pan_tilt_home().await.is_ok());
        assert!(camera.power_on().await.is_ok());
        assert!(camera.zoom_stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_basic_camera_limited_functionality() {
        let transport = Arc::new(MockTransport);
        let camera = BasicCamera::new(transport);
        
        // These should compile
        assert!(camera.pan_tilt_home().await.is_ok());
        assert!(camera.power_on().await.is_ok());
        
        // These should NOT compile (zoom not supported):
        // camera.zoom_stop().await; // <- Compilation error!
    }

    #[test]
    fn test_profile_names() {
        assert_eq!(PTZOpticsG2Profile::NAME, "PTZOptics G2");
        assert_eq!(SonyFR7Profile::NAME, "Sony FR7");
        assert_eq!(BasicCameraProfile::NAME, "Basic PTZ");
    }
}