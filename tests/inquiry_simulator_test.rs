//! Direct test of the VISCA simulator's inquiry response functionality.

#![cfg(feature = "rt-tokio")]

use grafton_visca::{testing::camera_simulator::ViscaCameraSimulator, transport::AsyncTransport};

#[tokio::test]
async fn test_simulator_power_inquiry_direct() {
    let simulator = ViscaCameraSimulator::new();

    // Send power inquiry command directly
    let power_inquiry = vec![0x81, 0x09, 0x04, 0x00, 0xFF];
    simulator
        .send(&power_inquiry)
        .await
        .expect("send should succeed");

    // Receive the response
    let response = simulator.recv().await.expect("should receive response");

    // Verify it's a data reply (0x90 0x50)
    assert_eq!(response[0], 0x90, "Should be response header");
    assert_eq!(response[1], 0x50, "Should be data reply");
    assert_eq!(response[2], 0x02, "Power should be on (0x02)");
    assert_eq!(response[3], 0xFF, "Should have terminator");
}

#[tokio::test]
async fn test_simulator_zoom_inquiry_direct() {
    let simulator = ViscaCameraSimulator::new();

    // Send zoom position inquiry
    let zoom_inquiry = vec![0x81, 0x09, 0x04, 0x47, 0xFF];
    simulator
        .send(&zoom_inquiry)
        .await
        .expect("send should succeed");

    // Receive the response
    let response = simulator.recv().await.expect("should receive response");

    // Verify it's a data reply with zoom position
    assert_eq!(response[0], 0x90, "Should be response header");
    assert_eq!(response[1], 0x50, "Should be data reply");
    assert_eq!(response.len(), 7, "Should be 0x90 0x50 [4 nibbles] 0xFF");

    // Verify position is 0x0000 (all nibbles are 0)
    assert_eq!(response[2], 0x00);
    assert_eq!(response[3], 0x00);
    assert_eq!(response[4], 0x00);
    assert_eq!(response[5], 0x00);
    assert_eq!(response[6], 0xFF);
}

#[tokio::test]
async fn test_simulator_pan_tilt_inquiry_direct() {
    let simulator = ViscaCameraSimulator::new();

    // Send pan/tilt position inquiry
    let pt_inquiry = vec![0x81, 0x09, 0x06, 0x12, 0xFF];
    simulator
        .send(&pt_inquiry)
        .await
        .expect("send should succeed");

    // Receive the response
    let response = simulator.recv().await.expect("should receive response");

    // Verify it's a data reply with pan/tilt positions
    assert_eq!(response[0], 0x90, "Should be response header");
    assert_eq!(response[1], 0x50, "Should be data reply");
    assert_eq!(response.len(), 11, "Should be 0x90 0x50 [8 nibbles] 0xFF");

    // Verify positions are both 0 (center)
    // Pan: nibbles 2-5
    assert_eq!(response[2], 0x00);
    assert_eq!(response[3], 0x00);
    assert_eq!(response[4], 0x00);
    assert_eq!(response[5], 0x00);
    // Tilt: nibbles 6-9
    assert_eq!(response[6], 0x00);
    assert_eq!(response[7], 0x00);
    assert_eq!(response[8], 0x00);
    assert_eq!(response[9], 0x00);
    assert_eq!(response[10], 0xFF);
}

#[tokio::test]
async fn test_simulator_exposure_compensation_inquiry() {
    let simulator = ViscaCameraSimulator::new();

    // Send exposure compensation inquiry command directly
    let inquiry = vec![0x81, 0x09, 0x04, 0x4E, 0xFF];
    simulator.send(&inquiry).await.expect("send should succeed");

    // Receive the response
    let response = simulator.recv().await.expect("should receive response");

    // Verify it's a data reply (0x90 0x50)
    // Exposure compensation response format: 0x90 0x50 0x00 0x00 high_nibble low_nibble 0xFF
    // Default value is 0, which gets adjusted to 7 (0 + 7), so nibbles are 0x00 and 0x07
    assert_eq!(response[0], 0x90, "Should be response header");
    assert_eq!(response[1], 0x50, "Should be data reply");
    assert_eq!(response[2], 0x00, "First padding byte");
    assert_eq!(response[3], 0x00, "Second padding byte");
    assert_eq!(
        response[4], 0x00,
        "High nibble of adjusted value (7 >> 4 = 0)"
    );
    assert_eq!(
        response[5], 0x07,
        "Low nibble of adjusted value (7 & 0x0F = 7)"
    );
    assert_eq!(response[6], 0xFF, "Should have terminator");
}

#[tokio::test]
async fn test_simulator_mixed_commands_and_inquiries() {
    let simulator = ViscaCameraSimulator::new();

    // Send a regular command (zoom in)
    let zoom_in = vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];
    simulator.send(&zoom_in).await.expect("send should succeed");

    // Should receive ACK
    let response = simulator.recv().await.expect("should receive ACK");
    assert_eq!(response[0], 0x90);
    assert_eq!(response[1] & 0xF0, 0x40, "Should be ACK");

    // Now send an inquiry while command is executing
    let power_inquiry = vec![0x81, 0x09, 0x04, 0x00, 0xFF];
    simulator
        .send(&power_inquiry)
        .await
        .expect("send should succeed");

    // Should receive inquiry response immediately (not blocked by command)
    let response = simulator
        .recv()
        .await
        .expect("should receive inquiry response");
    assert_eq!(response[0], 0x90);
    assert_eq!(response[1], 0x50, "Should be data reply");
    assert_eq!(response[2], 0x02, "Power should be on");
}
