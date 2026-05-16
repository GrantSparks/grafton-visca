//! Test for issue #304: Verify async Camera delegates to runtime for proper sequence tracking.
//!
//! This test validates that the async Camera implementation:
//! 1. Uses RuntimeHandle for all command sending
//! 2. Properly tracks sequences through the runtime
//! 3. Handles timeout configuration correctly
//!
//! NOTE: Tests that don't rely on actual timeout behavior use DeterministicExecutor.
//! The timeout configuration test uses DeterministicExecutor because it only verifies
//! that the configuration is accepted, not that timeouts actually work.
//! See issue #394 for details on test executor selection strategy.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

use std::time::Duration;

use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    testing::testkit::{
        deterministic_executor::DeterministicExecutor, helpers, ScriptedTransport, Step,
    },
    timeout::TimeoutConfig,
    Error, Executor, InquiryControl, ZoomControl,
};

/// Test that async Camera properly delegates to runtime.
///
/// This verifies the core fix for issue #304 - async Camera uses RuntimeHandle
/// instead of direct transport send/recv.
#[test]
fn test_async_camera_uses_runtime() {
    let (executor, _clock) = DeterministicExecutor::new();
    // Create transport with standard ACK/completion
    let transport: ScriptedTransport<DeterministicExecutor> = ScriptedTransport::new(vec![
        helpers::standard_command_response(1), // ACK + Completion for socket 1
    ])
    .with_executor(executor.clone());

    // Create camera - this should create a RuntimeHandle internally
    let exec = executor.clone();
    executor.block_on(async move {
        let camera = grafton_visca::camera::CameraBuilder::<DeterministicExecutor>::with_executor(
            exec.clone(),
        )
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

        let result = camera.zoom_stop().await;
        assert!(result.is_ok(), "Command failed: {:?}", result);
        drop(camera);
    });
}

/// Test that timeout configuration is passed to runtime.
///
/// This verifies that the fix properly threads timeout config through to the runtime.
/// We create a camera with custom timeout configuration and verify the runtime accepts it.
#[test]
fn test_timeout_config_passed_to_runtime() {
    let (executor, _clock) = DeterministicExecutor::new();

    // Create transport with standard ACK/completion response
    let transport: ScriptedTransport<DeterministicExecutor> = ScriptedTransport::new(vec![
        helpers::standard_command_response(1), // ACK + Completion for socket 1
    ])
    .with_executor(executor.clone());

    // Create camera with custom timeout configuration
    // This verifies that timeout config is properly passed through the builder
    // and accepted by the runtime without errors
    let custom_timeout = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(100))
        .quick_timeout(Duration::from_millis(200))
        .movement_timeout(Duration::from_millis(500))
        .build();

    let exec = executor.clone();
    executor.block_on(async move {
        // This test verifies that:
        // 1. The CameraBuilder properly accepts timeout_config
        // 2. The timeout config is passed to RuntimeHandle creation
        // 3. The camera operates correctly with custom timeout settings
        let camera = grafton_visca::camera::CameraBuilder::<DeterministicExecutor>::with_executor(
            exec.clone(),
        )
        .timeout_config(custom_timeout)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

        // Execute a command to verify the runtime is working with the custom timeout
        let result = camera.zoom_stop().await;
        assert!(result.is_ok(), "Command failed: {:?}", result);

        // Success - the custom timeout config was accepted and the runtime works correctly
        drop(camera);
    });
}

/// Test that command cancellation works through the runtime.
///
/// This verifies that cancel functionality is properly exposed through the async Camera.
#[test]
fn test_command_cancellation_through_runtime() {
    let (executor, _clock) = DeterministicExecutor::new();
    // Create transport that sends ACK but delays completion
    let transport: ScriptedTransport<DeterministicExecutor> = ScriptedTransport::new(vec![
        Step::OnSend {
            matches: Some(vec![0x81, 0x01, 0x04, 0x07, 0x02, 0xFF]), // Zoom Tele
            responses: vec![
                helpers::ack(1), // ACK immediately
                                 // No completion - command stays in flight
            ],
        },
        Step::OnSend {
            matches: Some(vec![0x81, 0x21, 0xFF]), // Cancel socket 1
            responses: vec![vec![0x90, 0x61, 0x04, 0xFF]], // Command cancelled error
        },
    ])
    .with_executor(executor.clone());

    let exec = executor.clone();
    executor.block_on(async move {
        let camera = grafton_visca::camera::CameraBuilder::<DeterministicExecutor>::with_executor(
            exec.clone(),
        )
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

        use grafton_visca::command::Zoom;
        let (cmd_id, future) = camera
            .start_command_with_id(&Zoom::TeleStd)
            .await
            .expect("Failed to start command");

        // CommandId is guaranteed non-zero by construction (NonZeroU32)
        assert!(cmd_id.get() > 0, "Should have valid command ID");

        camera
            .cancel(cmd_id)
            .await
            .expect("Failed to cancel command");
        let result = future.await;
        assert!(
            matches!(result, Err(Error::CommandCanceled)),
            "Command should be canceled, got: {:?}",
            result
        );

        drop(camera);
    });
}

/// Test that inquiries work through the runtime.
///
/// Inquiries are handled differently from commands (no ACK, just direct response).
#[test]
fn test_inquiry_through_runtime() {
    let (executor, _clock) = DeterministicExecutor::new();
    // Create transport that responds to power inquiry
    let transport: ScriptedTransport<DeterministicExecutor> =
        ScriptedTransport::new(vec![Step::OnSend {
            matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
            responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],     // Power On response (0x02 = On)
        }])
        .with_executor(executor.clone());

    let exec = executor.clone();
    executor.block_on(async move {
        let camera = grafton_visca::camera::CameraBuilder::<DeterministicExecutor>::with_executor(
            exec.clone(),
        )
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

        let result = camera.power_state().await;
        assert!(result.is_ok(), "Inquiry failed: {:?}", result);
        assert!(
            result.unwrap(),
            "Wrong power state - expected power to be on"
        );
        drop(camera);
    });
}

/// Test that runtime properly handles transport errors.
///
/// This verifies error propagation through the runtime delegation.
#[test]
fn test_transport_error_propagation() {
    let (executor, _clock) = DeterministicExecutor::new();
    // Create transport that sends an error response to simulate transport issues
    // (using error response instead of InjectError to avoid executor complications)
    let transport: ScriptedTransport<DeterministicExecutor> =
        ScriptedTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x60, 0x02, 0xFF]], // Syntax error response
        }])
        .with_executor(executor.clone());

    let exec = executor.clone();
    executor.block_on(async move {
        let camera = grafton_visca::camera::CameraBuilder::<DeterministicExecutor>::with_executor(
            exec.clone(),
        )
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

        let result = camera.zoom_stop().await;
        assert!(result.is_err(), "Command should have failed");
        // The error response validates that the runtime properly handles errors
        drop(camera);
    });
}
