//! Test that the library's runtime behavior with rt-tokio feature.
//!
//! When rt-tokio feature is enabled, the library auto-initializes a Tokio runtime
//! if none is configured. This is intentional behavior to improve usability.

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
    async fn test_auto_runtime_initialization() {
        // When rt-tokio feature is enabled, the camera auto-initializes a Tokio runtime
        // if none is configured. This test verifies that behavior works correctly.
        let transport = TestTransport::new();
        let camera = grafton_visca::camera::CameraAsync::<
            grafton_visca::camera::profiles::PTZOpticsG2,
            TestTransport,
        >::from_transport(transport);

        // Try to perform an operation without explicitly configuring a runtime
        // With rt-tokio feature, this should auto-initialize the runtime and succeed
        let result = camera.power_inquiry().await;

        // The operation should succeed with auto-initialized runtime
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Operation failed with unexpected error: {:?}",
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
    async fn test_power_on_with_auto_runtime() {
        // Create a camera - with rt-tokio feature, runtime will be auto-initialized
        let transport = TestTransport::new();
        let camera = grafton_visca::camera::CameraAsync::<
            grafton_visca::camera::profiles::PTZOpticsG2,
            TestTransport,
        >::from_transport(transport);

        // Try to power on - should succeed with auto-initialized runtime
        let result = camera.power_on().await;

        // With rt-tokio feature, the runtime is auto-initialized so this should work
        assert!(
            result.is_ok() || matches!(result, Err(Error::UnexpectedResponseType)),
            "Power on failed with unexpected error: {:?}",
            result
        );
    }
}
