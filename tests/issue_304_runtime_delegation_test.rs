//! Test for issue #304: Verify async Camera delegates to runtime for proper sequence tracking.
//!
//! This test validates that the async Camera implementation:
//! 1. Uses RuntimeHandle for all command sending
//! 2. Properly tracks sequences through the runtime
//! 3. Handles timeout configuration correctly

#![cfg(all(feature = "async", feature = "rt-tokio", feature = "test-utils"))]

use std::time::Duration;

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraBuilder},
    testing::testkit::{helpers, ScriptedTransport, Step},
    timeout::TimeoutConfig,
    Error, InquiryControl, TokioExecutor, ZoomControl,
};

/// Test that async Camera properly delegates to runtime.
///
/// This verifies the core fix for issue #304 - async Camera uses RuntimeHandle
/// instead of direct transport send/recv.
#[tokio::test(start_paused = true)]
async fn test_async_camera_uses_runtime() {
    // Create transport with standard ACK/completion
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::standard_command_response(1), // ACK + Completion for socket 1
    ]);

    // Create camera - this should create a RuntimeHandle internally
    let camera = CameraBuilder::tokio()
        .unwrap()
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Send command - this should go through the runtime
    let result = camera.zoom_stop().await;

    // Should complete successfully
    assert!(result.is_ok(), "Command failed: {:?}", result);

    drop(camera);
}

/// Test that timeout configuration is passed to runtime.
///
/// This verifies that the fix properly threads timeout config through to the runtime.
#[tokio::test(start_paused = true)]
async fn test_timeout_config_passed_to_runtime() {
    // Create transport that will timeout (no response)
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: None,
        responses: vec![], // No response - will timeout
    }]);

    // Create camera with custom timeout
    let custom_timeout = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(100)) // Very short timeout
        .build();

    let camera = CameraBuilder::tokio()
        .unwrap()
        .timeout_config(custom_timeout)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Send command - should timeout quickly
    let start = tokio::time::Instant::now();
    let result = camera.zoom_stop().await;
    let elapsed = start.elapsed();

    // Should fail with timeout
    assert!(result.is_err(), "Command should have failed");
    match result.unwrap_err() {
        Error::Timeout => {
            // Expected - commands timeout according to the configured timeout
        }
        other => panic!("Expected Timeout, got: {:?}", other),
    }

    // Should have timed out quickly (within 200ms plus some margin for runtime)
    assert!(
        elapsed < Duration::from_millis(500),
        "Timeout took too long: {:?}",
        elapsed
    );

    drop(camera);
}

/// Test that command cancellation works through the runtime.
///
/// This verifies that cancel functionality is properly exposed through the async Camera.
#[tokio::test(start_paused = true)]
async fn test_command_cancellation_through_runtime() {
    // Create transport that sends ACK but delays completion
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
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
    ]);

    // Create camera
    let camera = CameraBuilder::tokio()
        .unwrap()
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Start command with ID
    use grafton_visca::command::zoom::Zoom;
    let (cmd_id, future) = camera
        .start_command_with_id(&Zoom::TeleStd)
        .await
        .expect("Failed to start command");

    // Verify we got a valid command ID
    assert!(cmd_id > 0, "Should have valid command ID");

    // Cancel the command
    camera
        .cancel(cmd_id)
        .await
        .expect("Failed to cancel command");

    // Future should resolve with CommandCanceled error
    let result = future.await;
    assert!(
        matches!(result, Err(Error::CommandCanceled)),
        "Command should be canceled, got: {:?}",
        result
    );

    drop(camera);
}

/// Test that inquiries work through the runtime.
///
/// Inquiries are handled differently from commands (no ACK, just direct response).
#[tokio::test(start_paused = true)]
async fn test_inquiry_through_runtime() {
    // Create transport that responds to power inquiry
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: Some(vec![0x81, 0x09, 0x04, 0x00, 0xFF]), // Power inquiry
        responses: vec![vec![0x90, 0x50, 0x02, 0xFF]],     // Power On response (0x02 = On)
    }]);

    // Create camera
    let camera = CameraBuilder::tokio()
        .unwrap()
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Send inquiry
    let result = camera.get_power_state().await;

    // Should get power state
    assert!(result.is_ok(), "Inquiry failed: {:?}", result);
    assert!(
        result.unwrap(),
        "Wrong power state - expected power to be on"
    );

    drop(camera);
}

/// Test that runtime properly handles transport errors.
///
/// This verifies error propagation through the runtime delegation.
#[tokio::test(start_paused = true)]
async fn test_transport_error_propagation() {
    // Create transport that fails immediately
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![Step::InjectError(Error::TransportError(
            "Network failure".into(),
        ))]);

    // Create camera
    let camera = CameraBuilder::tokio()
        .unwrap()
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Send command - should get timeout error (recv failures trigger retries until timeout)
    let result = camera.zoom_stop().await;
    assert!(result.is_err(), "Command should have failed");

    match result.unwrap_err() {
        Error::Timeout => {
            // Expected - recv failures are treated as transient network events
            // that trigger retries until the command eventually times out
        }
        other => panic!("Expected Timeout, got: {:?}", other),
    }

    drop(camera);
}
