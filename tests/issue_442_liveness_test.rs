//! Regression tests for issue #442: Runtime loop liveness fix
//!
//! These tests verify that the runtime loop properly wakes on all control
//! channels (submit_rx, metrics_rx, completions_rx) even when next_deadline()
//! returns a far-future deadline.
//!
//! The fix replaced `try_recv()` polling with `recv_async()` in a unified
//! `future::race()` across all control channels, ensuring new submissions
//! and API calls like `metrics()` wake the loop immediately.
//!
//! Test scenarios from the issue's Testing Plan:
//! 1. Submission wakes loop during long deadline sleep
//! 2. metrics() and subscribe_completions() do not hang during long waits
//! 3. Housekeeping invariants under load
//! 4. Timeout ordering preserved (behavioral equivalence)

#![cfg(all(
    feature = "mode-async",
    feature = "runtime-tokio",
    feature = "test-utils"
))]

use std::sync::Arc;
use std::time::{Duration, Instant};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraBuilder},
    runtime::RuntimeHandle,
    testing::testkit::{helpers, ScriptedTransport, Step},
    timeout::TimeoutConfig,
    Error, PanTiltControl, TokioExecutor, ZoomControl,
};

/// Test 1: Submission wakes loop during long deadline sleep
///
/// This test verifies that submitting a new command while another is pending
/// with a 30-second movement timeout doesn't wait 30 seconds.
///
/// Scenario:
/// 1. Spawn first command that gets ACK but no completion (creating ~30s pending deadline)
/// 2. While first command is pending, submit a second command
/// 3. Assert the second command completes quickly (not waiting for first's 30s timeout)
///
/// If this test takes > 5 seconds, the liveness fix may have regressed.
#[tokio::test]
async fn test_submission_wakes_loop_during_long_deadline_sleep() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());

    // Create transport that:
    // - First command: ACK only (no completion) - creates 30s pending deadline
    // - Second command: ACK + Completion (normal success)
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        // First command: only ACK, no completion - simulates long-running movement
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(1)], // ACK only, no completion
        },
        // Second command: normal ACK + completion on socket 2
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(2), helpers::complete(2)],
        },
    ])
    .with_executor(executor.clone());

    // Configure with 30s movement timeout
    let timeout_config = TimeoutConfig::builder()
        .movement_timeout(Duration::from_secs(30))
        .build();

    // Use RuntimeHandle directly since it implements Clone
    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new_with_timeout(transport, executor.clone(), timeout_config)
            .await
            .expect("Failed to create runtime");

    // Clone for background task
    let runtime_clone = runtime.clone();

    // Spawn first command in background - it will get ACK but wait for completion
    // that never comes (until timeout after 30s)
    let first_handle = tokio::spawn(async move {
        runtime_clone
            .send_command(
                &grafton_visca::command::zoom::Zoom::TeleStd,
                grafton_visca::CameraId::CAMERA_1,
                None,
            )
            .await
    });

    // Give the runtime time to send the command and receive ACK
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now record start time and send second command
    // The loop should be "sleeping" until the 30s movement deadline,
    // but the new submission should wake it immediately
    let start = Instant::now();

    // Send second command while first is pending with 30s deadline
    // This MUST wake the loop immediately if the fix is in place
    // Use Zoom::Stop as the second command (simpler than constructing PanTilt::Move)
    let result = runtime
        .send_command(
            &grafton_visca::command::zoom::Zoom::Stop,
            grafton_visca::CameraId::CAMERA_1,
            None,
        )
        .await;

    let elapsed = start.elapsed();

    // Verify second command completed
    assert!(
        result.is_ok(),
        "Second command should have completed: {:?}",
        result
    );

    // Critical assertion: If elapsed time is > 5 seconds, the liveness fix may have regressed
    // The loop should wake immediately on new submission, not wait for the 30s deadline
    assert!(
        elapsed < Duration::from_secs(5),
        "Command submission took {}s - liveness fix may have regressed! \
         Expected < 5s (loop should wake immediately), got {}s. \
         If the fix were reverted, this would take ~30s.",
        elapsed.as_secs_f64(),
        elapsed.as_secs_f64()
    );

    // More strict check: should complete within 1 second under normal conditions
    assert!(
        elapsed < Duration::from_secs(1),
        "Command submission took {}s - expected < 1s for immediate wake",
        elapsed.as_secs_f64()
    );

    // Clean up - abort the pending first command
    first_handle.abort();
    drop(runtime);
}

