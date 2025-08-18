//! Test that the library's runtime behavior with rt-tokio feature.
//!
//! When rt-tokio feature is enabled, the camera requires explicit executor configuration
//! to ensure predictable behavior and avoid hidden runtime initialization.

#![cfg(feature = "async")]

#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
use grafton_visca::{
    camera::{AsyncMode, Camera},
    PowerControl, TokioExecutor,
};

#[cfg(not(feature = "rt-tokio"))]
#[test]
fn test_no_tokio_fallback_without_runtime() {
    // This test verifies that when async features are enabled but rt-tokio is not,
    // and no runtime is configured, we get an error rather than a fallback.

    // Note: This is a compile-time test to ensure no Tokio dependencies are pulled in
    // when rt-tokio feature is not enabled.

    // If this test compiles successfully with `cargo test --features async --no-default-features`,
    // it means we're not depending on Tokio implicitly.
}

#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
mod async_tests {
    use grafton_visca::testing::testkit::{helpers, ScriptedTransport, Step};

    use super::*;

    #[tokio::test]
    async fn test_executor_required_for_async_operations() {
        // With the new API, you must provide an executor at construction time
        // This test verifies that the executor is properly integrated
        // Uses TokioExecutor to avoid executor coordination issues

        let executor = grafton_visca::TokioExecutor::from_handle(tokio::runtime::Handle::current());

        // Create a scripted transport that responds to power inquiry
        let transport: ScriptedTransport<grafton_visca::TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],     // Power on response
            }]);

        let camera: Camera<AsyncMode, grafton_visca::camera::profiles::PtzOpticsG2, _, _> =
            Camera::with_executor(transport, executor)
                .await
                .expect("Failed to create camera");

        // Operations should work with the configured executor
        let result = camera.power_inquiry().await;

        // Should succeed with deterministic response
        assert!(result.is_ok(), "Operation failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_operations_with_executor() {
        // Create a camera with the new executor-based API
        // Uses TokioExecutor to avoid executor coordination issues

        let executor = grafton_visca::TokioExecutor::from_handle(tokio::runtime::Handle::current());

        let transport: ScriptedTransport<grafton_visca::TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],     // Power on response
            }]);

        let camera: Camera<AsyncMode, grafton_visca::camera::profiles::PtzOpticsG2, _, _> =
            Camera::with_executor(transport, executor)
                .await
                .expect("Failed to create camera");

        // Operations should work with the configured executor
        // The socket manager will be automatically initialized on first use
        let result = camera.power_inquiry().await;

        // Should succeed with configured executor
        assert!(result.is_ok(), "Operation failed: {:?}", result);
    }

    #[tokio::test]
    async fn test_power_on_with_explicit_executor() {
        // Create a camera with explicitly configured executor
        // Uses TokioExecutor to avoid executor coordination issues

        let executor = grafton_visca::TokioExecutor::from_handle(tokio::runtime::Handle::current());

        // Use helpers to create a power command sequence (ACK then completion)
        let transport: ScriptedTransport<grafton_visca::TokioExecutor> =
            ScriptedTransport::new(vec![helpers::auto_respond_step()]);

        let camera: Camera<AsyncMode, grafton_visca::camera::profiles::PtzOpticsG2, _, _> =
            Camera::with_executor(transport, executor)
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
        let handle = tokio::runtime::Handle::current();
        let executor = TokioExecutor::from_handle(handle);

        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
                responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],     // Power on response
            }]);

        let camera: Camera<AsyncMode, grafton_visca::camera::profiles::PtzOpticsG2, _, _> =
            Camera::with_executor(transport, executor)
                .await
                .expect("Failed to create camera");

        // Operations should work with the executor created from handle
        let result = camera.power_inquiry().await;

        // Should succeed with the executor
        assert!(result.is_ok(), "Operation failed: {:?}", result);
    }
}
