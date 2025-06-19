//! Example demonstrating how to implement a custom transport
//!
//! This example shows how to create your own transport implementation
//! that can be used with the grafton-visca library. We'll create a
//! simple in-memory transport for testing purposes.

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the async-client feature.");
    eprintln!("Run with: cargo run --example custom_transport_example --features async-client");
}

#[cfg(feature = "async-client")]
mod async_example {
    use grafton_visca::{
        camera::{Camera, PTZOpticsG2},
        command::response::Response,
        transport::{Transport, TransportFuture},
        Command, Error,
    };

    #[cfg(feature = "blocking-client")]
    use grafton_visca::transport::BlockingTransport;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    /// A mock transport that simulates camera responses for testing
    ///
    /// This transport doesn't actually communicate with a camera but instead
    /// returns predefined responses. This is useful for:
    /// - Unit testing
    /// - Development without a physical camera
    /// - Demonstrating the transport interface
    #[derive(Debug, Clone)]
    struct MockTransport {
        /// Queue of responses to return
        response_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
        /// Whether to simulate ACK responses
        send_acks: bool,
    }

    impl MockTransport {
        /// Create a new mock transport
        fn new(send_acks: bool) -> Self {
            Self {
                response_queue: Arc::new(Mutex::new(VecDeque::new())),
                send_acks,
            }
        }

        /// Add a response to the queue
        fn queue_response(&self, response: Vec<u8>) {
            if let Ok(mut queue) = self.response_queue.lock() {
                queue.push_back(response);
            }
        }

        /// Add standard ACK and completion responses for a command
        fn queue_standard_response(&self) {
            if self.send_acks {
                // ACK response
                self.queue_response(vec![0x90, 0x40, 0xFF]);
            }
            // Completion response
            self.queue_response(vec![0x90, 0x50, 0xFF]);
        }

        /// Add an inquiry response with data
        fn queue_inquiry_response(&self, data: Vec<u8>) {
            if self.send_acks {
                // ACK response
                self.queue_response(vec![0x90, 0x40, 0xFF]);
            }
            // Completion with data
            let mut response = vec![0x90, 0x50];
            response.extend(data);
            response.push(0xFF);
            self.queue_response(response);
        }
    }

    /// Implement the blocking transport trait
    #[cfg(feature = "blocking-client")]
    impl BlockingTransport for MockTransport {
        fn send_command_blocking(&mut self, command: &dyn Command) -> Result<Response, Error> {
            // Log the command being sent
            let bytes = command.to_bytes()?;
            println!("Mock transport sending: {:02X?}", bytes);

            // Process all responses until we get a completion
            let final_response = loop {
                let response_bytes = self
                    .response_queue
                    .lock()
                    .map_err(|_| Error::InvalidResponse {
                        expected: "Lock".to_string(),
                        actual: vec![],
                    })?
                    .pop_front()
                    .ok_or(Error::InvalidResponse {
                        expected: "Response in queue".to_string(),
                        actual: vec![],
                    })?;

                println!("Mock transport received: {:02X?}", response_bytes);

                // Parse the response type
                if response_bytes.len() >= 3 && response_bytes[0] == 0x90 {
                    match response_bytes[1] & 0xF0 {
                        0x40 => {
                            // ACK - continue waiting
                            continue;
                        }
                        0x50 => {
                            // Completion
                            if response_bytes.len() == 3 {
                                break Response::Completion;
                            } else {
                                // Completion with data - would need proper parsing
                                // For this example, we'll just return completion
                                break Response::Completion;
                            }
                        }
                        0x60 => {
                            // Error
                            if response_bytes.len() >= 4 {
                                let error = Error::from_code(response_bytes[2]);
                                break Response::Error(error);
                            }
                        }
                        _ => {}
                    }
                }

                return Err(Error::InvalidResponseFormat);
            };

            Ok(final_response)
        }
    }

    /// Example of an async mock transport
    #[derive(Debug, Clone)]
    struct AsyncMockTransport {
        inner: MockTransport,
    }

    impl AsyncMockTransport {
        fn new(send_acks: bool) -> Self {
            Self {
                inner: MockTransport::new(send_acks),
            }
        }

        #[allow(dead_code)]
        fn queue_response(&self, response: Vec<u8>) {
            self.inner.queue_response(response);
        }

