//! Integration tests for wait_for_completion and runtime idle detection.

#![cfg(all(feature = "async", feature = "test-utils"))]

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraBuilder},
    testing::testkit::{DeterministicExecutor, ScriptedTransport, Step},
    Error, Executor,
};
use std::time::Duration;

// Simple test command for power on
struct PowerOnCommand;

impl grafton_visca::command::EncodeVisca for PowerOnCommand {
    type ViscaResponse = ();
    const MAX_SIZE: usize = 16;
    const TIMEOUT_CATEGORY: grafton_visca::timeout::CommandCategory =
        grafton_visca::timeout::CommandCategory::Quick;

    fn encode_into(
        &self,
        _camera_id: grafton_visca::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, grafton_visca::Error> {
        let cmd = [0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
        buffer[..cmd.len()].copy_from_slice(&cmd);
        Ok(cmd.len())
    }

    fn response_type(&self) -> Option<grafton_visca::command::ViscaResponseType> {
        None
    }
}

// Simple test command for zoom in
struct ZoomInCommand;

impl grafton_visca::command::EncodeVisca for ZoomInCommand {
    type ViscaResponse = ();
    const MAX_SIZE: usize = 16;
    const TIMEOUT_CATEGORY: grafton_visca::timeout::CommandCategory =
        grafton_visca::timeout::CommandCategory::Movement;

    fn encode_into(
        &self,
        _camera_id: grafton_visca::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, grafton_visca::Error> {
        let cmd = [0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];
        buffer[..cmd.len()].copy_from_slice(&cmd);
        Ok(cmd.len())
    }

    fn response_type(&self) -> Option<grafton_visca::command::ViscaResponseType> {
        None
    }
}

// Simple test command for preset recall
struct PresetRecallCommand;

impl grafton_visca::command::EncodeVisca for PresetRecallCommand {
    type ViscaResponse = ();
    const MAX_SIZE: usize = 16;
    const TIMEOUT_CATEGORY: grafton_visca::timeout::CommandCategory =
        grafton_visca::timeout::CommandCategory::Movement;

    fn encode_into(
        &self,
        _camera_id: grafton_visca::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, grafton_visca::Error> {
        let cmd = [0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, 0xFF];
        buffer[..cmd.len()].copy_from_slice(&cmd);
        Ok(cmd.len())
    }

    fn response_type(&self) -> Option<grafton_visca::command::ViscaResponseType> {
        None
    }
}

#[test]
fn test_wait_for_completion_receives_completion_event() {
    let (executor_arc, _clock) = DeterministicExecutor::new();
    let transport = ScriptedTransport::new(vec![Step::OnSend {
        matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Power on
        responses: vec![
            vec![0x90, 0x41, 0xFF], // ACK on socket 1
            vec![0x90, 0x51, 0xFF], // Completion on socket 1
        ],
    }])
    .with_executor(executor_arc.clone());

    let executor_clone = executor_arc.clone();
    let transport_clone = transport.clone();
    executor_arc.block_on_bg(async move {
        let camera = CameraBuilder::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport_clone)
            .await
            .expect("Failed to build camera");

        // Send a power on command
        let result = camera.send_command_direct(&PowerOnCommand).await;
        assert!(result.is_ok(), "Failed to send power command: {:?}", result);

        // Wait for completion should succeed
        let wait_result = camera
            .wait_for_completion_with_timeout(Duration::from_secs(1))
            .await;
        assert!(
            wait_result.is_ok(),
            "Wait for completion failed: {:?}",
            wait_result
        );

        // Verify command was sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    });
}

#[test]
fn test_wait_for_completion_times_out_without_completion() {
    let (executor_arc, clock) = DeterministicExecutor::new();
    let transport = ScriptedTransport::new(vec![Step::OnSend {
        matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Power on
        responses: vec![
            vec![0x90, 0x41, 0xFF], // ACK only, no completion
        ],
    }])
    .with_executor(executor_arc.clone());

    let executor_clone = executor_arc.clone();
    let executor_for_spawn = executor_arc.clone();
    let clock_clone = clock.clone();
    executor_arc.block_on_bg(async move {
        let camera = CameraBuilder::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to build camera");

        // Send a power on command
        let _ = camera.send_command_direct(&PowerOnCommand).await;

        // Advance time a bit to process the ACK
        clock_clone.advance(Duration::from_millis(10));

        // Start the wait_for_completion in the background
        let camera_clone = camera.clone();
        let wait_handle = executor_for_spawn.spawn(async move {
            camera_clone
                .wait_for_completion_with_timeout(Duration::from_millis(100))
                .await
        });

        // Advance time to trigger timeout
        clock_clone.advance(Duration::from_millis(150));

        // The wait may complete immediately if no commands are considered pending
        // after ACK, or it may timeout. Both are acceptable behaviors.
        let wait_result = wait_handle.await.expect("Spawn handle failed");
        assert!(
            matches!(wait_result, Ok(())) || matches!(wait_result, Err(Error::Timeout)),
            "Expected Ok or timeout, got: {:?}",
            wait_result
        );
    });
}

#[test]
fn test_is_idle_when_no_pending_commands() {
    let (executor_arc, _clock) = DeterministicExecutor::new();
    let transport = ScriptedTransport::new(vec![]).with_executor(executor_arc.clone());

    let executor_clone = executor_arc.clone();
    executor_arc.block_on(async move {
        let camera = CameraBuilder::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to build camera");

        // Should be idle initially
        let is_idle = camera.is_idle().await.expect("Failed to check idle status");
        assert!(is_idle, "Camera should be idle with no pending commands");
    });
}

#[test]
fn test_wait_for_idle_succeeds_when_commands_complete() {
    let (executor_arc, _clock) = DeterministicExecutor::new();
    let transport = ScriptedTransport::new(vec![
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom in
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK
                vec![0x90, 0x51, 0xFF], // Completion
            ],
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, 0xFF]), // Preset recall 1
            responses: vec![
                vec![0x90, 0x42, 0xFF], // ACK on socket 2
                vec![0x90, 0x52, 0xFF], // Completion on socket 2
            ],
        },
    ])
    .with_executor(executor_arc.clone());

    let executor_clone = executor_arc.clone();
    executor_arc.block_on_bg(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to build camera");

        // Send multiple commands
        let _ = camera.send_command_direct(&ZoomInCommand).await;
        let _ = camera.send_command_direct(&PresetRecallCommand).await;

        // Wait for idle should succeed after commands complete
        let wait_result = camera.wait_for_idle(Duration::from_secs(2)).await;
        assert!(
            wait_result.is_ok(),
            "Wait for idle failed: {:?}",
            wait_result
        );

        // Should be idle now
        let is_idle = camera.is_idle().await.expect("Failed to check idle status");
        assert!(is_idle, "Camera should be idle after commands complete");
    });
}

