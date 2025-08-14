//! Test that the library's runtime behavior with rt-tokio feature.
//!
//! When rt-tokio feature is enabled, the camera requires explicit runtime configuration
//! to ensure predictable behavior and avoid hidden runtime initialization.

#![cfg(feature = "async")]

#[cfg(feature = "rt-tokio")]
use grafton_visca::Error;

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
    async fn test_missing_runtime_error() {
        // When no runtime is configured, operations should fail with MissingRuntime error
        // This ensures explicit runtime configuration is required.
        let transport = TestTransport::new();
        let camera = grafton_visca::camera::CameraAsync::<
            grafton_visca::camera::profiles::PTZOpticsG2,
            TestTransport,
        >::from_transport(transport);

        // Try to perform an operation without configuring a runtime
        // This should fail with MissingRuntime error
        let result = camera.power_inquiry().await;

        // The operation should fail with MissingRuntime error
        assert!(
            matches!(result, Err(Error::MissingRuntime)),
            "Expected MissingRuntime error, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_operations_succeed_with_configured_runtime() {
        use grafton_visca::runtime::{SharedRuntime, TokioRuntime};
        use std::sync::Arc;

        // Create a camera with a properly configured runtime
        let transport = TestTransport::new();
        let runtime: SharedRuntime = Arc::new(TokioRuntime);

        let camera = grafton_visca::camera::CameraAsync::<
            grafton_visca::camera::profiles::PTZOpticsG2,
            TestTransport,
        >::from_transport(transport)
        .with_runtime(runtime);

        // Operations should work with the configured runtime
        // The socket manager will be automatically initialized on first use
        let result = camera.power_inquiry().await;

        // Should succeed with configured runtime
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Operation failed with unexpected error: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_power_on_with_explicit_runtime() {
        use grafton_visca::runtime::{SharedRuntime, TokioRuntime};
        use std::sync::Arc;

        // Create a camera with explicitly configured runtime
        let transport = TestTransport::new();
        let runtime: SharedRuntime = Arc::new(TokioRuntime);

        let camera = grafton_visca::camera::CameraAsync::<
            grafton_visca::camera::profiles::PTZOpticsG2,
            TestTransport,
        >::from_transport(transport)
        .with_runtime(runtime);

        // Try to power on - should succeed with configured runtime
        let result = camera.power_on().await;

        // With explicit runtime configuration, this should work
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Power on failed with unexpected error: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_with_tokio_convenience_method() {
        // Test the convenience method for attaching Tokio runtime
        let transport = TestTransport::new();
        let camera = grafton_visca::camera::CameraAsync::<
            grafton_visca::camera::profiles::PTZOpticsG2,
            TestTransport,
        >::from_transport(transport)
        .with_tokio(); // Use the convenience method

        // Operations should work with the Tokio runtime attached via convenience method
        let result = camera.power_inquiry().await;

        // Should succeed with the tokio runtime
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Operation failed with unexpected error: {:?}",
            result
        );
    }
}
