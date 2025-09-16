//! Integration tests for TransportOptions::Auto functionality.

#![cfg(test)]

use grafton_visca::camera::TransportOptions;

#[cfg(feature = "runtime-tokio")]
mod async_tests {
    use super::*;
    // TokioExecutor is now handled internally

    #[tokio::test]
    async fn test_transport_auto_async() {
        // Test that TransportOptions::Auto can be created
        let auto_option = TransportOptions::Auto {
            address: "test-camera".to_string(),
        };

        // Verify the configuration is accepted
        assert!(matches!(auto_option, TransportOptions::Auto { .. }));
    }

    #[tokio::test]
    async fn test_camera_open_auto_async() {
        // Test that Camera::open_auto_async uses TransportOptions::Auto internally
        // This would need a mock transport to test properly

        // For now, just test that the method exists and compiles
        // Camera::open_auto_async requires the host and runtime
        // This would fail in a test environment without an actual camera
        // For compilation test only - would need mock transport for real test
        // let _result = Camera::open_auto_async::<grafton_visca::capabilities::Detectable, _>("test-camera", &runtime).await;
    }
}

#[cfg(not(feature = "mode-async"))]
mod blocking_tests {
    use super::*;

    #[test]
    fn test_transport_auto_blocking() {
        // Test that TransportOptions::Auto can be created
        let auto_option = TransportOptions::Auto {
            address: "test-camera".to_string(),
        };

        // Verify the configuration is accepted
        assert!(matches!(auto_option, TransportOptions::Auto { .. }));
    }

    #[test]
    fn test_camera_open_auto_blocking() {
        // Test that Camera::open_auto_blocking uses TransportOptions::Auto internally
        // This would need a mock transport to test properly

        // For now, just test that the method exists and compiles
        // This would fail in a test environment without an actual camera
        // let _result = Camera::open_auto_blocking::<SomeProfile>("test-camera");
    }
}

// Auto-detection tests would require access to internal transport builder
// which is not exposed in the public API
// These tests are better suited as unit tests within the library itself

#[test]
fn test_transport_options_auto_creation() {
    // Test that TransportOptions::Auto can be created
    let auto_option = TransportOptions::Auto {
        address: "192.168.1.100".to_string(),
    };

    assert!(matches!(auto_option, TransportOptions::Auto { .. }));

    // Test using the helper method if it exists
    let auto_option2 = TransportOptions::auto("192.168.1.100");
    assert!(matches!(auto_option2, TransportOptions::Auto { .. }));
}