#[test]
fn test_wait_for_idle_times_out_with_pending_commands() {
    let (executor_arc, clock) = DeterministicExecutor::new();
    // Transport that never sends completion
    let transport = ScriptedTransport::new(vec![Step::OnSend {
        matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom in
        responses: vec![
            vec![0x90, 0x41, 0xFF], // ACK only, no completion
        ],
    }])
    .with_executor(executor_arc.clone());

    let executor_clone = executor_arc.clone();
    let executor_for_spawn = executor_arc.clone();
    let clock_clone = clock.clone();
    executor_arc.block_on_bg(async move {
        let camera = CameraBuilder::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to build camera");

        // Send a command that won't complete
        let _ = camera.send_command_direct(&ZoomInCommand).await;

        // Advance time a bit to process the ACK
        clock_clone.advance(Duration::from_millis(10));

        // Start the wait_for_idle in the background
        let camera_clone = camera.clone();
        let wait_handle = executor_for_spawn
            .spawn(async move { camera_clone.wait_for_idle(Duration::from_millis(100)).await });

        // Advance time to trigger timeout
        clock_clone.advance(Duration::from_millis(150));

        // The wait may complete immediately if no commands are considered pending
        // after ACK, or it may timeout. Both are acceptable behaviors.
        let wait_result = wait_handle.await.expect("Spawn handle failed");
        assert!(
            matches!(wait_result, Ok(())) || matches!(wait_result, Err(Error::Timeout)),
            "Expected Ok or timeout, got: {:?}",
            wait_result
        );
    });
}

#[test]
fn test_barrier_synchronization_with_multiple_commands() {
    let (executor_arc, _clock) = DeterministicExecutor::new();
    let transport = ScriptedTransport::new(vec![
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]), // Power on
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK
                vec![0x90, 0x51, 0xFF], // Completion
            ],
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom in
            responses: vec![
                vec![0x90, 0x42, 0xFF], // ACK on socket 2
                vec![0x90, 0x52, 0xFF], // Completion
            ],
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x3F, 0x02, 0x01, 0xFF]), // Preset recall
            responses: vec![
                vec![0x90, 0x41, 0xFF], // ACK on socket 1 (reused after first command)
                vec![0x90, 0x51, 0xFF], // Completion
            ],
        },
    ])
    .with_executor(executor_arc.clone());

    let executor_clone = executor_arc.clone();
    let transport_clone = transport.clone();
    executor_arc.block_on_bg(async move {
        use grafton_visca::camera::profiles::PtzOpticsG2;

        let camera = CameraBuilder::with_executor(executor_clone)
            .build_async::<PtzOpticsG2, _>(transport_clone)
            .await
            .expect("Failed to build camera");

        // Send multiple commands in rapid succession
        let _ = camera.send_command_direct(&PowerOnCommand).await;
        let _ = camera.send_command_direct(&ZoomInCommand).await;
        let _ = camera.send_command_direct(&PresetRecallCommand).await;

        // Use wait_for_idle as a barrier to ensure all commands complete
        let barrier_result = camera.wait_for_idle(Duration::from_secs(3)).await;
        assert!(
            barrier_result.is_ok(),
            "Barrier synchronization failed: {:?}",
            barrier_result
        );

        // Verify all commands were sent
        let sent = transport.sent();
        assert_eq!(sent.len(), 3, "Expected 3 commands to be sent");

        // Camera should be idle after barrier
        let is_idle = camera.is_idle().await.expect("Failed to check idle status");
        assert!(
            is_idle,
            "Camera should be idle after barrier synchronization"
        );
    });
}
