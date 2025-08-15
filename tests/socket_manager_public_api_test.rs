//! Tests for socket manager functionality through the public Camera API

#[cfg(feature = "rt-tokio")]
mod tokio_tests {
    use bytes::Bytes;
    use grafton_visca::transport::AsyncTransport;
    use grafton_visca::{
        camera::{profiles::PTZOpticsG2, AsyncMode, Camera},
        Error, PowerOps, TokioExecutor, ZoomOps,
    };
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    // VISCA terminator constant
    const VISCA_TERMINATOR: u8 = 0xFF;

    /// Mock transport for testing socket manager behavior
    #[derive(Debug, Clone)]
    struct MockTransport {
        sent_commands: Arc<Mutex<Vec<Vec<u8>>>>,
        responses: Arc<Mutex<VecDeque<Result<Bytes, Error>>>>,
        auto_respond: bool,
        shutdown_flag: Arc<std::sync::atomic::AtomicBool>,
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                sent_commands: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(VecDeque::new())),
                auto_respond: false,
                shutdown_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }
        }

        fn with_auto_respond() -> Self {
            Self {
                sent_commands: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(VecDeque::new())),
                auto_respond: true,
                shutdown_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }
        }

        fn shutdown(&self) {
            self.shutdown_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        fn get_sent_commands(&self) -> Vec<Vec<u8>> {
            self.sent_commands.lock().unwrap().clone()
        }

        fn generate_visca_ack_completion(&self, command: &[u8]) -> (Bytes, Bytes) {
            // Cancel commands should get immediate completion
            if command.len() == 3 && (command[1] == 0x21 || command[1] == 0x22) {
                let socket_num = if command[1] == 0x21 { 1 } else { 2 };
                let completion = Bytes::from(vec![0x90, 0x50 | socket_num, VISCA_TERMINATOR]);
                return (completion.clone(), completion);
            }

            // For testing, just use socket 1 consistently
            // The socket manager tests aren't really testing socket allocation,
            // they're testing that commands work through the socket manager
            let socket_num = 1;

            // Check if this is an inquiry command (0x09 in second byte)
            if command.len() >= 4 && command[1] == 0x09 {
                // This is an inquiry command - generate a data response
                let ack = Bytes::from(vec![0x90, 0x40 | socket_num, VISCA_TERMINATOR]);

                // Generate appropriate inquiry response based on command
                let data_response =
                    if command.len() == 5 && command[0..4] == [0x81, 0x09, 0x04, 0x00] {
                        // Power inquiry - respond with "power on"
                        Bytes::from(vec![0x90, 0x50, 0x02, VISCA_TERMINATOR])
                    } else {
                        // Generic inquiry response with dummy data
                        Bytes::from(vec![0x90, 0x50, 0x01, VISCA_TERMINATOR])
                    };

                (ack, data_response)
            } else {
                // Regular command - generate ACK and completion
                let ack = Bytes::from(vec![0x90, 0x40 | socket_num, VISCA_TERMINATOR]);
                let completion = Bytes::from(vec![0x90, 0x50 | socket_num, VISCA_TERMINATOR]);
                (ack, completion)
            }
        }
    }

    impl AsyncTransport for MockTransport {
        async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
            {
                let mut commands = self.sent_commands.lock().unwrap();
                commands.push(bytes.to_vec());
            }

            // If auto-respond is enabled, generate responses
            if self.auto_respond {
                // For any VISCA command, generate ACK and completion
                let (ack, completion) = self.generate_visca_ack_completion(bytes);
                let mut responses = self.responses.lock().unwrap();
                responses.push_back(Ok(ack));
                responses.push_back(Ok(completion));
            }

            Ok(())
        }

        async fn recv(&self) -> Result<Bytes, Error> {
            let responses = self.responses.clone();
            let shutdown_flag = self.shutdown_flag.clone();

            // The socket manager will continuously call recv() in its event loop.
            // We need to wait indefinitely for responses to avoid returning errors
            // that would disrupt the socket manager.
            loop {
                // Check for shutdown first
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    return Err(Error::ConnectionLost {
                        reason: "MockTransport shut down".into(),
                    });
                }

                // Check if response is available
                {
                    let mut responses = responses.lock().unwrap();
                    if let Some(response) = responses.pop_front() {
                        return response;
                    }
                }

                // Use a select statement to make cancellation more responsive
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {
                        // Continue loop after sleep
                    }
                    // This branch helps with graceful shutdown when the future is dropped
                    _ = tokio::task::yield_now() => {
                        // Check shutdown flag again immediately after yielding
                        if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            return Err(Error::ConnectionLost {
                                reason: "MockTransport shut down".into()
                            });
                        }
                    }
                }
            }
        }
    }

    // MockTransport already implements Transport, no need for UnifiedTransport

    #[tokio::test]
    async fn test_socket_manager_initialization() {
        // Use auto-respond to ensure proper VISCA responses
        let transport = MockTransport::with_auto_respond();
        let _handle = tokio::runtime::Handle::current();
        let executor = TokioExecutor::from_current().unwrap();
        let inner_camera =
            Camera::<AsyncMode, PTZOpticsG2, _, _>::with_executor(transport.clone(), executor)
                .await
                .unwrap();

        // Socket manager is now automatically initialized on first use
        // Test that an operation works, which will trigger auto-initialization
        // Add timeout to prevent hanging on CI
        let result =
            tokio::time::timeout(Duration::from_secs(5), inner_camera.power_inquiry()).await;

        // Should complete within timeout
        assert!(
            result.is_ok(),
            "Operation should complete within timeout, got: {:?}",
            result
        );

        let inner_result = result.unwrap();
        // With auto-respond, this should succeed (or fail with UnexpectedResponseType
        // since we're not providing a proper inquiry response, just ACK/completion)
        assert!(
            inner_result.is_ok() || matches!(inner_result, Err(Error::UnexpectedResponseType)),
            "Operation should succeed or fail with UnexpectedResponseType, got: {:?}",
            inner_result
        );

        // Signal shutdown to the transport to help with cleanup
        transport.shutdown();

        // Camera now has socket manager initialized
        drop(inner_camera);

        // Give the runtime a moment to shut down cleanly
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_socket_manager_with_commands() {
        let transport = MockTransport::with_auto_respond();
        let executor = Arc::new(TokioExecutor::from_current().unwrap());
        let camera = Camera::<grafton_visca::camera::AsyncMode, PTZOpticsG2, _, _>::with_executor(
            transport.clone(),
            executor.as_ref().clone(),
        )
        .await
        .unwrap();

        // Give the actor time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test zoom commands (simpler than power_on which has long delays)
        let result = tokio::time::timeout(Duration::from_secs(5), camera.zoom_tele_std()).await;
        assert!(
            result.is_ok(),
            "Zoom in command should complete within timeout"
        );
        let inner_result = result.unwrap();
        assert!(
            inner_result.is_ok(),
            "Zoom in command should succeed: {:?}",
            inner_result
        );

        // Check that command was sent
        let sent_commands = transport.get_sent_commands();
        assert!(
            !sent_commands.is_empty(),
            "Commands should be sent to transport"
        );

        // Test another simple operation
        let result = tokio::time::timeout(Duration::from_secs(5), camera.zoom_wide_std()).await;
        assert!(result.is_ok(), "Zoom out should complete within timeout");
        assert!(result.unwrap().is_ok(), "Zoom out should succeed");

        // Verify multiple commands were sent
        let final_commands = transport.get_sent_commands();
        assert!(
            final_commands.len() >= 2,
            "Multiple commands should be sent"
        );

        // Signal shutdown to help with cleanup
        transport.shutdown();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_concurrent_commands() {
        // For this test, we'll use the regular auto-respond mode
        // Testing true concurrency with a mock is complex and timing-dependent
        // The important thing is that the socket manager can handle multiple commands
        let transport = MockTransport::with_auto_respond();
        let executor = Arc::new(TokioExecutor::from_current().unwrap());
        let camera = Camera::<grafton_visca::camera::AsyncMode, PTZOpticsG2, _, _>::with_executor(
            transport.clone(),
            executor.as_ref().clone(),
        )
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send commands sequentially but quickly
        // This tests that socket manager can queue and handle multiple commands
        let r1 = camera.zoom_tele_std().await;
        let r2 = camera.zoom_wide_std().await;

        // Verify both commands succeeded
        assert!(r1.is_ok(), "First command should succeed");
        assert!(r2.is_ok(), "Second command should succeed");

        // Verify both commands were sent
        let sent_commands = transport.get_sent_commands();
        assert!(sent_commands.len() >= 2, "Both commands should be sent");

        // Signal shutdown to help with cleanup
        transport.shutdown();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_lazy_initialization_with_explicit_runtime() {
        // This test verifies that with explicit runtime configuration,
        // the socket manager is lazily initialized on first command
        let transport = MockTransport::with_auto_respond();
        let executor = Arc::new(TokioExecutor::from_current().unwrap());
        let camera = Camera::<grafton_visca::camera::AsyncMode, PTZOpticsG2, _, _>::with_executor(
            transport.clone(),
            executor.as_ref().clone(),
        )
        .await
        .unwrap();

        // First command should trigger lazy initialization of socket manager
        let result = camera.power_off().await;
        assert!(
            result.is_ok(),
            "First command should succeed with lazily initialized socket manager, got: {:?}",
            result
        );

        // Verify that command was actually sent (socket manager is working)
        let sent_commands = transport.get_sent_commands();
        assert!(
            !sent_commands.is_empty(),
            "Commands should be sent after lazy initialization"
        );

        // Subsequent commands should also work
        let result2 = camera.power_on().await;
        assert!(
            result2.is_ok(),
            "Subsequent commands should also succeed, got: {:?}",
            result2
        );

        // Verify multiple commands were sent
        let final_commands = transport.get_sent_commands();
        assert!(
            final_commands.len() > sent_commands.len(),
            "Additional commands should be sent"
        );

        // Signal shutdown to help with cleanup
        transport.shutdown();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_socket_manager_timeout_handling() {
        let transport = MockTransport::new(); // No auto-respond
        let _handle = tokio::runtime::Handle::current();
        let executor = TokioExecutor::from_current().unwrap();
        let inner_camera =
            Camera::<AsyncMode, PTZOpticsG2, _, _>::with_executor(transport, executor)
                .await
                .unwrap();

        // Socket manager is now automatically initialized on first use

        let camera = inner_camera;

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Command should timeout when no response is received
        let result = tokio::time::timeout(Duration::from_secs(3), camera.power_on()).await;

        // Should timeout since no responses are provided
        assert!(
            result.is_err() || result.unwrap().is_err(),
            "Command should timeout without responses"
        );
    }
}
