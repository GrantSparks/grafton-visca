//! Internal trait for unified camera access across wrapper types.

use crate::Camera;

/// Internal trait that provides access to the underlying Camera instance.
///
/// This trait is implemented by all camera wrapper types (root Camera,
/// blocking::Camera, and async::Camera) to provide a consistent way
/// to access the underlying camera functionality.
pub(crate) trait CameraLike {
    /// Get an immutable reference to the underlying Camera.
    fn inner(&self) -> &Camera;
    
    /// Get a mutable reference to the underlying Camera.
    fn inner_mut(&mut self) -> &mut Camera;
}

// Implement for the root Camera type
impl CameraLike for Camera {
    fn inner(&self) -> &Camera {
        self
    }
    
    fn inner_mut(&mut self) -> &mut Camera {
        self
    }
}

// Implement for the blocking wrapper
impl CameraLike for crate::blocking::Camera {
    fn inner(&self) -> &Camera {
        &self.0
    }
    
    fn inner_mut(&mut self) -> &mut Camera {
        &mut self.0
    }
}

// Implement for the async wrapper
impl CameraLike for crate::r#async::Camera {
    fn inner(&self) -> &Camera {
        &self.0
    }
    
    fn inner_mut(&mut self) -> &mut Camera {
        &mut self.0
    }
}