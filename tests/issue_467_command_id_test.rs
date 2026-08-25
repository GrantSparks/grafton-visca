//! Tests for issue #467: Make command IDs type-safe.
//!
//! This test verifies:
//! 1. `send_command_with_id` and `start_command_with_id` reject inquiries
//! 2. Real commands return valid non-zero `CommandId`s
//! 3. The `CommandId` type provides compile-time safety

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

use grafton_visca::{
    camera::CameraBuilder,
    command::{Zoom, ZoomPositionInquiry},
    testing::testkit::{
        deterministic_executor::DeterministicExecutorExt,
        scripted_transport::{ScriptedTransport, Step},
        DeterministicExecutor,
    },
    Error, Executor,
};

/// Test that `start_command_with_id` returns `Error::InquiryNotCancelable` for inquiry commands.
#[test]
fn test_start_command_with_id_rejects_inquiry() {
    let (executor, _clock) = DeterministicExecutor::new();

    // No steps needed - we expect the error before any I/O
    let transport = ScriptedTransport::new(vec![]).with_executor(executor.clone());

    let executor_clone = executor.clone();
    executor.clone().block_on(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Try to start an inquiry with `start_command_with_id` - should fail
        let result = camera.start_command_with_id(&ZoomPositionInquiry).await;

        match result {
            Err(Error::InquiryNotCancelable) => (),
            Err(e) => panic!("Expected InquiryNotCancelable error, got: {:?}", e),
            Ok(_) => panic!("Expected InquiryNotCancelable error, but got Ok"),
        }
    });
}

/// Test that `send_command_with_id` returns `Error::InquiryNotCancelable` for inquiry commands.
#[test]
fn test_send_command_with_id_rejects_inquiry() {
    let (executor, _clock) = DeterministicExecutor::new();

    // No steps needed - we expect the error before any I/O
    let transport = ScriptedTransport::new(vec![]).with_executor(executor.clone());

    let executor_clone = executor.clone();
    executor.clone().block_on(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Try to send an inquiry with `send_command_with_id` - should fail
        let result = camera.send_command_with_id(&ZoomPositionInquiry).await;

        assert!(
            matches!(result, Err(Error::InquiryNotCancelable)),
            "Expected InquiryNotCancelable error, got: {:?}",
            result
        );
    });
}

/// Test that real commands return valid non-zero `CommandId`s.
#[test]
fn test_command_with_id_returns_valid_command_id() {
    let (executor, _clock) = DeterministicExecutor::new();

    let steps = vec![
        // Response to zoom command - send ACK and completion
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele Standard
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on socket 1
                vec![0x90, 0x51, 0xFF], // Completion
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

        // Send a real command and get its ID
        let (cmd_id, _response) = camera
            .send_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to send command");

        // CommandId is guaranteed non-zero by construction (NonZeroU32)
        assert!(cmd_id.get() > 0, "CommandId should be non-zero");

        // Verify the raw value is usable for logging
        let _logged = format!("Command ID: {}", cmd_id);
    });
}

/// Test that `start_command_with_id` returns a valid `CommandId` for commands.
#[test]
fn test_start_command_with_id_returns_valid_id() {
    let (executor, _clock) = DeterministicExecutor::new();

    let steps = vec![
        // Response to zoom command - send ACK but NOT completion
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele Standard
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on socket 1
            ],
        },
        // Response to cancel by ID
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
        use grafton_visca::camera::profiles::GenericVisca;

        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .open_async::<GenericVisca, _>(transport)
            .await
            .expect("Failed to create camera");

        let (cmd_id, future) = camera
            .start_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to start command");

        // CommandId is guaranteed non-zero
        assert!(cmd_id.get() > 0, "CommandId should be non-zero");

        // Cancel using the type-safe CommandId
        camera.cancel(cmd_id).await.expect("Failed to cancel");

        // The future should resolve with cancelled error
        let result = future.await;
        assert!(
            matches!(result, Err(Error::CommandCanceled)),
            "Command should be canceled, got: {:?}",
            result
        );
    });
}

/// Test that sequential command IDs are unique and non-zero.
#[test]
fn test_sequential_command_ids_are_unique() {
    let (executor, _clock) = DeterministicExecutor::new();

    let steps = vec![
        // First zoom command
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]),
            responses: vec![vec![0x90, 0x41, 0xFF], vec![0x90, 0x51, 0xFF]],
        },
        // Second zoom command
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x03, 0xFF]), // Zoom Wide
            responses: vec![vec![0x90, 0x41, 0xFF], vec![0x90, 0x51, 0xFF]],
        },
    ];

    let transport = ScriptedTransport::new(steps).with_executor(executor.clone());

    let executor_clone = executor.clone();
    // Use block_on_bg to allow virtual time advancement between commands.
    // This is needed because PtzOpticsG2 has MIN_COMMAND_SPACING = 100ms,
    // requiring the deterministic executor to advance time between sends.
    executor.clone().block_on_bg(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(executor_clone)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        let (id1, _) = camera
            .send_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to send first command");

        let (id2, _) = camera
            .send_command_with_id(&Zoom::WideStd)
            .await
            .expect("Failed to send second command");

        // Both IDs should be non-zero
        assert!(id1.get() > 0, "First CommandId should be non-zero");
        assert!(id2.get() > 0, "Second CommandId should be non-zero");

        // IDs should be different (sequential allocation)
        assert_ne!(id1, id2, "Sequential CommandIds should be unique");
    });
}
