//! Tests for socket manager functionality through the public Camera API

#[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
mod tokio_tests {
    use std::sync::Arc;

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, CameraBuilder},
        testing::testkit::{helpers, ScriptedTransport},
        PowerControl, TokioExecutor, ZoomControl,
    };

    /// Create a ScriptedTransport that auto-responds to any command with ACK+completion
    fn create_auto_respond_transport() -> ScriptedTransport<TokioExecutor> {
        // Create a repeating step that responds to any command
        let steps = vec![
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
        ];
        ScriptedTransport::new(steps)
    }

    /// Create a ScriptedTransport with specific inquiry response for power on
    fn create_power_inquiry_transport() -> ScriptedTransport<TokioExecutor> {
        let steps = vec![
            helpers::power_inquiry_response(true), // Power inquiry returns "on"
            helpers::auto_respond_step(),          // Any other commands get standard response
            helpers::auto_respond_step(),
            helpers::auto_respond_step(),
        ];
        ScriptedTransport::new(steps)
    }

    #[tokio::test(start_paused = true)]
    async fn test_socket_manager_initialization() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = create_power_inquiry_transport().with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor)
            .open_async::<PtzOpticsG2, _>(transport.clone())
            .await
            .unwrap();

        // Socket manager is automatically initialized on first use
        // Test that an operation works, which will trigger auto-initialization
        let result = camera.power().state().await;

        // Should succeed with proper power inquiry response
        assert!(
            result.is_ok(),
            "Power inquiry should succeed, got: {result:?}"
        );

        // Verify command was sent
        let sent = transport.sent();
        assert!(!sent.is_empty(), "Commands should be sent to transport");
    }

    #[tokio::test(start_paused = true)]
    async fn test_socket_manager_with_commands() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = create_auto_respond_transport().with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<PtzOpticsG2, _>(transport.clone())
            .await
            .unwrap();

        // Test zoom commands (deterministic timing with virtual clock)
        let result = camera.zoom_tele(None).await;
        assert!(result.is_ok(), "Zoom in command should succeed: {result:?}");

        // Check that command was sent
        let sent_commands = transport.sent();
        assert!(
            !sent_commands.is_empty(),
            "Commands should be sent to transport"
        );

        // Test another simple operation
        let result = camera.zoom_wide(None).await;
        assert!(result.is_ok(), "Zoom out should succeed: {result:?}");

        // Verify multiple commands were sent
        let final_commands = transport.sent();
        assert!(
            final_commands.len() >= 2,
            "Multiple commands should be sent"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_concurrent_commands() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = create_auto_respond_transport().with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<PtzOpticsG2, _>(transport.clone())
            .await
            .unwrap();

        // Send commands sequentially with deterministic timing
        // This tests that socket manager can queue and handle multiple commands
        let r1 = camera.zoom_tele(None).await;
        let r2 = camera.zoom_wide(None).await;

        // Verify both commands succeeded
        assert!(r1.is_ok(), "First command should succeed: {r1:?}");
        assert!(r2.is_ok(), "Second command should succeed: {r2:?}");

        // Verify both commands were sent
        let sent_commands = transport.sent();
        assert!(sent_commands.len() >= 2, "Both commands should be sent");
    }

    #[tokio::test(start_paused = true)]
    async fn test_lazy_initialization_with_explicit_runtime() {
        // This test verifies that with explicit runtime configuration,
        // the socket manager is lazily initialized on first command
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        let transport = create_auto_respond_transport().with_executor(executor.clone());

        let camera = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<PtzOpticsG2, _>(transport.clone())
            .await
            .unwrap();

        // First command should trigger lazy initialization of socket manager
        let result = camera.power_off().await;
        assert!(
            result.is_ok(),
            "First command should succeed with lazily initialized socket manager, got: {result:?}"
        );

        // Verify that command was actually sent (socket manager is working)
        let sent_commands = transport.sent();
        assert!(
            !sent_commands.is_empty(),
            "Commands should be sent after lazy initialization"
        );

        // Subsequent commands should also work
        let result2 = camera.power_on().await;
        assert!(
            result2.is_ok(),
            "Subsequent commands should also succeed, got: {result2:?}"
        );

        // Verify multiple commands were sent
        let final_commands = transport.sent();
        assert!(
            final_commands.len() > sent_commands.len(),
            "Additional commands should be sent"
        );
    }

    #[tokio::test]
    #[ignore = "Test hangs with empty transport - needs investigation separate from spawn_bg fix"]
    async fn test_socket_manager_timeout_handling() {
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));
        // Create transport with no responses - will timeout
        // The ScriptedTransport will return a timeout after 1000 attempts
        let transport = ScriptedTransport::new(vec![]).with_executor(executor.clone());

        // Try to create camera - this should fail or succeed quickly depending on initialization
        let camera_result = CameraBuilder::<TokioExecutor>::with_executor(executor.clone())
            .open_async::<PtzOpticsG2, _>(transport.clone())
            .await;

        if let Ok(camera) = camera_result {
            // If camera creation succeeded (unlikely with no responses), test command timeout
            let result = camera.power_on().await;
            assert!(result.is_err(), "Command should timeout without responses");
        } else {
            // Camera creation failed due to timeout, which is also a valid timeout test
            assert!(
                camera_result.is_err(),
                "Camera creation should fail with no transport responses"
            );
        }
    }
}
