//! Blocking facade for Camera that provides synchronous methods.

use crate::{
    camera::core::CameraCore, capabilities::ProfileMetadata, transport::gat_transport::Transport,
};

/// Blocking camera interface that wraps CameraCore with sync methods.
///
/// This provides a synchronous API by using a minimal executor to
/// block on the futures returned by CameraCore.
#[derive(Debug)]
pub struct CameraBlocking<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    core: CameraCore<P, T>,
}

impl<P, T> CameraBlocking<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Create a new blocking camera instance.
    pub fn new(transport: T) -> Self {
        Self {
            core: CameraCore::new(transport),
        }
    }

    /// Get a reference to the core camera.
    pub fn core(&self) -> &CameraCore<P, T> {
        &self.core
    }

    /// Get the camera profile information.
    pub fn profile_info(&self) -> &'static str {
        self.core.profile_info()
    }
}

// Extension traits will add blocking methods that call the core methods
// and block on the futures. For example:
//
// impl<P, T> ZoomMethodsExt for CameraBlocking<P, T>
// where
//     P: ProfileMetadata + Zoom,
//     T: Transport,
// {
//     fn zoom_stop(&self) -> Result<(), Error> {
//         let command = ZoomStopCommand::new();
//         block_on(self.core.send_command(&command))?;
//         Ok(())
//     }
// }

/// Extension trait to convert async Camera to blocking.
pub trait BlockingExt<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Get a blocking view of this camera.
    fn blocking(self) -> CameraBlocking<P, T>;
}

impl<P, T> BlockingExt<P, T> for crate::camera::async_facade::CameraAsync<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    fn blocking(self) -> CameraBlocking<P, T> {
        CameraBlocking { core: self.core }
    }
}
