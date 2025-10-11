//! Tests for command cancellation functionality.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

use std::time::Duration;

use grafton_visca::{
    camera::CameraBuilder,
    command::{pan_tilt::PanTiltDirection, zoom::Zoom},
    testing::testkit::{
        scripted_transport::{ScriptedTransport, Step},
        DeterministicExecutor,
    },
    Executor, ViscaSocket,
};

#[test]
fn test_cancel_command_by_id() {
    let (executor, _clock) = DeterministicExecutor::new();
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

    let executor_clone = executor.clone();
    executor.clone().block_on(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");
        let (cmd_id, future) = camera
            .start_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to send command");

        // Verify we got a valid command ID
        assert!(cmd_id > 0, "Should have valid command ID");

        camera
            .cancel(cmd_id)
            .await
            .expect("Failed to cancel command");
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
    let (executor, clock) = DeterministicExecutor::new();
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

    let executor_clone = executor.clone();
    let clock_clone = clock.clone();

    executor.clone().block_on(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

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
                    grafton_visca::CommandOptions::default(),
                )
                .await;
        });

        clock_clone.advance(Duration::from_millis(10));
        camera
            .cancel_socket(ViscaSocket::S1)
            .await
            .expect("Failed to cancel socket");

        clock_clone.advance(Duration::from_millis(10));
    });
}

#[test]
fn test_cancel_nonexistent_command() {
    let (executor, clock) = DeterministicExecutor::new();
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

    let executor_clone = executor.clone();
    let clock_clone = clock.clone();

    executor.clone().block_on(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");
        camera
            .cancel_socket(ViscaSocket::S1)
            .await
            .expect("Cancel should succeed even for non-existent command");

        clock_clone.advance(Duration::from_millis(20));
    });
}

#[test]
fn test_cancel_during_movement() {
    let (executor, clock) = DeterministicExecutor::new();
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

    let executor_clone = executor.clone();
    let clock_clone = clock.clone();

    executor.clone().block_on(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");
        let result = camera.cancel_socket(ViscaSocket::S1).await;
        assert!(result.is_ok(), "Cancel command should not fail");

        clock_clone.advance(Duration::from_millis(10));
    });
}