/// Test 2: metrics() and subscribe_completions() do not hang during long waits
///
/// This test verifies that API calls to metrics() and subscribe_completions()
/// return promptly even when the loop has a far-future deadline pending.
///
/// Scenario:
/// 1. Create a RuntimeHandle with a pending command (ACK received, no completion)
/// 2. Call metrics() and subscribe_completions() with a timeout
/// 3. Assert both complete within the timeout (not waiting for 30s deadline)
#[tokio::test]
async fn test_metrics_and_completions_do_not_hang_during_long_waits() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());

    // Create transport that sends ACK but never completion
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: None,
        responses: vec![helpers::ack(1)], // ACK only, no completion
    }])
    .with_executor(executor.clone());

    // Configure with long movement timeout to create far-future deadline
    let timeout_config = TimeoutConfig::builder()
        .movement_timeout(Duration::from_secs(30))
        .build();

    // Use RuntimeHandle directly to access metrics() and subscribe_completions()
    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new_with_timeout(transport, executor.clone(), timeout_config)
            .await
            .expect("Failed to create runtime");

    // Clone for the background task
    let runtime_clone = runtime.clone();

    // Spawn a command that will get ACK but no completion (creates ~30s pending deadline)
    let cmd_handle = tokio::spawn(async move {
        runtime_clone
            .send_command(
                &grafton_visca::command::zoom::Zoom::TeleStd,
                grafton_visca::CameraId::CAMERA_1,
                None,
            )
            .await
    });

    // Give the runtime time to send the command and receive ACK
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now test that metrics() returns promptly
    let metrics_start = Instant::now();
    let metrics_result = tokio::time::timeout(Duration::from_secs(1), runtime.metrics()).await;

    let metrics_elapsed = metrics_start.elapsed();

    // metrics() should complete within the timeout
    assert!(
        metrics_result.is_ok(),
        "metrics() timed out after 1 second - liveness fix may have regressed! \
         Expected prompt return, but loop may be sleeping until deadline."
    );

    // Verify we got a valid MetricsSummary
    let metrics = metrics_result
        .unwrap()
        .expect("metrics() returned an error");
    assert!(
        metrics_elapsed < Duration::from_millis(500),
        "metrics() took {}ms - expected < 500ms for immediate response",
        metrics_elapsed.as_millis()
    );

    // Test subscribe_completions() similarly
    let completions_start = Instant::now();
    let completions_result =
        tokio::time::timeout(Duration::from_secs(1), runtime.subscribe_completions()).await;

    let completions_elapsed = completions_start.elapsed();

    // subscribe_completions() should complete within the timeout
    assert!(
        completions_result.is_ok(),
        "subscribe_completions() timed out after 1 second - liveness fix may have regressed!"
    );

    // Verify we got a valid receiver
    let _completion_rx = completions_result
        .unwrap()
        .expect("subscribe_completions() returned an error");

    assert!(
        completions_elapsed < Duration::from_millis(500),
        "subscribe_completions() took {}ms - expected < 500ms for immediate response",
        completions_elapsed.as_millis()
    );

    // Verify metrics shows the pending command was sent
    assert!(
        metrics.commands_sent >= 1,
        "Expected at least 1 sent command, got {}",
        metrics.commands_sent
    );

    // Clean up
    cmd_handle.abort();
    drop(runtime);
}

/// Test 3: Housekeeping invariants under load
///
/// This test verifies that timeout/retry housekeeping runs after all events,
/// not just transport events. The fix removed early `continue` statements that
/// could bypass housekeeping.
///
/// Scenario:
/// 1. Configure short ACK timeout (100ms)
/// 2. Send multiple commands with different response scenarios
/// 3. Verify all commands complete or fail appropriately
/// 4. Verify timeouts occur and retries are processed
#[tokio::test]
async fn test_housekeeping_invariants_under_load() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());

    // Create transport with mixed response scenarios:
    // Command 1: Normal success (ACK + Completion)
    // Command 2: No response (will timeout)
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        // Command 1: Normal ACK + Completion
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(1), helpers::complete(1)],
        },
        // Command 2: No response - will trigger timeout
        Step::OnSend {
            matches: None,
            responses: vec![], // No response
        },
    ])
    .with_executor(executor.clone());

    // Configure short timeouts for faster test execution
    let timeout_config = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(100))
        .movement_timeout(Duration::from_millis(200))
        .build();

    let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
        .timeout_config(timeout_config)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    // Command 1: Should succeed
    let result1 = camera.zoom_stop().await;
    assert!(
        result1.is_ok(),
        "Command 1 should have succeeded: {:?}",
        result1
    );

    // Command 2: Should timeout (no response)
    let result2 = camera.pan_tilt_stop().await;
    assert!(
        matches!(result2, Err(Error::Timeout)),
        "Command 2 should have timed out, got: {:?}",
        result2
    );

    // The fact that command 2 timed out correctly proves that:
    // 1. check_timeouts() is being called (housekeeping runs)
    // 2. Timeouts are processed after all event types
    // If housekeeping were bypassed, the timeout might not fire properly

    drop(camera);
}

