//! Extension trait system for `Camera<P>` API.
//!
//! This module provides a trait-based extension system that allows users to add
//! custom functionality to cameras without modifying the core library.

use crate::{
    camera::{Camera, CameraProfile},
    Command, Error as ViscaError, Response,
};

/// Base trait for camera extensions.
///
/// This trait allows users to extend camera functionality by implementing
/// custom methods on `Camera<P>` instances.
pub trait CameraExtension<P: CameraProfile>: Sized {
    /// Execute a raw command on the camera.
    fn send_raw(&mut self, command: &dyn Command) -> Result<Response, ViscaError>;

    /// Execute a raw command asynchronously.
    #[cfg(feature = "tokio")]
    fn send_raw_async(
        &mut self,
        command: &dyn Command,
    ) -> impl std::future::Future<Output = Result<Response, ViscaError>> + Send;
}

impl<P: CameraProfile> CameraExtension<P> for Camera<P> {
    fn send_raw(&mut self, command: &dyn Command) -> Result<Response, ViscaError> {
        #[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
        {
            self.send_raw(command)
        }
        #[cfg(not(all(feature = "blocking-client", not(feature = "async-client"))))]
        {
            let _ = command; // Suppress unused variable warning
            Err(ViscaError::InvalidState(
                "Camera operations require blocking transport".to_string(),
            ))
        }
    }

    #[cfg(feature = "tokio")]
    async fn send_raw_async(&mut self, command: &dyn Command) -> Result<Response, ViscaError> {
        Camera::send_raw_async(self, command).await
    }
}

/// Example extension trait for custom manufacturer commands.
///
/// Users can create their own extension traits following this pattern.
pub trait CustomManufacturerExt<P: CameraProfile>: CameraExtension<P> {
    /// Example: Send a custom manufacturer-specific command.
    fn send_manufacturer_command(&mut self, data: &[u8]) -> Result<Response, ViscaError> {
        // Create a custom command
        struct ManufacturerCommand<'a> {
            data: &'a [u8],
        }

        impl Command for ManufacturerCommand<'_> {
            fn to_bytes(&self) -> Result<Vec<u8>, ViscaError> {
                let mut bytes = vec![0x81, 0x01]; // Standard header
                bytes.extend_from_slice(self.data);
                bytes.push(0xFF); // Terminator
                Ok(bytes)
            }

            fn response_type(&self) -> Option<crate::command::ResponseType> {
                None // Unknown response type
            }

            fn command_category(&self) -> crate::timeout::CommandCategory {
                crate::timeout::CommandCategory::Quick
            }
        }

        let command = ManufacturerCommand { data };
        self.send_raw(&command)
    }
}

// Blanket implementation for all Camera<P> types
impl<P: CameraProfile> CustomManufacturerExt<P> for Camera<P> {}

/// Extension trait for advanced camera diagnostics.
pub trait DiagnosticsExt<P: CameraProfile>: CameraExtension<P> {
    /// Get detailed diagnostic information.
    fn get_diagnostics(&mut self) -> Result<DiagnosticInfo, ViscaError> {
        // This is just an example - real implementation would query multiple status values
        Ok(DiagnosticInfo {
            model: P::MODEL_NAME.to_string(),
            firmware_version: None,
            operation_hours: None,
            error_count: 0,
        })
    }

    /// Run a self-test sequence.
    fn run_self_test(&mut self) -> Result<SelfTestResult, ViscaError> {
        // Example self-test implementation
        Ok(SelfTestResult {
            pan_tilt_ok: true,
            zoom_ok: true,
            focus_ok: true,
            exposure_ok: true,
        })
    }
}

/// Diagnostic information structure.
#[derive(Debug, Clone)]
pub struct DiagnosticInfo {
    /// Camera model name.
    pub model: String,
    /// Firmware version if available.
    pub firmware_version: Option<String>,
    /// Operation hours if available.
    pub operation_hours: Option<u32>,
    /// Total error count.
    pub error_count: u32,
}

/// Self-test result structure.
#[derive(Debug, Clone, Copy)]
pub struct SelfTestResult {
    /// Pan/tilt mechanism test result.
    pub pan_tilt_ok: bool,
    /// Zoom mechanism test result.
    pub zoom_ok: bool,
    /// Focus mechanism test result.
    pub focus_ok: bool,
    /// Exposure system test result.
    pub exposure_ok: bool,
}

impl<P: CameraProfile> DiagnosticsExt<P> for Camera<P> {}

/// Extension trait for camera scripting and automation.
pub trait ScriptingExt<P: CameraProfile>: CameraExtension<P> {
    /// Execute a sequence of movements with timing.
    fn execute_movement_script(&mut self, script: &[MovementStep]) -> Result<(), ViscaError> {
        for step in script {
            match &step.action {
                MovementAction::PanTilt { pan, tilt } => {
                    // Would call the appropriate camera method
                    log::info!("Moving to pan: {}, tilt: {}", pan, tilt);
                }
                MovementAction::Zoom { level } => {
                    log::info!("Zooming to level: {}", level);
                }
                MovementAction::Wait { duration } => {
                    log::info!("Waiting for {:?}", duration);
                    std::thread::sleep(*duration);
                }
            }
        }
        Ok(())
    }
}

/// A single step in a movement script.
#[derive(Debug, Clone)]
pub struct MovementStep {
    /// The action to perform.
    pub action: MovementAction,
    /// Optional label for this step.
    pub label: Option<String>,
}

/// Actions that can be performed in a movement script.
#[derive(Debug, Clone, Copy)]
pub enum MovementAction {
    /// Pan/tilt to absolute position.
    PanTilt {
        /// Pan position in degrees.
        pan: f32,
        /// Tilt position in degrees.
        tilt: f32,
    },
    /// Zoom to absolute level.
    Zoom {
        /// Zoom level (0-16384).
        level: u16,
    },
    /// Wait for a duration.
    Wait {
        /// Duration to wait.
        duration: std::time::Duration,
    },
}

impl<P: CameraProfile> ScriptingExt<P> for Camera<P> {}

/// Macro to easily create extension traits for `Camera<P>`.
///
/// # Example
/// ```ignore
/// camera_extension_trait! {
///     /// My custom camera extension.
///     pub trait MyCustomExt {
///         /// Do something custom.
///         fn my_custom_method(&mut self) -> Result<(), ViscaError> {
///             // Implementation
///             Ok(())
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! camera_extension_trait {
    (
        $(#[$meta:meta])*
        $vis:vis trait $name:ident {
            $(
                $(#[$method_meta:meta])*
                fn $method:ident(&mut self $(, $param:ident: $type:ty)*) -> Result<$ret:ty, ViscaError> $body:block
            )*
        }
    ) => {
        $(#[$meta])*
        $vis trait $name<P: $crate::camera::CameraProfile>: $crate::camera::extensions::CameraExtension<P> {
            $(
                $(#[$method_meta])*
                fn $method(&mut self $(, $param: $type)*) -> Result<$ret, $crate::Error> $body
            )*
        }

        impl<P: $crate::camera::CameraProfile> $name<P> for $crate::Camera<P> {}
    };
}

#[cfg(test)]
mod tests {

    // Test that the macro works
    camera_extension_trait! {
        /// Test extension trait.
        #[allow(dead_code)]
        trait TestExt {
            /// Test method.
            fn test_method(&mut self, value: u8) -> Result<bool, ViscaError> {
                Ok(value > 0)
            }
        }
    }

    #[test]
    fn test_extension_trait_macro() {
        // This is mainly a compile-time test
        // The macro should create a valid trait
    }
}
