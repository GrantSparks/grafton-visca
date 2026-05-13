//! Tests for issue #371: Using per-command CameraId for CANCEL commands.
//!
//! This test verifies that CANCEL commands use the correct camera ID from the command
//! being cancelled, not a hard-coded CAMERA_1.

//! NOTE: These tests are disabled when real runtimes are available because
//! DeterministicExecutor has issues with timeout handling when real runtimes
//! are present. See issue #394 for details.
#![cfg(all(
    feature = "mode-async",
    feature = "test-utils",
    not(feature = "runtime-tokio"),
    not(feature = "runtime-smol")
))]

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraBuilder},
    command::Zoom,
    testing::testkit::{
        scripted_transport::{ScriptedTransport, Step},
        DeterministicExecutor,
    },
    CameraId, Error, Executor, ViscaSocket,
};

/// Test that cancel commands use the correct camera ID for the command being cancelled
#[test]
fn test_cancel_uses_correct_camera_id() {
    // Create executor and transport
    let (executor, _clock) = DeterministicExecutor::new();

    // Camera ID 4 test - verify that cancel uses 0x84, not 0x81
    let steps = vec![
        // Zoom command with camera ID 4
        Step::OnSend {
            matches: Some(vec![0x84, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele with camera 4
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on socket 1
            ],
        },
        // Cancel should use camera ID 4 (0x84), not CAMERA_1 (0x81)
        Step::OnSend {
            matches: Some(vec![0x84, 0x21, 0xFF]), // Cancel socket 1 with camera 4
            responses: vec![
                vec![0x90, 0x61, 0x04, 0xFF], // Command cancelled
            ],
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor_clone = executor.clone();

    executor.clone().block_on(async move {
        // Build camera with custom camera ID 4
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
            .camera_id(CameraId::new(4).unwrap())
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Start a zoom command
        let (cmd_id, future) = camera
            .start_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to send command");

        // Cancel the command - this should use camera ID 4
        camera.cancel(cmd_id).await.expect("Failed to cancel");

        // The command should be cancelled
        let result = future.await;
        assert!(
            matches!(result, Err(Error::CommandCanceled)),
            "Command should be canceled, got: {:?}",
            result
        );
    });
}

/// Test multiple cameras with different IDs to ensure each uses its own ID for cancel
#[test]
fn test_multiple_cameras_cancel_with_own_ids() {
    // Create executor and transport
    let (executor, _clock) = DeterministicExecutor::new();

    let steps = vec![
        // First camera (ID 2) zoom command
        Step::OnSend {
            matches: Some(vec![0x82, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele with camera 2
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on socket 1
            ],
        },
        // Cancel for camera 2 should use 0x82
        Step::OnSend {
            matches: Some(vec![0x82, 0x21, 0xFF]), // Cancel socket 1 with camera 2
            responses: vec![
                vec![0x90, 0x61, 0x04, 0xFF], // Command cancelled
            ],
        },
        // Second camera (ID 7) zoom command
        Step::OnSend {
            matches: Some(vec![0x87, 0x01, 0x04, 0x07, 0x03, 0xFF]), // Zoom Wide with camera 7
            responses: vec![
                vec![0x90, 0x42, 0xFF], // ACK on socket 2
            ],
        },
        // Cancel for camera 7 should use 0x87
        Step::OnSend {
            matches: Some(vec![0x87, 0x22, 0xFF]), // Cancel socket 2 with camera 7
            responses: vec![
                vec![0x90, 0x62, 0x04, 0xFF], // Command cancelled
            ],
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor_clone = executor.clone();

    executor.clone().block_on(async move {
        // Create first camera with ID 2
        let camera1 = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
            .camera_id(CameraId::new(2).unwrap())
            .open_async::<PtzOpticsG2, _>(transport.clone())
            .await
            .expect("Failed to create camera 1");

        // Create second camera with ID 7
        let camera2 = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
            .camera_id(CameraId::new(7).unwrap())
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera 2");

        // Start commands on both cameras
        let (cmd_id1, future1) = camera1
            .start_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to send command on camera 1");

        let (cmd_id2, future2) = camera2
            .start_command_with_id(&Zoom::WideStd)
            .await
            .expect("Failed to send command on camera 2");

        // Cancel both commands - each should use its own camera ID
        camera1
            .cancel(cmd_id1)
            .await
            .expect("Failed to cancel camera 1");
        camera2
            .cancel(cmd_id2)
            .await
            .expect("Failed to cancel camera 2");

        // Both commands should be cancelled
        let result1 = future1.await;
        let result2 = future2.await;

        assert!(
            matches!(result1, Err(Error::CommandCanceled)),
            "Camera 1 command should be canceled, got: {:?}",
            result1
        );
        assert!(
            matches!(result2, Err(Error::CommandCanceled)),
            "Camera 2 command should be canceled, got: {:?}",
            result2
        );
    });
}

/// Test cancel by socket uses correct camera ID when command is on socket
///
/// Note: cancel_socket is only meaningful for sockets that have active command context.
/// Use cancel() with the command ID when canceling a specific operation.
#[test]
fn test_cancel_socket_uses_correct_camera_id() {
    // Create executor and transport
    let (executor, _clock) = DeterministicExecutor::new();

    let steps = vec![
        // Zoom command with camera ID 5
        Step::OnSend {
            matches: Some(vec![0x85, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom with camera 5
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on socket 1
            ],
        },
        // Process the ACK to assign command to socket, then cancel
        // The cancel will use camera ID 5 from the command on the socket
        Step::OnSend {
            matches: Some(vec![0x85, 0x21, 0xFF]), // Cancel socket 1 with camera 5
            responses: vec![
                vec![0x90, 0x61, 0x04, 0xFF], // Command cancelled
            ],
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());
    let executor_clone = executor.clone();

    executor.clone().block_on(async move {
        // Build camera with camera ID 5
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone.clone())
            .camera_id(CameraId::new(5).unwrap())
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Start a command
        let (_cmd_id, future) = camera
            .start_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to send command");

        // Wait briefly for the ACK to be processed and command assigned to socket
        // This simulates real-world timing where you wouldn't immediately cancel
        executor_clone
            .sleep(std::time::Duration::from_millis(1))
            .await;

        // Cancel by socket - should use camera ID 5 from the command on the socket
        camera
            .cancel_socket(ViscaSocket::S1)
            .await
            .expect("Failed to cancel socket");

        // Command should be cancelled
        let result = future.await;
        assert!(
            matches!(result, Err(Error::CommandCanceled)),
            "Command should be canceled, got: {:?}",
            result
        );
    });
}
