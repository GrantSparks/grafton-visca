//! Async facade for Camera that provides ergonomic async methods.

use crate::{
    camera::core::CameraCore, capabilities::ProfileMetadata, transport::core::Transport,
};

/// Async camera interface that wraps CameraCore with async methods.
///
/// This type is re-exported as `Camera` and provides the primary
/// async API for camera control.
#[deprecated(
    since = "0.5.0",
    note = "Use `UnifiedCamera` instead, which provides a simpler API without generics"
)]
#[derive(Debug)]
pub struct CameraAsync<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    pub(super) core: CameraCore<P, T>,
}

impl<P, T> CameraAsync<P, T>
where
    P: ProfileMetadata,
    T: Transport,
{
    /// Create a new async camera instance.
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

// Extension traits will add async methods that call the core methods
// and await the futures. For example:
//
// impl<P, T> ZoomMethodsExt for CameraAsync<P, T>
// where
//     P: ProfileMetadata + Zoom,
//     T: Transport,
// {
//     async fn zoom_stop(&self) -> Result<(), Error> {
//         let command = ZoomStopCommand::new();
//         self.core.send_command(&command).await?;
//         Ok(())
//     }
// }
