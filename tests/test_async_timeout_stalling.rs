//! Regression test for issue #342: Async tests hang under DeterministicExecutor.
//!
//! This test ensures that timeout-driven commands complete deterministically
//! without hanging when there are no responses from the transport.

#![cfg(all(feature = "async", feature = "test-utils"))]

use std::time::Duration;

use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    prelude::advanced::ProtocolStyle,
    testing::testkit::{deterministic_executor::DeterministicExecutor, ScriptedTransport, Step},
    timeout::TimeoutConfig,
    Error, ZoomControl,
};

/// Minimal standalone repro test for timeout stalling.
///
/// This test:
/// - Sends one command with no responses at all
/// - Uses short ACK timeout (50-100ms)
/// - Verifies the command completes with Error::Timeout under deterministic executor
/// - Acts as a sentinel for future changes
#[test]
fn test_command_timeout_completes_deterministically() {
    let (executor, clock) = DeterministicExecutor::new();

    // Create transport that provides no responses (will trigger timeout)
    let transport: ScriptedTransport<DeterministicExecutor> =
        ScriptedTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![], // No responses - command will timeout
        }])
        .with_executor(executor.clone());

    // Configure short timeouts
    let timeout_config = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(100))
        .build();

    let exec = executor.clone();
    let clock_clone = clock.clone();

    executor.block_on_bg(async move {
        // Create camera with timeout configuration
        let camera =
            grafton_visca::camera::builder::CameraBuilder::<DeterministicExecutor>::with_executor(
                exec.clone(),
            )
            .timeout_config(timeout_config)
            .protocol_style(ProtocolStyle::RawVisca)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Send a command that will timeout
        let result_future = camera.zoom_stop();

        // Drive the executor to process initial setup
        exec.drive_until_idle();

        // Advance virtual time to trigger timeout
        // We need to advance past the ACK timeout (100ms)
        clock_clone.advance(Duration::from_millis(150));
        exec.drive_until_idle();

        // The command should complete with a timeout error
        let result = result_future.await;

        // Verify we got a timeout error
        assert!(result.is_err(), "Command should have timed out");
        match result.unwrap_err() {
            Error::Timeout => {
                // Expected - test passes
            }
            other => panic!("Expected Error::Timeout, got: {:?}", other),
        }

        // Clean shutdown
        drop(camera);
    });
}

/// Test that multiple timeout-based commands don't cause starvation.
///
/// This ensures the runtime can handle multiple pending timeouts without
/// getting stuck in an infinite loop.
#[test]
fn test_multiple_timeouts_no_starvation() {
    let (executor, clock) = DeterministicExecutor::new();

    // Transport provides no responses for any commands
    let transport: ScriptedTransport<DeterministicExecutor> = ScriptedTransport::new(vec![
        Step::OnSend {
            matches: None,
            responses: vec![], // First command - no response
        },
        Step::OnSend {
            matches: None,
            responses: vec![], // Second command - no response
        },
    ])
    .with_executor(executor.clone());

    // Configure timeouts
    let timeout_config = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(80))
        .build();

    let exec = executor.clone();
    let clock_clone = clock.clone();

    executor.block_on_bg(async move {
        let camera =
            grafton_visca::camera::builder::CameraBuilder::<DeterministicExecutor>::with_executor(
                exec.clone(),
            )
            .timeout_config(timeout_config)
            .protocol_style(ProtocolStyle::RawVisca)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Send multiple commands that will timeout
        // Create both futures - they will both be submitted when awaited
        let result1_future = camera.zoom_stop();
        let result2_future = camera.zoom_tele_std();

        // Use join to run them concurrently
        let both_results = futures_lite::future::zip(result1_future, result2_future);

        // Process initial setup
        exec.drive_until_idle();

        // Advance time to trigger timeouts
        clock_clone.advance(Duration::from_millis(100));
        exec.drive_until_idle();

        // Both commands should timeout
        let (result1, result2) = both_results.await;

        assert!(
            matches!(result1, Err(Error::Timeout)),
            "First command should timeout"
        );
        assert!(
            matches!(result2, Err(Error::Timeout)),
            "Second command should timeout"
        );

        drop(camera);
    });
}

/// Test that runtime properly shuts down even with pending timeouts.
///
/// This verifies the shutdown race implementation works correctly.
#[test]
fn test_shutdown_with_pending_timeouts() {
    let (executor, _clock) = DeterministicExecutor::new();

    // Transport with no responses
    let transport: ScriptedTransport<DeterministicExecutor> =
        ScriptedTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![],
        }])
        .with_executor(executor.clone());

    let timeout_config = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(5000)) // Long timeout
        .build();

    let exec = executor.clone();

    executor.block_on_bg(async move {
        let camera =
            grafton_visca::camera::builder::CameraBuilder::<DeterministicExecutor>::with_executor(
                exec.clone(),
            )
            .timeout_config(timeout_config)
            .protocol_style(ProtocolStyle::RawVisca)
            .open_async::<PtzOpticsG2, _>(transport)
            .await
            .expect("Failed to create camera");

        // Start a command that would timeout after 5 seconds
        // Drop the future immediately to avoid borrowing issues
        {
            let _future = camera.zoom_stop();
            // Start processing the command
            exec.drive_until_idle();
            // Future is dropped here
        }

        // Now drop the camera - this should trigger shutdown signal
        drop(camera);

        // The runtime should shut down cleanly without waiting for the 5-second timeout
        exec.drive_until_idle();

        // If we get here, the test passes - shutdown worked
    });
}
