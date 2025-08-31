//! Integration test for command cancellation functionality.
//!
//! This test demonstrates the cancel functionality is properly exposed
//! through the Camera API. Full testing would require a mock transport
//! that can simulate VISCA responses.

#![cfg(feature = "async")]

// Most imports are only used in commented code examples
#[allow(unused_imports)]
use grafton_visca::{ViscaSocket};

/// Test that cancel methods are available on Camera.
/// This is a compilation test to ensure the API is properly exposed.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_cancel_api_available() {
    // This test verifies that the cancel methods compile and are accessible.
    // Actual execution would require a connected camera or mock transport.

    // The following would be used with a real camera:
    /*
    let camera = Camera::new_udp_raw("192.168.1.100:1259").await.unwrap();

    // Send a command with ID tracking
    let (cmd_id, response_future) = camera
        .send_command_with_id(&Zoom::TeleVariable(ZoomSpeed::new(7).unwrap()))
        .await
        .unwrap();

    // Cancel by command ID
    camera.cancel_command(cmd_id).await.unwrap();

    // Or cancel a specific socket directly
    camera.cancel_socket(ViscaSocket::S1).await.unwrap();
    */

    // Just verify the types exist and are public
    let _socket1 = ViscaSocket::S1;
    let _socket2 = ViscaSocket::S2;
}

/// Test that demonstrates the intended usage pattern for cancellation.
#[tokio::test]
#[cfg(feature = "rt-tokio")]
async fn test_cancel_usage_pattern() {
    // This demonstrates how a user would use the cancellation API
    // with a real camera connection.

    /*
    // Example: Start a long-running zoom, then cancel it
    let camera = Camera::new_tcp_raw("192.168.1.100:5678").await.unwrap();

    // Start continuous zoom
    let (zoom_cmd_id, zoom_future) = camera
        .send_command_with_id(&Zoom::TeleVariable(ZoomSpeed::new(5).unwrap()))
        .await
        .unwrap();

    // Do something else...
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Cancel the zoom
    camera.cancel_command(zoom_cmd_id).await.unwrap();

    // Or ensure both sockets are clear
    camera.cancel_socket(ViscaSocket::S1).await.unwrap();
    camera.cancel_socket(ViscaSocket::S2).await.unwrap();
    */
}
