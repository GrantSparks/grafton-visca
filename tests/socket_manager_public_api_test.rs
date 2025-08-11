//! Tests for socket manager functionality through the public Camera API

#[cfg(feature = "rt-tokio")]
mod tokio_tests {
    use bytes::Bytes;
    use grafton_visca::r#async::prelude::*;
    use grafton_visca::runtime::TokioRuntime;
    use grafton_visca::transport::Transport;
    use grafton_visca::{
        camera::profiles::PTZOpticsG2, r#async, Camera, Error, PanTiltDirection, PresetNumber,
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
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                sent_commands: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(VecDeque::new())),
                auto_respond: false,
            }
        }

        fn with_auto_respond() -> Self {
            Self {
                sent_commands: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(VecDeque::new())),
                auto_respond: true,
            }
        }

        fn get_sent_commands(&self) -> Vec<Vec<u8>> {
            self.sent_commands.lock().unwrap().clone()
        }

        fn generate_visca_ack_completion(&self) -> (Bytes, Bytes) {
            // Always use Socket1 for responses - this simulates the camera
            // using Socket1 for all sequential commands
            let socket_byte = 0x90;

            // Generate ACK response
            let ack = Bytes::from(vec![socket_byte, 0x41, VISCA_TERMINATOR]);
            // Generate Completion response
            let completion = Bytes::from(vec![socket_byte, 0x51, VISCA_TERMINATOR]);
            (ack, completion)
        }

        fn with_concurrent_response() -> Self {
            let mut transport = Self::new();
            transport.auto_respond = true;
            transport
        }
    }

    impl Transport for MockTransport {
        type Error = Error;
        type SendFut<'a> = core::future::Ready<Result<(), Error>>;
        type RecvFut<'a> =
            std::pin::Pin<Box<dyn std::future::Future<Output = Result<Bytes, Error>> + Send + 'a>>;

        fn send(&self, bytes: &[u8]) -> Self::SendFut<'_> {
            {
                let mut commands = self.sent_commands.lock().unwrap();
                commands.push(bytes.to_vec());
            }

            // If auto-respond is enabled, generate responses
            if self.auto_respond {
                // For any VISCA command, generate ACK and completion
                let (ack, completion) = self.generate_visca_ack_completion();
                let mut responses = self.responses.lock().unwrap();
                responses.push_back(Ok(ack));
                responses.push_back(Ok(completion));
            }

            core::future::ready(Ok(()))
        }

        fn recv(&self) -> Self::RecvFut<'_> {
            let responses = self.responses.clone();

            Box::pin(async move {
                // Poll for responses with a reasonable timeout
                let timeout = tokio::time::Instant::now() + Duration::from_secs(5);

                loop {
                    // Check if response is available
                    {
                        let mut responses = responses.lock().unwrap();
                        if let Some(response) = responses.pop_front() {
                            return response;
                        }
                    }

                    // Check if we've timed out
                    if tokio::time::Instant::now() >= timeout {
                        return Err(Error::TransportError(std::borrow::Cow::Borrowed(
                            "No response available after timeout",
                        )));
                    }

                    // Wait a short time before checking again
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
        }
    }

    // MockTransport already implements Transport, no need for UnifiedTransport

    #[tokio::test]
    async fn test_socket_manager_initialization() {
        let transport = MockTransport::new();
        let handle = tokio::runtime::Handle::current();
        let runtime = Arc::new(TokioRuntime);
        let mut inner_camera = Camera::<PTZOpticsG2, _>::new(transport)
            .with_spawner(handle)
            .with_runtime(runtime);

        // Test initialization through public API
        let result = inner_camera.initialize_socket_manager();
        assert!(
            result.is_ok(),
            "Socket manager should initialize successfully"
        );

        let camera = r#async::Camera::new(inner_camera);
        // Camera is now ready with socket manager initialized
        drop(camera);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_socket_manager_with_commands() {
        let transport = MockTransport::with_auto_respond();
        let handle = tokio::runtime::Handle::current();
        let runtime = Arc::new(TokioRuntime);
        let mut inner_camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_spawner(handle)
            .with_runtime(runtime);

        // Initialize socket manager
        inner_camera
            .initialize_socket_manager()
            .expect("Failed to initialize socket manager");

        let camera = r#async::Camera::new(inner_camera);

        // Give the actor time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test sending commands through the public API with timeout
        // Note: PTZOpticsG2 has a 10-second power on time, so we need a longer timeout
        let result = tokio::time::timeout(Duration::from_secs(15), camera.power_on()).await;
        assert!(
            result.is_ok(),
            "Power on command should complete within timeout"
        );
        assert!(result.unwrap().is_ok(), "Power on command should succeed");

        // Check that command was sent
        let sent_commands = transport.get_sent_commands();
        assert!(
            !sent_commands.is_empty(),
            "Commands should be sent to transport"
        );

        // Test other camera operations with timeout
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            camera.preset_recall(PresetNumber::new(1).unwrap()),
        )
        .await;
        assert!(
            result.is_ok(),
            "Preset recall should complete within timeout"
        );
        assert!(result.unwrap().is_ok(), "Recall preset should succeed");

        let result = tokio::time::timeout(
            Duration::from_secs(2),
            camera.pan_tilt_move(
                PanTiltDirection::Up,
                5.try_into().unwrap(),
                5.try_into().unwrap(),
            ),
        )
        .await;
        assert!(
            result.is_ok(),
            "Move direction should complete within timeout"
        );
        assert!(result.unwrap().is_ok(), "Move direction should succeed");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_concurrent_commands() {
        let transport = MockTransport::with_concurrent_response();
        let handle = tokio::runtime::Handle::current();
        let runtime = Arc::new(TokioRuntime);
        let mut inner_camera = Camera::<PTZOpticsG2, _>::new(transport.clone())
            .with_spawner(handle)
            .with_runtime(runtime);

        inner_camera
            .initialize_socket_manager()
            .expect("Failed to initialize socket manager");

        let camera = r#async::Camera::new(inner_camera);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send multiple commands concurrently with timeout
        let result = tokio::time::timeout(Duration::from_secs(2), async {
            tokio::join!(camera.zoom_in(), camera.zoom_out())
        })
        .await;

        assert!(
            result.is_ok(),
            "Concurrent commands should complete within timeout"
        );
        let (r1, r2) = result.unwrap();

        // Print debug info to understand what's happening
        eprintln!("Result 1: {r1:?}");
        eprintln!("Result 2: {r2:?}");

        // Verify both commands were sent
        let sent_commands = transport.get_sent_commands();
        let count = sent_commands.len();
        eprintln!("Sent commands count: {count}");
        for (i, cmd) in sent_commands.iter().enumerate() {
            eprintln!("Command {i}: {cmd:02x?}");
        }

        assert!(r1.is_ok(), "First command should succeed");
        assert!(r2.is_ok(), "Second command should succeed");
        assert!(sent_commands.len() >= 2, "Both commands should be sent");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_command_without_socket_manager() {
        // Use auto-respond for automatic ACK and completion
        let transport = MockTransport::with_auto_respond();

        let runtime = Arc::new(TokioRuntime);
        let inner_camera =
            Camera::<PTZOpticsG2, _>::new(transport.clone()).with_runtime_only(runtime);
        let camera = r#async::Camera::new(inner_camera);

        // Don't initialize socket manager - commands should still work via direct transport
        // Note: PTZOpticsG2 has a 10-second power on time, so we need a longer timeout
        let result = tokio::time::timeout(Duration::from_secs(15), camera.power_on()).await;
        assert!(result.is_ok(), "Command should complete within timeout");
        let inner_result = result.unwrap();
        assert!(
            inner_result.is_ok(),
            "Command should work without socket manager: {:?}",
            inner_result
        );

        let sent_commands = transport.get_sent_commands();
        assert_eq!(sent_commands.len(), 1, "Command should be sent directly");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_socket_manager_timeout_handling() {
        let transport = MockTransport::new(); // No auto-respond
        let handle = tokio::runtime::Handle::current();
        let runtime = Arc::new(TokioRuntime);
        let mut inner_camera = Camera::<PTZOpticsG2, _>::new(transport)
            .with_spawner(handle)
            .with_runtime(runtime);

        inner_camera
            .initialize_socket_manager()
            .expect("Failed to initialize socket manager");

        let camera = r#async::Camera::new(inner_camera);

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
