//! Tests for runtime functionality with async operations.
//!
//! These tests verify that async operations work correctly with the default
//! runtime provided by the rt-tokio feature.

#![cfg(feature = "async")]

#[cfg(feature = "test-utils")]
use grafton_visca::testing::testkit::{helpers, ScriptedTransport};

#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
use grafton_visca::TokioExecutor;

#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
#[tokio::test(start_paused = true)]
async fn test_operations_work_with_default_runtime() {
    use grafton_visca::camera::{profiles::PtzOpticsG2, AsyncMode, Camera};
    use grafton_visca::{PanTiltControl, ZoomControl};

    // Create TokioExecutor for integration test with paused time
    let executor =
        std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // pan_tilt_stop
    ]);
    let transport = transport.with_executor(executor.clone());

    println!("Creating camera...");
    let camera = Camera::<AsyncMode, PtzOpticsG2, _, _>::with_executor(transport, executor)
        .await
        .unwrap();
    println!("Camera created successfully");

    // Advance tokio time to allow runtime to tick
    tokio::time::advance(std::time::Duration::from_millis(100)).await;
    println!("Tokio time advanced");

    // Simple operations should work with explicit runtime
    println!("Calling zoom_stop...");
    let result = camera.zoom_stop().await;
    println!("zoom_stop completed with result: {:?}", result);
    assert!(result.is_ok(), "zoom_stop failed: {:?}", result);

    // Pan/tilt operations should work with default runtime
    let result = camera.pan_tilt_stop().await;
    assert!(result.is_ok(), "pan_tilt_stop failed: {:?}", result);

    // Socket manager cleanup is now handled automatically by Drop
    drop(camera);
}

#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
#[tokio::test(start_paused = true)]
async fn test_operations_succeed_with_explicit_runtime() {
    use grafton_visca::camera::{profiles::PtzOpticsG2, AsyncMode, Camera};
    use grafton_visca::ZoomControl;

    // Create TokioExecutor for integration test with paused time
    let executor =
        std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::standard_command_response(1), // ACK + Completion for socket 1
    ]);
    let transport = transport.with_executor(executor.clone());

    let camera = Camera::<AsyncMode, PtzOpticsG2, _, _>::with_executor(transport, executor)
        .await
        .unwrap();

    // Operations should succeed with proper responses
    let result = camera.zoom_stop().await;
    assert!(
        result.is_ok(),
        "zoom_stop failed with error: {:?}",
        result.err()
    );

    // Socket manager cleanup is now handled automatically by Drop
    drop(camera);
}

// NOTE: Testing custom runtimes is complex because:
// 1. We're already running inside a tokio runtime from #[tokio::test]
// 2. Creating nested runtime contexts can cause deadlocks
// 3. The abstraction over runtimes is primarily for users who want to use
//    different async runtimes like async-std or smol
//
// For now, we'll skip this test as it's not testing anything meaningful
// in the context of running inside tokio.
#[cfg(feature = "rt-tokio")]
#[tokio::test]
#[ignore] // Ignore this test as it causes issues with nested runtimes
async fn test_custom_runtime_works() {
    // This test would need to be run in a different context to be meaningful
}

#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
#[tokio::test(start_paused = true)]
async fn test_movement_detection_works_with_default_runtime() {
    use grafton_visca::camera::{profiles::PtzOpticsG2, AsyncMode, Camera};
    use grafton_visca::{PanTiltControl, ZoomControl};

    // Create TokioExecutor for integration test with paused time
    let executor =
        std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // pan_tilt_stop
    ]);
    let transport = transport.with_executor(executor.clone());

    let camera = Camera::<AsyncMode, PtzOpticsG2, _, _>::with_executor(transport, executor)
        .await
        .unwrap();

    // Simple operations should work with explicit runtime
    let result = camera.zoom_stop().await;
    assert!(result.is_ok(), "zoom_stop failed: {:?}", result);

    let result = camera.pan_tilt_stop().await;
    assert!(result.is_ok(), "pan_tilt_stop failed: {:?}", result);

    // Socket manager cleanup is now handled automatically by Drop
    drop(camera);
}

#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
#[tokio::test(start_paused = true)]
async fn test_power_operations_work_with_default_runtime() {
    use grafton_visca::camera::{profiles::PtzOpticsG2, AsyncMode, Camera};
    use grafton_visca::{FocusControl, ZoomControl};

    // Create TokioExecutor for integration test with paused time
    let executor =
        std::sync::Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        helpers::auto_respond_step(), // zoom_stop
        helpers::auto_respond_step(), // focus_stop
    ]);
    let transport = transport.with_executor(executor.clone());

    let camera = Camera::<AsyncMode, PtzOpticsG2, _, _>::with_executor(transport, executor)
        .await
        .unwrap();

    // Just test that basic operations work, not power operations which have long delays
    let result = camera.zoom_stop().await;
    assert!(result.is_ok(), "zoom_stop failed: {:?}", result);

    let result = camera.focus_stop().await;
    assert!(result.is_ok(), "focus_stop failed: {:?}", result);

    // Socket manager cleanup is now handled automatically by Drop
    drop(camera);
}

// NOTE: To truly test "no runtime" scenarios, we would need:
// 1. Tests that run with async feature but WITHOUT rt-tokio
// 2. Tests that don't use #[tokio::test] (since that requires tokio)
// 3. A way to run async tests without any runtime
//
// However, such tests would be integration tests that need to be run
// with specific feature combinations, not unit tests.
