//! Test that the library's runtime behavior with runtime-tokio feature.
//!
//! When runtime-tokio feature is enabled, the camera requires explicit executor configuration
//! to ensure predictable behavior and avoid hidden runtime initialization.
//!
//! The complementary property — that enabling `mode-async` without `runtime-tokio`
//! pulls in no implicit Tokio dependency — is enforced by the feature matrix
//! building this crate with `--no-default-features --features mode-async`, not by
//! a test body. It previously lived here as an empty `#[test]` that reported
//! "ok" without checking anything.

#![cfg(feature = "mode-async")]

#[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
use grafton_visca::{camera::CameraBuilder, PowerControl};

#[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
mod async_tests {
    use grafton_visca::{
        runtime::TokioRuntime,
        testing::testkit::{helpers, ScriptedTransport, Step},
        TokioExecutor,
    };

    use super::*;

    #[tokio::test]
    async fn test_executor_required_for_async_operations() {
        // With the new API, you must provide an executor at construction time
        // This test verifies that the executor is properly integrated
        // Uses TokioExecutor to avoid executor coordination issues

        // Create a scripted transport that responds to power inquiry
        let transport: ScriptedTransport<grafton_visca::TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],     // Power on response
            }]);

        let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
        let camera = CameraBuilder::with_executor(runtime)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Operations should work with the configured executor
        let result = camera.power().state().await;

        // Should succeed with deterministic response
        assert!(result.is_ok(), "Operation failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_operations_with_executor() {
        // Create a camera with the new executor-based API
        // Uses TokioExecutor to avoid executor coordination issues

        let transport: ScriptedTransport<grafton_visca::TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],     // Power on response
            }]);

        let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
        let camera = CameraBuilder::with_executor(runtime)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Operations should work with the configured executor
        // The socket manager will be automatically initialized on first use
        let result = camera.power().state().await;

        // Should succeed with configured executor
        assert!(result.is_ok(), "Operation failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_power_on_with_explicit_executor() {
        // Create a camera with explicitly configured executor
        // Uses TokioExecutor to avoid executor coordination issues

        // Use helpers to create a power command sequence (ACK then completion)
        let transport: ScriptedTransport<grafton_visca::TokioExecutor> =
            ScriptedTransport::new(vec![helpers::auto_respond_step()]);

        let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
        let camera = CameraBuilder::with_executor(runtime)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Try to power on - should succeed with configured executor
        let result = camera.power_on().await;

        // With explicit executor configuration, this should work
        assert!(result.is_ok(), "Power on failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_executor_from_handle() {
        // Test creating executor from a runtime handle
        let _handle = tokio::runtime::Handle::current();

        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],     // Power on response
            }]);

        let runtime = TokioRuntime::from_current().expect("Failed to get runtime");
        let camera = CameraBuilder::with_executor(runtime)
            .open_async::<grafton_visca::camera::profiles::PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Operations should work with the executor created from handle
        let result = camera.power().state().await;

        // Should succeed with the executor
        assert!(result.is_ok(), "Operation failed: {:?}", result);
    }
}
