//! Tests for socket manager functionality through the public Camera API

#[cfg(feature = "tokio")]
mod tokio_tests {
    use bytes::Bytes;
    use grafton_visca::r#async::{PanTiltOps, PowerOps, PresetsOps, ZoomOps};
    use grafton_visca::transport::Transport;
    use grafton_visca::{Camera, CameraModel, Error, PanTiltDirection, PresetNumber};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

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

        fn add_response(&self, response: Result<Bytes, Error>) {
            self.responses.lock().unwrap().push_back(response);
        }

        fn generate_visca_ack_completion(&self) -> (Bytes, Bytes) {
            // Generate ACK response (90 41 FF for socket 1)
            let ack = Bytes::from(vec![0x90, 0x41, 0xFF]);
            // Generate Completion response (90 51 FF for socket 1)
            let completion = Bytes::from(vec![0x90, 0x51, 0xFF]);
            (ack, completion)
        }
    }

    impl Transport for MockTransport {
        type Error = Error;
        type SendFut<'a> = core::future::Ready<Result<(), Error>>;
        type RecvFut<'a> =
            std::pin::Pin<Box<dyn std::future::Future<Output = Result<Bytes, Error>> + Send + 'a>>;

        fn send(&self, bytes: &[u8]) -> Self::SendFut<'_> {
            self.sent_commands.lock().unwrap().push(bytes.to_vec());

            // If auto-respond is enabled, queue up ACK and Completion responses
            if self.auto_respond {
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
                // Simulate a short delay before response
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut responses = responses.lock().unwrap();
                responses.pop_front().unwrap_or_else(|| {
                    Err(Error::TransportError("No response available".to_string()))
                })
            })
        }
    }

    #[tokio::test]
    async fn test_socket_manager_initialization() {
        let transport = MockTransport::new();
        let handle = tokio::runtime::Handle::current();
        let mut camera =
            Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport, handle);

        // Test initialization through public API
        let result = camera.initialize_socket_manager();
        assert!(
            result.is_ok(),
            "Socket manager should initialize successfully"
        );
    }

    #[tokio::test]
    async fn test_socket_manager_with_commands() {
        let transport = MockTransport::with_auto_respond();
        let handle = tokio::runtime::Handle::current();
        let mut camera =
            Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport.clone(), handle);

        // Initialize socket manager
        camera
            .initialize_socket_manager()
            .expect("Failed to initialize socket manager");

        // Give the actor time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Test sending commands through the public API
        let result = camera.power_on().await;
        assert!(result.is_ok(), "Power on command should succeed");

        // Check that command was sent
        let sent_commands = transport.get_sent_commands();
        assert!(
            !sent_commands.is_empty(),
            "Commands should be sent to transport"
        );

        // Test other camera operations
        let result = camera.preset_recall(PresetNumber::new(1).unwrap()).await;
        assert!(result.is_ok(), "Recall preset should succeed");

        let result = camera
            .pan_tilt_move(
                PanTiltDirection::Up,
                5.try_into().unwrap(),
                5.try_into().unwrap(),
            )
            .await;
        assert!(result.is_ok(), "Move direction should succeed");
    }

    #[tokio::test]
    async fn test_concurrent_commands() {
        let transport = MockTransport::with_auto_respond();
        let handle = tokio::runtime::Handle::current();
        let mut camera =
            Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport.clone(), handle);

        camera
            .initialize_socket_manager()
            .expect("Failed to initialize socket manager");

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send multiple commands concurrently
        let camera_clone1 = camera.clone();
        let camera_clone2 = camera.clone();

        let (r1, r2) = tokio::join!(camera_clone1.zoom_in(), camera_clone2.zoom_out(),);

        assert!(r1.is_ok(), "First command should succeed");
        assert!(r2.is_ok(), "Second command should succeed");

        // Verify both commands were sent
        let sent_commands = transport.get_sent_commands();
        assert!(sent_commands.len() >= 2, "Both commands should be sent");
    }

    #[tokio::test]
    async fn test_command_without_socket_manager() {
        let transport = MockTransport::new();
        transport.add_response(Ok(Bytes::from(vec![0x90, 0x41, 0xFF]))); // ACK
        transport.add_response(Ok(Bytes::from(vec![0x90, 0x51, 0xFF]))); // Completion

        let handle = tokio::runtime::Handle::current();
        let camera =
            Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport.clone(), handle);

        // Don't initialize socket manager - commands should still work via direct transport
        let result = camera.power_on().await;
        assert!(result.is_ok(), "Command should work without socket manager");

        let sent_commands = transport.get_sent_commands();
        assert_eq!(sent_commands.len(), 1, "Command should be sent directly");
    }

    #[tokio::test]
    async fn test_socket_manager_timeout_handling() {
        let transport = MockTransport::new(); // No auto-respond
        let handle = tokio::runtime::Handle::current();
        let mut camera =
            Camera::with_profile_and_spawner(CameraModel::PTZOpticsG2, transport, handle);

        camera
            .initialize_socket_manager()
            .expect("Failed to initialize socket manager");

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