        fn queue_standard_response(&self) {
            self.inner.queue_standard_response();
        }
    }

    impl Transport for AsyncMockTransport {
        fn send_command<'a>(
            &'a mut self,
            command: &'a dyn Command,
        ) -> TransportFuture<'a, Response> {
            Box::pin(async move {
                // Simply delegate to the blocking implementation
                self.inner.send_command_blocking(command)
            })
        }
    }

    /// Example of a logging transport wrapper
    ///
    /// This demonstrates how to wrap an existing transport to add functionality
    struct LoggingTransport<T: Transport> {
        inner: T,
        log_prefix: String,
    }

    impl<T: Transport> LoggingTransport<T> {
        fn new(inner: T, log_prefix: String) -> Self {
            Self { inner, log_prefix }
        }
    }

    impl<T: Transport> Transport for LoggingTransport<T> {
        fn send_command<'a>(
            &'a mut self,
            command: &'a dyn Command,
        ) -> TransportFuture<'a, Response> {
            Box::pin(async move {
                let bytes = command.to_bytes()?;
                println!("{}: Sending command: {:02X?}", self.log_prefix, bytes);

                let response = self.inner.send_command(command).await?;
                println!("{}: Received response: {:?}", self.log_prefix, response);

                Ok(response)
            })
        }
    }

    #[tokio::main]
    pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
        println!("=== Custom Transport Example ===\n");

        // Example 1: Using a mock transport for testing
        println!("1. Mock Transport Example");
        println!("-------------------------");

        let mock_transport = AsyncMockTransport::new(true);

        // Queue some responses for our commands
        mock_transport.queue_standard_response(); // For power on
        mock_transport.queue_standard_response(); // For home

        // Example of queuing an inquiry response (for demonstration)
        // This would be used for commands that expect data in the response
        mock_transport
            .inner
            .queue_inquiry_response(vec![0x90, 0x50, 0x02, 0x03, 0x04, 0xFF]);

        let camera = Camera::<PTZOpticsG2>::new(mock_transport);

        // These commands will use our queued responses
        camera.power_on().await?;
        println!("Power on command sent (mock)");

        camera.home().await?;
        println!("Home command sent (mock)\n");

        // Example 2: Using a logging wrapper
        println!("2. Logging Transport Wrapper Example");
        println!("------------------------------------");

        let base_transport = AsyncMockTransport::new(false);
        base_transport.queue_standard_response(); // For zoom in

        let logging_transport = LoggingTransport::new(base_transport, "[CAMERA-01]".to_string());
        let camera_with_logging = Camera::<PTZOpticsG2>::new(logging_transport);

        camera_with_logging.zoom_in().await?;

        // Example 3: Custom transport with state tracking
        println!("\n3. State-Tracking Transport Example");
        println!("-----------------------------------");

        #[derive(Debug)]
        struct StateTrackingTransport {
            inner: AsyncMockTransport,
            command_count: Arc<Mutex<usize>>,
        }

        impl StateTrackingTransport {
            fn new() -> Self {
                Self {
                    inner: AsyncMockTransport::new(false),
                    command_count: Arc::new(Mutex::new(0)),
                }
            }
        }

        impl Transport for StateTrackingTransport {
            fn send_command<'a>(
                &'a mut self,
                command: &'a dyn Command,
            ) -> TransportFuture<'a, Response> {
                Box::pin(async move {
                    // Increment command count
                    if let Ok(mut count) = self.command_count.lock() {
                        *count += 1;
                    }

                    // Delegate to inner transport
                    self.inner.send_command(command).await
                })
            }
        }

        let tracking_transport = StateTrackingTransport::new();
        tracking_transport.inner.queue_standard_response();
        tracking_transport.inner.queue_standard_response();

        // Keep a reference to the command count Arc so we can check it later
        let command_count_ref = tracking_transport.command_count.clone();

        let tracking_camera = Camera::<PTZOpticsG2>::new(tracking_transport);

        // Send some commands
        tracking_camera.zoom_stop().await?;
        tracking_camera.zoom_in().await?;

        // Check the command count
        let count = command_count_ref.lock().unwrap();
        println!("Commands sent: {}", *count);

        println!("\n=== Custom Transport Example Complete ===");

        Ok(())
    }
}

#[cfg(feature = "async-client")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    async_example::main()
}
