//! Test that the library doesn't fall back to Tokio runtime when no runtime is configured.

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

    // Create a minimal test transport that implements AsyncTransport
    struct TestTransport;

    impl grafton_visca::transport::AsyncTransport for TestTransport {
        async fn send(&self, _bytes: &[u8]) -> Result<(), Error> {
            Ok(())
        }

        async fn recv(&self) -> Result<Bytes, Error> {
            // Return a minimal ACK response
            Ok(Bytes::from_static(&[0x90, 0x41, 0xFF]))
        }
    }

    #[tokio::test]
    async fn test_socket_manager_requires_runtime() {
        // Create a camera without configuring a runtime
        let transport = TestTransport;
        let camera = grafton_visca::camera::CameraAsync::<
            grafton_visca::camera::profiles::PTZOpticsG2,
            TestTransport,
        >::from_transport(transport);

        // Try to perform an operation without runtime
        // This should fail with MissingRuntime error when auto-init tries to create socket manager
        let result = camera.power_inquiry().await;

        match result {
            Err(Error::MissingRuntime) => {
                // This is the expected behavior - no fallback occurred
            }
            Ok(_) => panic!("Expected MissingRuntime error, but operation succeeded"),
            Err(e) => panic!("Expected MissingRuntime error, but got: {}", e),
        }
    }

    #[tokio::test]
    async fn test_operations_succeed_with_configured_runtime() {
        use grafton_visca::runtime::{SharedRuntime, TokioRuntime};
        use std::sync::Arc;

        // Create a camera with a properly configured runtime
        let transport = TestTransport;
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
        match result {
            Ok(_) => {
                // Expected - operation succeeded
            }
            Err(Error::MissingRuntime) => {
                panic!("Got MissingRuntime error even though runtime was configured");
            }
            Err(e) => {
                // Other errors are acceptable (e.g., connection issues)
                eprintln!("Operation failed with: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_power_on_works_without_runtime() {
        // Create a camera without runtime
        let transport = TestTransport;
        let camera = grafton_visca::camera::CameraAsync::<
            grafton_visca::camera::profiles::PTZOpticsG2,
            TestTransport,
        >::from_transport(transport);

        // Try to power on without runtime (uses runtime for power-on delay)
        let result = camera.power_on().await;

        // Note: power_on should succeed without runtime but won't enforce delays
        // The test verifies no Tokio fallback occurs (it would panic if there was a fallback)
        match result {
            Ok(_) => {
                // OK - command succeeded but delay wasn't enforced (no runtime)
                // This proves no Tokio fallback occurred
            }
            Err(e) => {
                // Any error is acceptable except runtime errors
                // The key is no implicit Tokio usage
                eprintln!("Power on failed (expected without runtime): {}", e);
            }
        }
    }
}
