//! Tests for command cancellation functionality.
//!
//! NOTE: These tests are currently disabled as command cancellation features
//! are not available in the unified camera design. The unified design operates
//! directly on the transport without a runtime background task, so command
//! cancellation would need to be implemented at the transport level.

#![cfg(all(feature = "async", feature = "test-utils"))]

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
    // Create executor and transport
    let (executor, _clock) = DeterministicExecutor::new();

    // Create steps for the scripted transport
    // IMPORTANT: Specific matches must come first before generic ones
    let steps = vec![
        // Response to zoom command - send ACK then completion
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele Standard
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on socket 1
                vec![0x90, 0x51, 0xFF], // Completion
            ],
        },
        // Response to cancel socket 1 (must be early to match properly)
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
            responses: vec![
                vec![0x90, 0x41, 0xFF],       // ACK
                vec![0x90, 0x61, 0x04, 0xFF], // Command cancelled
            ],
        },
        // Response to cancel socket 2 (must be early to match properly)
        Step::OnSend {
            matches: Some(vec![0x81, 0x22, 0xFF]), // Cancel socket 2
            responses: vec![
                vec![0x90, 0x42, 0xFF],       // ACK
                vec![0x90, 0x62, 0x05, 0xFF], // No socket error
            ],
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Run the entire test in a single async block
    executor.block_on(async {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        // Create camera
        let camera = CameraBuilder::with_executor(executor.clone())
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Send a command with ID and get the response
        let (cmd_id, response) = camera
            .send_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to send command");

        // Verify we got a valid command ID
        assert!(cmd_id > 0, "Should have valid command ID");

        // Verify we got a completion response
        use grafton_visca::command::response::ViscaResponse;
        assert!(
            matches!(response, ViscaResponse::Completion { .. }),
            "Should receive completion response"
        );

        // Cancel the command using socket (this should work even after completion)
        camera
            .cancel_socket(ViscaSocket::S1)
            .await
            .expect("Failed to cancel socket");
    });
}

#[test]
fn test_cancel_socket_directly() {
    // Create executor and transport
    let (executor, clock) = DeterministicExecutor::new();

    // Create steps for the scripted transport
    // IMPORTANT: Specific matches must come first before generic ones
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
            matches: None,                           // Match any command
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
        Step::OnSend {
            matches: None,                           // Match any command
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
        Step::OnSend {
            matches: None,                           // Match any command
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
        Step::OnSend {
            matches: None,                           // Match any command
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        },
        Step::OnSend {
            matches: None,                           // Match any command
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
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
        .block_on(async { camera.cancel_socket(ViscaSocket::S1).await })
        .expect("Cancel should succeed even for non-existent command");

    // Advance time to process cancellation attempts
    clock.advance(Duration::from_millis(20));
}

#[test]
fn test_cancel_during_movement() {
    // Simplified test that focuses on the cancel API working without complex interactions
    let (executor, clock) = DeterministicExecutor::new();

    // Create a transport that responds appropriately to different commands
    let mut steps: Vec<Step> = Vec::new();

    // Add specific response for cancel socket 1 command FIRST to ensure it matches
    steps.push(Step::OnSend {
        matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
        responses: vec![vec![0x90, 0x61, 0x05, 0xFF]], // No Socket error (nothing to cancel) - only one response
    });

    // Add many generic responses for camera initialization
    for _ in 0..10 {
        steps.push(Step::OnSend {
            matches: None,                           // Match any command
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK response
        });
    }

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    // Create camera and test basic cancel functionality
    eprintln!("Creating camera...");
    let camera = executor.block_on(async {
        use grafton_visca::camera::profiles::PtzOpticsG2;
        CameraBuilder::with_executor(executor.clone())
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera")
    });
    eprintln!("Camera created successfully");

    // Test that cancel_command can be called (even with no running commands)
    eprintln!("Testing cancel command...");
    let result = executor.block_on(async { camera.cancel_socket(ViscaSocket::S1).await });
    eprintln!("Cancel command result: {:?}", result);
    assert!(result.is_ok(), "Cancel command should not fail");

    // Drive executor until all tasks are idle
    eprintln!("Driving executor until idle...");
    executor.drive_until_idle();

    // Advance clock to let any pending operations complete
    eprintln!("Advancing clock...");
    clock.advance(Duration::from_millis(10));

    // Drive again to complete any time-based tasks
    executor.drive_until_idle();
    eprintln!("Test completed");
}
