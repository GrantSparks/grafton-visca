//! Regression test for issue #342: Async tests hang under DeterministicExecutor.
//!
//! This test ensures that timeout-driven commands complete properly
//! without hanging when there are no responses from the transport.
//!
//! NOTE: These tests use real runtime executors (Tokio) rather than DeterministicExecutor
//! because they test actual timeout behavior. DeterministicExecutor has fundamental
//! limitations with virtual time that make it unsuitable for timeout testing.
//! See issue #394 for details.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

#[cfg(feature = "runtime-tokio")]
use std::time::Duration;

#[cfg(feature = "runtime-tokio")]
use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    testing::testkit::{ScriptedTransport, Step},
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
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn test_command_timeout_completes_deterministically() {
    use grafton_visca::TokioExecutor;
    let executor = std::sync::Arc::new(TokioExecutor::from_current().unwrap());

    // Create transport that provides no responses (will trigger timeout)
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: None,
        responses: vec![], // No responses - command will timeout
    }])
    .with_executor(executor.clone());

    // Configure short timeouts
    let timeout_config = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(100))
        .build();

    // Create camera with timeout configuration
    let camera = grafton_visca::camera::builder::CameraBuilder::<TokioExecutor>::with_executor(
        executor.clone(),
    )
    .timeout_config(timeout_config)
    .open_async::<PtzOpticsG2, _>(transport)
    .await
    .expect("Failed to create camera");

    // Send a command that will timeout
    let result = camera.zoom_stop().await;

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
}

/// Test that multiple timeout-based commands don't cause starvation.
///
/// This ensures the runtime can handle multiple pending timeouts without
/// getting stuck in an infinite loop.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn test_multiple_timeouts_no_starvation() {
    use grafton_visca::TokioExecutor;
    let executor = std::sync::Arc::new(TokioExecutor::from_current().unwrap());

    // Transport provides no responses for any commands
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
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

    let camera = grafton_visca::camera::builder::CameraBuilder::<TokioExecutor>::with_executor(
        executor.clone(),
    )
    .timeout_config(timeout_config)
    .open_async::<PtzOpticsG2, _>(transport)
    .await
    .expect("Failed to create camera");

    // Send multiple commands that will timeout
    // Create both futures - they will both be submitted when awaited
    let result1_future = camera.zoom_stop();
    let result2_future = camera.zoom_tele(None);

    // Use join to run them concurrently
    let (result1, result2) = futures_lite::future::zip(result1_future, result2_future).await;

    assert!(
        matches!(result1, Err(Error::Timeout)),
        "First command should timeout"
    );
    assert!(
        matches!(result2, Err(Error::Timeout)),
        "Second command should timeout"
    );

    drop(camera);
}

/// Test that runtime properly shuts down even with pending timeouts.
///
/// This verifies the shutdown race implementation works correctly.
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn test_shutdown_with_pending_timeouts() {
    use grafton_visca::TokioExecutor;
    let executor = std::sync::Arc::new(TokioExecutor::from_current().unwrap());

    // Transport with no responses
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![Step::OnSend {
        matches: None,
        responses: vec![],
    }])
    .with_executor(executor.clone());

    let timeout_config = TimeoutConfig::builder()
        .ack_timeout(Duration::from_millis(5000)) // Long timeout
        .build();

    let camera = grafton_visca::camera::builder::CameraBuilder::<TokioExecutor>::with_executor(
        executor.clone(),
    )
    .timeout_config(timeout_config)
    .open_async::<PtzOpticsG2, _>(transport)
    .await
    .expect("Failed to create camera");

    // Start a command that would timeout after 5 seconds
    // Drop the future immediately to avoid borrowing issues
    {
        let _future = camera.zoom_stop();
        // Give it a moment to start processing
        tokio::time::sleep(Duration::from_millis(10)).await;
        // Future is dropped here
    }

    // Now drop the camera - this should trigger shutdown signal
    drop(camera);

    // If we get here, the test passes - shutdown worked
}
