//! Test for issue #297: ACK race condition fix.
//!
//! This test verifies that commands are pre-registered before sending
//! to eliminate the race condition where an ACK arrives before the
//! command is registered in the pending list.

#![cfg(all(feature = "async", feature = "rt-tokio", feature = "test-utils"))]

use grafton_visca::camera::{profiles::PtzOpticsG2, CameraBuilder};
use grafton_visca::testing::testkit::{helpers, ScriptedTransport, Step};
use grafton_visca::{TokioExecutor, ZoomControl};

/// Test that an ACK delivered synchronously from within send() is properly handled.
///
/// This reproduces the race condition from issue #297 where:
/// 1. Command is sent
/// 2. ACK arrives immediately (before send() returns)
/// 3. Previously: ACK would be dropped because command wasn't registered yet
/// 4. Now: Command is pre-registered, so ACK is properly matched
#[tokio::test(start_paused = true)]
async fn test_ack_race_with_immediate_response() {
    // Create transport that sends ACK immediately from within send()
    // The ACK and completion are sent together immediately upon send
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: None,
        responses: vec![
            helpers::ack(1),      // ACK immediately
            helpers::complete(1), // Followed by completion
        ],
    }]);

    // Create camera with the scripted transport
    let camera = CameraBuilder::tokio()
        .unwrap()
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Send a command - this should trigger the immediate ACK and completion
    // The key test is that the ACK is matched even though it arrives immediately
    let result = camera.zoom_stop().await;

    // The command should complete successfully without timing out or logging warnings
    assert!(result.is_ok(), "Command failed: {:?}", result);

    drop(camera);
}

/// Test that transport failures are properly handled.
///
/// Note: ScriptedTransport's InjectError only works on recv(), not send().
/// Recv failures are treated as transient network events that trigger retries
/// until the command eventually times out.
#[tokio::test(start_paused = true)]
async fn test_rollback_on_send_failure() {
    // Create transport that fails on recv by injecting an error
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![Step::InjectError(
            grafton_visca::Error::TransportError("Network error".into()),
        )]);

    // Create camera with the scripted transport
    let camera = CameraBuilder::tokio()
        .unwrap()
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Send a command - this should fail with timeout after retries
    let result = camera.zoom_stop().await;

    // The command should fail with timeout (recv failures trigger retries)
    assert!(result.is_err(), "Command should have failed");
    let err = result.unwrap_err();
    assert!(
        matches!(err, grafton_visca::Error::Timeout),
        "Expected Timeout, got: {:?}",
        err
    );

    drop(camera);
}

/// Test that the fix maintains correct behavior for normal operation.
#[tokio::test(start_paused = true)]
async fn test_normal_operation_still_works() {
    // Create transport with normal ACK/completion sequence
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::standard_command_response(1), // ACK + Completion for socket 1
    ]);

    // Create camera
    let camera = CameraBuilder::tokio()
        .unwrap()
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Send command
    let result = camera.zoom_stop().await;

    // Should complete successfully
    assert!(result.is_ok(), "Normal operation failed: {:?}", result);

    drop(camera);
}

/// Test multiple commands with immediate ACKs to ensure ordering is preserved.
#[tokio::test(start_paused = true)]
async fn test_multiple_immediate_acks_preserve_order() {
    use grafton_visca::PanTiltControl;

    // Create transport that sends immediate ACKs for each command
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        // First command: immediate ACK and completion
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(1), helpers::complete(1)],
        },
        // Second command: immediate ACK and completion
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(2), helpers::complete(2)],
        },
    ]);

    // Create camera
    let camera = CameraBuilder::tokio()
        .unwrap()
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .unwrap();

    // Send multiple commands - both should handle immediate ACKs correctly
    let result1 = camera.zoom_stop().await;
    let result2 = camera.pan_tilt_stop().await;

    // Both commands should complete successfully
    assert!(result1.is_ok(), "First command failed: {:?}", result1);
    assert!(result2.is_ok(), "Second command failed: {:?}", result2);

    drop(camera);
}