/// Test 4: Timeout ordering unchanged (behavioral equivalence)
///
/// This test verifies that the liveness fix didn't change timeout semantics.
/// Timeouts should still fire at approximately the configured time.
#[tokio::test]
async fn test_timeout_ordering_unchanged() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());

    // Create transport with no responses (will trigger timeout)
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: None,
        responses: vec![], // No response - command will timeout
    }])
    .with_executor(executor.clone());

    // Configure specific timeout for measurement
    let ack_timeout = Duration::from_millis(200);
    let timeout_config = TimeoutConfig::builder().ack_timeout(ack_timeout).build();

    let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
        .timeout_config(timeout_config)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("Failed to create camera");

    // Measure how long it takes for the command to timeout
    let start = Instant::now();
    let result = camera.zoom_stop().await;
    let elapsed = start.elapsed();

    // Verify timeout occurred
    assert!(
        matches!(result, Err(Error::Timeout)),
        "Expected Timeout error, got: {:?}",
        result
    );

    // Timeout should occur within a reasonable window of the configured time
    // Allow generous slack for scheduling jitter and retries
    // The retry mechanism may add additional time, so be permissive
    let min_expected = ack_timeout.saturating_sub(Duration::from_millis(50));
    let max_expected = Duration::from_secs(2); // Allow up to 2s for retries

    assert!(
        elapsed >= min_expected,
        "Timeout fired too early: {}ms (expected >= {}ms)",
        elapsed.as_millis(),
        min_expected.as_millis()
    );

    assert!(
        elapsed <= max_expected,
        "Timeout fired too late: {}ms (expected <= {}ms)",
        elapsed.as_millis(),
        max_expected.as_millis()
    );

    drop(camera);
}

/// Regression guard test: Ensures the fix cannot be easily reverted
///
/// This test is designed to FAIL if the liveness fix is reverted.
/// It combines multiple scenarios to catch regressions.
#[tokio::test]
async fn regression_guard_loop_responsiveness() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());

    // Create transport that will create a long-deadline scenario
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        // First command: ACK only (pending with long timeout)
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(1)],
        },
        // Second command: ACK + completion
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(2), helpers::complete(2)],
        },
    ])
    .with_executor(executor.clone());

    let timeout_config = TimeoutConfig::builder()
        .movement_timeout(Duration::from_secs(30)) // Long timeout
        .build();

    // Use RuntimeHandle directly since it implements Clone
    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new_with_timeout(transport, executor.clone(), timeout_config)
            .await
            .expect("Failed to create runtime");

    // Clone for background task
    let runtime_clone = runtime.clone();

    // Spawn first command in background (will get ACK but wait for completion)
    let first_handle = tokio::spawn(async move {
        runtime_clone
            .send_command(
                &grafton_visca::command::zoom::Zoom::TeleStd,
                grafton_visca::CameraId::CAMERA_1,
                None,
            )
            .await
    });

    // Give the runtime time to send and receive ACK
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Time the second command
    let start = Instant::now();
    let result = runtime
        .send_command(
            &grafton_visca::command::zoom::Zoom::Stop,
            grafton_visca::CameraId::CAMERA_1,
            None,
        )
        .await;
    let elapsed = start.elapsed();

    // Verify second command completed
    assert!(
        result.is_ok(),
        "Second command should have succeeded: {:?}",
        result
    );

    // This is the critical regression guard assertion
    // If the fix is reverted, this would take ~30 seconds
    assert!(
        elapsed < Duration::from_secs(2),
        "REGRESSION DETECTED: Loop responsiveness degraded! \
         Second command took {}s (expected < 2s). \
         The liveness fix (issue #442) may have been reverted. \
         Check that the runtime loop awaits all control channels \
         via recv_async() instead of polling with try_recv().",
        elapsed.as_secs_f64()
    );

    // Clean up
    first_handle.abort();
    drop(runtime);
}
