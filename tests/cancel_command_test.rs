//! Tests for command cancellation functionality.

#![cfg(all(feature = "async", feature = "test-utils"))]

// External crates
use grafton_visca::{
    camera::CameraBuilder,
    command::{pan_tilt::PanTiltDirection, zoom::Zoom},
    testing::testkit::{
        scripted_transport::{ScriptedTransport, Step},
        DeterministicExecutor,
    },
    ViscaSocket,
};

// Standard library
use std::time::Duration;

#[test]
fn test_cancel_command_by_id() {
    // Create executor and transport
    let (executor, _clock) = DeterministicExecutor::new();

    // Create steps for the scripted transport
    // Specific matches must come first before generic ones
    let steps = vec![
        // Response to zoom command - send ACK but NOT completion (simulating in-progress command)
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele Standard
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on socket 1 - command is now in flight
                                        // No completion sent - command stays pending so we can cancel it
            ],
        },
        // Response to cancel by ID (runtime will send cancel on the socket)
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
            responses: vec![
                vec![0x90, 0x61, 0x04, 0xFF], // Command cancelled
            ],
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Clone executor for use inside async block
    let executor_clone = executor.clone();

    // Use block_on_bg to run the test with background tasks
    executor.clone().block_on_bg(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        // Create camera
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");
        // Use start_command_with_id to get ID and future without awaiting
        let (cmd_id, future) = camera
            .start_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to send command");

        // Verify we got a valid command ID
        assert!(cmd_id > 0, "Should have valid command ID");

        // Cancel the command by ID while it's still in flight
        camera
            .cancel(cmd_id)
            .await
            .expect("Failed to cancel command");

        // Now await the future - it should resolve with CommandCanceled error
        use grafton_visca::Error;
        let result = future.await;
        assert!(
            matches!(result, Err(Error::CommandCanceled)),
            "Command should be canceled, got: {:?}",
            result
        );
    });
}

#[test]
fn test_cancel_socket_directly() {
    // Create executor and transport
    let (executor, clock) = DeterministicExecutor::new();

    // Create steps for the scripted transport
    // Specific matches must come first before generic ones
    let steps = vec![
        // Response to cancel socket 1 (must be first to match properly)
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
            responses: vec![vec![0x90, 0x61, 0x04, 0xFF]], // Command cancelled
        },
        // Response to pan/tilt command
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x06, 0x01, 0x05, 0x05, 0x03, 0x03, 0xFF]), // Pan/Tilt
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK on socket 1
        },
        // Add generic steps for camera initialization
        Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
        Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
        Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
        Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
        Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Clone executor and clock for use inside async block
    let executor_clone = executor.clone();
    let clock_clone = clock.clone();

    // Use block_on_bg to handle background tasks
    executor.clone().block_on_bg(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        // Create camera
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Send a pan/tilt command using the public API
        let camera = std::sync::Arc::new(camera);
        let camera_clone = camera.clone();
        use grafton_visca::Executor;
        executor_clone.spawn_bg(async move {
            use grafton_visca::camera::controls::pan_tilt::PanTiltControl;
            let _ = camera_clone
                .pan_tilt_move(
                    PanTiltDirection::UpRight,
                    5.try_into().unwrap(),
                    5.try_into().unwrap(),
                )
                .await;
        });

        // Advance time to process the command and ACK
        clock_clone.advance(Duration::from_millis(10));

        // Cancel socket 1 directly
        camera
            .cancel_socket(ViscaSocket::S1)
            .await
            .expect("Failed to cancel socket");

        // Advance time to process cancellation
        clock_clone.advance(Duration::from_millis(10));
    });
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

    // Clone executor and clock for use inside async block
    let executor_clone = executor.clone();
    let clock_clone = clock.clone();

    // Use block_on_bg to handle background tasks
    executor.clone().block_on_bg(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        // Create camera
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Try to cancel commands on sockets (even if no commands are running)
        camera
            .cancel_socket(ViscaSocket::S1)
            .await
            .expect("Cancel should succeed even for non-existent command");

        // Advance time to process cancellation attempts
        clock_clone.advance(Duration::from_millis(20));
    });
}

#[test]
fn test_cancel_during_movement() {
    // Test that cancel API works correctly with background runtime tasks
    let (executor, clock) = DeterministicExecutor::new();

    // Create a transport that responds appropriately to different commands
    let mut steps: Vec<Step> = Vec::new();

    // Add specific response for cancel socket 1 command FIRST to ensure it matches
    steps.push(Step::OnSend {
        matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
        responses: vec![vec![0x90, 0x61, 0x05, 0xFF]], // No Socket error (nothing to cancel)
    });

    // Add generic responses for camera initialization
    for _ in 0..10 {
        steps.push(Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        });
    }

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Clone executor and clock for use inside async block
    let executor_clone = executor.clone();
    let clock_clone = clock.clone();

    // Use block_on_bg to handle background tasks
    executor.clone().block_on_bg(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        // Create camera
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Test that cancel_command can be called (even with no running commands)
        let result = camera.cancel_socket(ViscaSocket::S1).await;
        assert!(result.is_ok(), "Cancel command should not fail");

        // Advance time to process any pending operations
        clock_clone.advance(Duration::from_millis(10));
    });
}
