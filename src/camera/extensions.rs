//! Extension trait system for `Camera<P>` API.
//!
//! This module provides a trait-based extension system that allows users to add
//! custom functionality to cameras without modifying the core library.
//!
//! # Example
//!
//! ```rust,no_run
//! use grafton_visca::{Camera, CameraProfile, Error};
//! use grafton_visca::camera::CameraExtension;
//!
//! // Define your own extension trait
//! trait MyCustomExt<P: CameraProfile>: CameraExtension<P> {
//!     fn my_custom_method(&self) -> Result<(), Error> {
//!         // Your custom implementation
//!         Ok(())
//!     }
//! }
//!
//! // Implement it for all cameras
//! impl<P: CameraProfile, T> MyCustomExt<P> for Camera<P, T> {}
//! ```
//!
//! The extension traits are only available when transport features are enabled:
//! - `async`: Provides async command execution
//! - Default (no features): Provides blocking command execution

use crate::camera::{Camera, CameraProfile};

/// Base trait for camera extensions.
///
/// This trait serves as a marker trait that all camera extensions must implement.
/// It ensures that extensions can only be implemented for types that are `Sized`.
///
/// This trait allows users to extend camera functionality by implementing
/// custom methods on `Camera<P>` instances without modifying the core library.
pub trait CameraExtension<P: CameraProfile>: Sized {}

impl<P: CameraProfile, T> CameraExtension<P> for Camera<P, T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;

    // Test extension trait defined manually
    #[allow(dead_code)]
    trait TestExt<P: CameraProfile>: CameraExtension<P> {
        /// Test method.
        fn test_method(&self, value: u8) -> Result<bool, Error> {
            Ok(value > 0)
        }
    }

    impl<P: CameraProfile> TestExt<P> for Camera<P> {}

    #[test]
    fn test_extension_trait_pattern() {
        // This is mainly a compile-time test
        // The trait should be properly implemented
    }
}
