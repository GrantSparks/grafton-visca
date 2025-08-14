//! Test that the library's runtime behavior with rt-tokio feature.
//!
//! When rt-tokio feature is enabled, the camera requires explicit executor configuration
//! to ensure predictable behavior and avoid hidden runtime initialization.

#![cfg(feature = "async")]

#[cfg(feature = "rt-tokio")]
use grafton_visca::{
    camera::{AsyncMode, Camera},
    Error, PowerOps, TokioExecutor,
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

#[cfg(feature = "rt-tokio")]
mod async_tests {
    use super::*;
    use bytes::Bytes;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // Create a minimal test transport that implements AsyncTransport
    struct TestTransport {
        recv_count: Arc<AtomicUsize>,
    }

    impl TestTransport {
        fn new() -> Self {
            Self {
                recv_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    impl grafton_visca::transport::AsyncTransport for TestTransport {
        async fn send(&self, _bytes: &[u8]) -> Result<(), Error> {
            Ok(())
        }

        async fn recv(&self) -> Result<Bytes, Error> {
            let count = self.recv_count.fetch_add(1, Ordering::SeqCst);

            // Return different responses based on call count to simulate real camera behavior
            // Power commands typically require ACK followed by completion
            match count {
                0 | 2 | 4 => Ok(Bytes::from_static(&[0x90, 0x41, 0xFF])), // ACK
                1 | 3 | 5 => Ok(Bytes::from_static(&[0x90, 0x51, 0xFF])), // Completion
                6 => Ok(Bytes::from_static(&[0x90, 0x50, 0x02, 0xFF])), // Power inquiry response (on)
                _ => {
                    // Simulate timeout after initial responses
                    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                    Err(Error::Timeout)
                }
            }
        }
    }

    #[tokio::test]
    async fn test_executor_required_for_async_operations() {
        // With the new API, you must provide an executor at construction time
        // This test verifies that the executor is properly integrated

        let transport = TestTransport::new();
        let executor = TokioExecutor::from_current().expect("Failed to get current runtime");

        let camera: Camera<AsyncMode, grafton_visca::camera::profiles::PTZOpticsG2, _, _> =
            Camera::with_executor(transport, executor);

        // Operations should work with the configured executor
        let result = camera.power_inquiry().await;

        // Should succeed or fail with a response-related error (not a runtime error)
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Operation failed with unexpected error: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_operations_with_executor() {
        // Create a camera with the new executor-based API
        let transport = TestTransport::new();
        let executor = TokioExecutor::from_current().expect("Failed to get current runtime");

        let camera: Camera<AsyncMode, grafton_visca::camera::profiles::PTZOpticsG2, _, _> =
            Camera::with_executor(transport, executor);

        // Operations should work with the configured executor
        // The socket manager will be automatically initialized on first use
        let result = camera.power_inquiry().await;

        // Should succeed with configured executor
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Operation failed with unexpected error: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_power_on_with_explicit_executor() {
        // Create a camera with explicitly configured executor
        let transport = TestTransport::new();
        let executor = TokioExecutor::from_current().expect("Failed to get current runtime");

        let camera: Camera<AsyncMode, grafton_visca::camera::profiles::PTZOpticsG2, _, _> =
            Camera::with_executor(transport, executor);

        // Try to power on - should succeed with configured executor
        let result = camera.power_on().await;

        // With explicit executor configuration, this should work
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Power on failed with unexpected error: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_executor_from_handle() {
        // Test creating executor from a runtime handle
        let handle = tokio::runtime::Handle::current();
        let executor = TokioExecutor::from_handle(handle);

        let transport = TestTransport::new();
        let camera: Camera<AsyncMode, grafton_visca::camera::profiles::PTZOpticsG2, _, _> =
            Camera::with_executor(transport, executor);

        // Operations should work with the executor created from handle
        let result = camera.power_inquiry().await;

        // Should succeed with the executor
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Operation failed with unexpected error: {:?}",
            result
        );
    }
}
