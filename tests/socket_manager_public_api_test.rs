//! Tests for socket manager functionality through the public Camera API

#[cfg(feature = "tokio")]
mod tokio_tests {
    use bytes::Bytes;
    use grafton_visca::r#async::prelude::*;
    use grafton_visca::transport::Transport;
    use grafton_visca::{
        camera::profiles::PTZOpticsG2, r#async, Camera, Error, PanTiltDirection, PresetNumber,
    };
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum SocketStatus {
        Free,
        WaitingForAck,
        WaitingForCompletion,
    }

    #[derive(Debug, Clone)]
    struct SocketInfo {
        status: SocketStatus,
        command_index: Option<usize>,
    }

    /// Mock transport for testing socket manager behavior
    #[derive(Debug, Clone)]
    struct MockTransport {
        sent_commands: Arc<Mutex<Vec<Vec<u8>>>>,
        responses: Arc<Mutex<VecDeque<Result<Bytes, Error>>>>,
        auto_respond: bool,
        // Stateful socket tracking
        socket_states: Arc<Mutex<[SocketInfo; 2]>>,
        pending_responses: Arc<Mutex<VecDeque<(usize, Bytes)>>>, // (command_index, response)
    }

    impl MockTransport {
        fn new() -> Self {
            Self {
                sent_commands: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(VecDeque::new())),
                auto_respond: false,
                socket_states: Arc::new(Mutex::new([
                    SocketInfo {
                        status: SocketStatus::Free,
                        command_index: None,
                    },
                    SocketInfo {
                        status: SocketStatus::Free,
                        command_index: None,
                    },
                ])),
                pending_responses: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        fn with_auto_respond() -> Self {
            Self {
                sent_commands: Arc::new(Mutex::new(Vec::new())),
                responses: Arc::new(Mutex::new(VecDeque::new())),
                auto_respond: true,
                socket_states: Arc::new(Mutex::new([
                    SocketInfo {
                        status: SocketStatus::Free,
                        command_index: None,
                    },
                    SocketInfo {
                        status: SocketStatus::Free,
                        command_index: None,
                    },
                ])),
                pending_responses: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        fn get_sent_commands(&self) -> Vec<Vec<u8>> {
            self.sent_commands.lock().unwrap().clone()
        }

        fn add_response(&self, response: Result<Bytes, Error>) {
            self.responses.lock().unwrap().push_back(response);
        }

        fn generate_visca_ack_completion(&self) -> (Bytes, Bytes) {
            // Always use Socket1 for responses - this simulates the camera
            // using Socket1 for all sequential commands
            let socket_byte = 0x90;

            // Generate ACK response
            let ack = Bytes::from(vec![socket_byte, 0x41, 0xFF]);
            // Generate Completion response
            let completion = Bytes::from(vec![socket_byte, 0x51, 0xFF]);
            (ack, completion)
        }

        fn with_concurrent_response() -> Self {
            let mut transport = Self::new();
            transport.auto_respond = true;
            transport
        }

        fn assign_socket_for_command(&self, command_index: usize) -> Option<usize> {
            let mut states = self.socket_states.lock().unwrap();

            // Find a free socket
            for (socket_idx, socket) in states.iter_mut().enumerate() {
                if socket.status == SocketStatus::Free {
                    socket.status = SocketStatus::WaitingForAck;
                    socket.command_index = Some(command_index);
                    return Some(socket_idx);
                }
            }
            None
        }

        fn generate_concurrent_responses(&self, command_index: usize) {
            if let Some(socket_idx) = self.assign_socket_for_command(command_index) {
                let socket_num = socket_idx + 1; // Socket1 = 1, Socket2 = 2

                // Generate ACK
                let ack = Bytes::from(vec![0x90, 0x40 | socket_num as u8, 0xFF]);
                self.pending_responses
                    .lock()
                    .unwrap()
                    .push_back((command_index, ack));

                // Generate Completion (will be sent later)
                let completion = Bytes::from(vec![0x90, 0x50 | socket_num as u8, 0xFF]);
                self.pending_responses
                    .lock()
                    .unwrap()
                    .push_back((command_index, completion));
            }
        }

        fn get_next_response_static(
            pending_responses: Arc<Mutex<VecDeque<(usize, Bytes)>>>,
            socket_states: Arc<Mutex<[SocketInfo; 2]>>,
        ) -> Option<Bytes> {
            let mut pending = pending_responses.lock().unwrap();
            let mut states = socket_states.lock().unwrap();

            // First, try to send any pending ACKs
            for i in 0..pending.len() {
                let (_cmd_idx, response) = &pending[i];
                let response_type = response[1] & 0xF0;
                let socket_num = (response[1] & 0x0F) as usize;

                if socket_num > 0 && socket_num <= 2 {
                    let socket_idx = socket_num - 1;

                    if response_type == 0x40
                        && states[socket_idx].status == SocketStatus::WaitingForAck
                    {
                        // Send ACK
                        states[socket_idx].status = SocketStatus::WaitingForCompletion;
                        return Some(pending.remove(i).unwrap().1);
                    }
                }
            }

            // Then, try to send completions
            for i in 0..pending.len() {
                let (_cmd_idx, response) = &pending[i];
                let response_type = response[1] & 0xF0;
                let socket_num = (response[1] & 0x0F) as usize;

                if socket_num > 0 && socket_num <= 2 {
                    let socket_idx = socket_num - 1;

                    if response_type == 0x50
                        && states[socket_idx].status == SocketStatus::WaitingForCompletion
                    {
                        // Send Completion and free socket
                        states[socket_idx].status = SocketStatus::Free;
                        states[socket_idx].command_index = None;
                        return Some(pending.remove(i).unwrap().1);
                    }
                }
            }

            None
        }
    }

    use async_trait::async_trait;

    impl grafton_visca::transport::BlockingTransport for MockTransport {
        fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
            // For tests, delegate to async version
            futures::executor::block_on(async { self.send(bytes).await })
        }

        fn recv_blocking(&self) -> Result<bytes::Bytes, Error> {
            // For tests, delegate to async version
            futures::executor::block_on(async { self.recv().await })
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
            let command_index = {
                let mut commands = self.sent_commands.lock().unwrap();
                commands.push(bytes.to_vec());
                commands.len() - 1
            };

            // If auto-respond is enabled, generate responses based on socket availability
            if self.auto_respond {
                // Check if this is a VISCA command (not inquiry)
                if bytes.len() >= 3 && bytes[1] == 0x01 {
                    self.generate_concurrent_responses(command_index);
                } else if bytes.len() >= 3 && bytes[1] == 0x09 {
                    // For inquiries, generate immediate response
                    let response = Bytes::from(vec![0x90, 0x50, 0x02, 0xFF]); // Simple inquiry response
                    self.responses.lock().unwrap().push_back(Ok(response));
                } else {
                    // For other commands, use simple ACK/Completion on Socket1
                    let (ack, completion) = self.generate_visca_ack_completion();
                    let mut responses = self.responses.lock().unwrap();
                    responses.push_back(Ok(ack));
                    responses.push_back(Ok(completion));
                }
            }

            Ok(())
        }

        async fn recv(&self) -> Result<Bytes, Error> {
            let responses = self.responses.clone();
            let pending_responses = self.pending_responses.clone();
            let socket_states = self.socket_states.clone();
            let auto_respond = self.auto_respond;

            // Simulate a short delay before response
            tokio::time::sleep(Duration::from_millis(10)).await;

            // First check if we have any pending concurrent responses
            if auto_respond {
                let response = MockTransport::get_next_response_static(
                    pending_responses.clone(),
                    socket_states.clone(),
                );
                if let Some(resp) = response {
                    return Ok(resp);
                }
            }

            // Otherwise, check the manual response queue
            let mut responses = responses.lock().unwrap();
            responses.pop_front().unwrap_or_else(|| {
                Err(Error::TransportError(std::borrow::Cow::Borrowed(
                    "No response available",
                )))
            })
        }
    }

    #[tokio::test]
    async fn test_socket_manager_initialization() {
        let transport = MockTransport::new();
        let handle = tokio::runtime::Handle::current();
        let mut inner_camera = Camera::<PTZOpticsG2, _>::new_async_with_spawner(transport, handle);

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

    #[tokio::test]
    async fn test_socket_manager_with_commands() {
        let transport = MockTransport::with_auto_respond();
        let handle = tokio::runtime::Handle::current();
        let mut inner_camera =
            Camera::<PTZOpticsG2, _>::new_async_with_spawner(transport.clone(), handle);

        // Initialize socket manager
        inner_camera
            .initialize_socket_manager()
            .expect("Failed to initialize socket manager");

        let camera = r#async::Camera::new(inner_camera);

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
        let transport = MockTransport::with_concurrent_response();
        let handle = tokio::runtime::Handle::current();
        let mut inner_camera =
            Camera::<PTZOpticsG2, _>::new_async_with_spawner(transport.clone(), handle);

        inner_camera
            .initialize_socket_manager()
            .expect("Failed to initialize socket manager");

        let camera = r#async::Camera::new(inner_camera);

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send multiple commands concurrently
        let (r1, r2) = tokio::join!(camera.zoom_in(), camera.zoom_out(),);

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

    #[tokio::test]
    async fn test_command_without_socket_manager() {
        let transport = MockTransport::new();
        transport.add_response(Ok(Bytes::from(vec![0x90, 0x41, 0xFF]))); // ACK
        transport.add_response(Ok(Bytes::from(vec![0x90, 0x51, 0xFF]))); // Completion

        let handle = tokio::runtime::Handle::current();
        let inner_camera = Camera::<PTZOpticsG2, _>::new_async_with_spawner(transport.clone(), handle);
        let camera = r#async::Camera::new(inner_camera);

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
        let mut inner_camera = Camera::<PTZOpticsG2, _>::new_async_with_spawner(transport, handle);

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
