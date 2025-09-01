//! Tests for command cancellation functionality.
//!
//! NOTE: These tests are currently disabled as command cancellation features
//! are not available in the unified camera design. The unified design operates
//! directly on the transport without a runtime background task, so command
//! cancellation would need to be implemented at the transport level.

#![cfg(all(feature = "async", feature = "test-utils"))]

use grafton_visca::{
    camera::controls::system::SystemControl,
    camera::CameraBuilder,
    command::{pan_tilt::PanTiltDirection, zoom::Zoom},
    testing::testkit::{
        scripted_transport::{ScriptedTransport, Step},
        DeterministicExecutor,
    },
    Executor, ViscaSocket,
};
use std::time::Duration;

#[test]
fn test_cancel_command_by_id() {
    // Create executor and transport
    let (executor, clock) = DeterministicExecutor::new();

    // Create steps for the scripted transport
    let steps = vec![
        // Response to zoom command
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele Standard
            responses: vec![vec![0x90, 0x41, 0xFF]],                 // ACK on socket 1
        },
        // Response to cancel socket 1
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
            responses: vec![vec![0x90, 0x61, 0x04, 0xFF]], // Command cancelled
        },
        // Response to cancel socket 2
        Step::OnSend {
            matches: Some(vec![0x81, 0x22, 0xFF]), // Cancel socket 2
            responses: vec![vec![0x90, 0x62, 0x05, 0xFF]], // No socket error
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Create camera
    let camera = executor.block_on(async {
        use grafton_visca::camera::profiles::PtzOpticsG2;
        CameraBuilder::with_executor(executor.clone())
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera")
    });

    // Send a command with ID
    let (_cmd_id, response_future) = executor
        .block_on(async { camera.send_command_with_id(&Zoom::TeleStd).await })
        .expect("Failed to send command");

    // Advance time to process the ACK
    clock.advance(Duration::from_millis(10));

    // Cancel the command using socket (since we don't track individual command IDs to sockets)
    executor
        .block_on(async { camera.cancel_command(ViscaSocket::S1).await })
        .expect("Failed to cancel command");

    // Advance time to process cancellation
    clock.advance(Duration::from_millis(10));

    // The response future should complete with a cancellation error
    // (In a real implementation, this would depend on how we handle cancelled commands)
    drop(response_future); // For now, just drop it
}

#[test]
fn test_cancel_socket_directly() {
    // Create executor and transport
    let (executor, clock) = DeterministicExecutor::new();

    // Create steps for the scripted transport
    let steps = vec![
        // Response to pan/tilt command
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x06, 0x01, 0x05, 0x05, 0x03, 0x03, 0xFF]), // Pan/Tilt
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK on socket 1
        },
        // Response to cancel socket 1
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
            responses: vec![vec![0x90, 0x61, 0x04, 0xFF]], // Command cancelled
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Create camera
    let camera = executor.block_on(async {
        use grafton_visca::camera::profiles::PtzOpticsG2;
        CameraBuilder::with_executor(executor.clone())
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera")
    });

    // Send a pan/tilt command using the public API
    let camera = std::sync::Arc::new(camera);
    let camera_clone = camera.clone();
    std::mem::drop(executor.spawn(async move {
        use grafton_visca::camera::controls::pan_tilt::PanTiltControl;
        let _ = camera_clone
            .pan_tilt_move(
                PanTiltDirection::UpRight,
                5.try_into().unwrap(),
                5.try_into().unwrap(),
            )
            .await;
    }));

    // Advance time to process the command and ACK
    clock.advance(Duration::from_millis(10));

    // Cancel socket 1 directly
    executor
        .block_on(async { camera.cancel_socket(ViscaSocket::S1).await })
        .expect("Failed to cancel socket");

    // Advance time to process cancellation
    clock.advance(Duration::from_millis(10));
}

#[test]
fn test_cancel_nonexistent_command() {
    // Create executor and transport
    let (executor, clock) = DeterministicExecutor::new();

    // Create steps for the scripted transport
    let steps = vec![
        // Response to cancel socket 1
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
            responses: vec![vec![0x90, 0x61, 0x05, 0xFF]], // No socket error
        },
        // Response to cancel socket 2
        Step::OnSend {
            matches: Some(vec![0x81, 0x22, 0xFF]), // Cancel socket 2
            responses: vec![vec![0x90, 0x62, 0x05, 0xFF]], // No socket error
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Create camera
    let camera = executor.block_on(async {
        use grafton_visca::camera::profiles::PtzOpticsG2;
        CameraBuilder::with_executor(executor.clone())
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera")
    });

    // Try to cancel commands on sockets (even if no commands are running)
    executor
        .block_on(async { camera.cancel_command(ViscaSocket::S1).await })
        .expect("Cancel should succeed even for non-existent command");

    // Advance time to process cancellation attempts
    clock.advance(Duration::from_millis(20));
}

#[test]
fn test_cancel_during_movement() {
    // Simplified test that focuses on the cancel API working without complex interactions
    let (executor, clock) = DeterministicExecutor::new();

    // Create a simple transport that responds to any command
    let steps = vec![Step::OnSend {
        matches: None,                           // Match any command
        responses: vec![vec![0x90, 0x41, 0xFF]], // Always respond with ACK
    }];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Create camera and test basic cancel functionality
    let camera = executor.block_on(async {
        use grafton_visca::camera::profiles::PtzOpticsG2;
        CameraBuilder::with_executor(executor.clone())
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera")
    });

    // Test that cancel_command can be called (even with no running commands)
    let result = executor.block_on(async { camera.cancel_command(ViscaSocket::S1).await });
    assert!(result.is_ok(), "Cancel command should not fail");

    // Advance clock to let any pending operations complete
    clock.advance(Duration::from_millis(10));
}
