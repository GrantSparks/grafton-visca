//! Extension trait system for `Camera<P>` API.
//!
//! This module provides a trait-based extension system that allows users to add
//! custom functionality to cameras without modifying the core library.
//!
//! The extension traits are only available when transport features are enabled:
//! - `blocking-client`: Provides blocking command execution
//! - `async-client`: Provides async command execution

use crate::camera::{Camera, CameraProfile};
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use crate::{Error, Response};

/// Base trait for camera extensions.
///
/// This trait allows users to extend camera functionality by implementing
/// custom methods on `Camera<P>` instances.
///
/// Note: This trait is only useful when transport features are enabled.
/// Without `blocking-client` or `async-client`, cameras have no transport capability.
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub trait CameraExtension<P: CameraProfile>: Sized {}

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
impl<P: CameraProfile> CameraExtension<P> for Camera<P> {}

// Provide a no-op extension trait when no transport features are enabled
#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
/// Base trait for camera extensions.
///
/// This trait serves as a marker trait that all camera extensions must implement.
/// It ensures that extensions can only be implemented for types that are `Sized`.
pub trait CameraExtension<P: CameraProfile>: Sized {}

#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
impl<P: CameraProfile> CameraExtension<P> for Camera<P> {}

/// Example extension trait for custom manufacturer commands.
///
/// Users can create their own extension traits following this pattern.
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub trait CustomManufacturerExt<P: CameraProfile>: CameraExtension<P> {
    /// Example: Send a custom manufacturer-specific command.
    fn send_manufacturer_command(&mut self, data: &[u8]) -> Result<Response, Error>;
}

// Implementation for Camera<P> types
#[cfg(all(feature = "blocking-client", not(feature = "async-client")))]
impl<P: CameraProfile> CustomManufacturerExt<P> for Camera<P> {
    fn send_manufacturer_command(&mut self, data: &[u8]) -> Result<Response, Error> {
        // Create a custom command
        struct ManufacturerCommand<'a> {
            data: &'a [u8],
        }

        impl Command for ManufacturerCommand<'_> {
            fn to_bytes(&self) -> Result<Vec<u8>, Error> {
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

/// Extension trait for advanced camera diagnostics.
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub trait DiagnosticsExt<P: CameraProfile>: CameraExtension<P> {
    /// Get detailed diagnostic information.
    fn get_diagnostics(&mut self) -> Result<DiagnosticInfo, Error> {
        // This is just an example - real implementation would query multiple status values
        Ok(DiagnosticInfo {
            model: P::MODEL_NAME.to_string(),
            firmware_version: None,
            operation_hours: None,
            error_count: 0,
        })
    }

    /// Run a self-test sequence.
    fn run_self_test(&mut self) -> Result<SelfTestResult, Error> {
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

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
impl<P: CameraProfile> DiagnosticsExt<P> for Camera<P> {}

/// Extension trait for camera scripting and automation.
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub trait ScriptingExt<P: CameraProfile>: CameraExtension<P> {
    /// Execute a sequence of movements with timing.
    fn execute_movement_script(&mut self, script: &[MovementStep]) -> Result<(), Error> {
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

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
impl<P: CameraProfile> ScriptingExt<P> for Camera<P> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Camera, Error};

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
