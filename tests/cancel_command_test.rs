//! Tests for command cancellation functionality.

#![cfg(all(feature = "async", feature = "test-utils"))]

use grafton_visca::{
    camera::CameraBuilder,
    command::{
        pan_tilt::{PanTilt, PanTiltDirection},
        zoom::Zoom,
    },
    runtime::SocketId,
    testing::testkit::{
        scripted_transport::{ScriptedTransport, Step},
        DeterministicExecutor,
    },
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
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK on socket 1
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
    let (cmd_id, response_future) = executor
        .block_on(async { camera.send_command_with_id(&Zoom::TeleStandard).await })
        .expect("Failed to send command");

    // Advance time to process the ACK
    clock.advance(Duration::from_millis(10));

    // Cancel the command
    executor
        .block_on(async { camera.cancel_command(cmd_id).await })
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

    // Send a pan/tilt command
    executor.spawn(async move {
        let _ = camera
            .send_command(&PanTilt::Direction(PanTiltDirection::UpRight, 5, 5, 3, 3))
            .await;
    });

    // Advance time to process the command and ACK
    clock.advance(Duration::from_millis(10));

    // Cancel socket 1 directly
    let camera_clone = camera.clone();
    executor
        .block_on(async {
            camera_clone.cancel_socket(SocketId::Socket1).await
        })
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

    // Try to cancel a command that doesn't exist
    executor
        .block_on(async { camera.cancel_command(999).await })
        .expect("Cancel should succeed even for non-existent command");

    // Advance time to process cancellation attempts
    clock.advance(Duration::from_millis(20));
}

#[test]
fn test_cancel_during_movement() {
    // Create executor and transport
    let (executor, clock) = DeterministicExecutor::new();

    // Create steps for the scripted transport
    let steps = vec![
        // Response to continuous zoom
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x27, 0xFF]), // Zoom Tele Variable speed 7
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK on socket 1
        },
        // Response to cancel command
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
            responses: vec![vec![0x90, 0x61, 0x04, 0xFF]], // Command cancelled
        },
        // Response to zoom stop
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]), // Zoom Stop
            responses: vec![vec![0x90, 0x41, 0xFF], vec![0x90, 0x51, 0xFF]], // ACK then Completion
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

    // Start a continuous zoom
    let (cmd_id, _response_future) = executor
        .block_on(async { 
            use grafton_visca::command::zoom::ZoomSpeed;
            camera.send_command_with_id(&Zoom::TeleVariable(ZoomSpeed::new(7).unwrap())).await 
        })
        .expect("Failed to send zoom command");

    // Advance time to simulate movement
    clock.advance(Duration::from_millis(100));

    // Cancel the zoom
    executor
        .block_on(async { camera.cancel_command(cmd_id).await })
        .expect("Failed to cancel zoom");

    // Send a stop command to ensure camera stopped
    executor
        .block_on(async { camera.send_command(&Zoom::Stop).await })
        .expect("Failed to send stop command");

    // Advance time to process all commands
    clock.advance(Duration::from_millis(50));
}
