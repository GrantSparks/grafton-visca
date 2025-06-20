//! Example demonstrating how to implement a custom transport
//!
//! This example shows how to create your own transport implementation
//! that can be used with the grafton-visca library. We'll create a
//! simple in-memory transport for testing purposes.

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the async-client feature.");
    eprintln!("Run with: cargo run --example custom_transport_example --features async");
}

#[cfg(feature = "async")]
mod async_example {
    use grafton_visca::{
        camera::{profiles::PTZOpticsG2, Camera},
        transport::{RawTransport, TransportFuture, ViscaTransport},
    };
    use std::collections::VecDeque;
    use std::fmt;
    use std::sync::{Arc, Mutex};

    /// A mock raw transport that simulates basic I/O for testing
    ///
    /// This transport doesn't actually communicate with a camera but instead
    /// returns predefined responses. This demonstrates the RawTransport interface.
    #[derive(Clone)]
    struct MockRawTransport {
        /// Queue of responses to return
        response_queue: Arc<Mutex<VecDeque<Vec<u8>>>>,
        /// Whether this transport is "connected"
        connected: bool,
    }

    impl fmt::Debug for MockRawTransport {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("MockRawTransport")
                .field("connected", &self.connected)
                .finish()
        }
    }

    impl MockRawTransport {
        /// Create a new mock transport
        fn new() -> Self {
            Self {
                response_queue: Arc::new(Mutex::new(VecDeque::new())),
                connected: true,
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
            // ACK response
            self.queue_response(vec![0x90, 0x40, 0xFF]);
            // Completion response
            self.queue_response(vec![0x90, 0x50, 0xFF]);
        }
    }

    /// Implement the RawTransport trait for basic I/O
    impl RawTransport for MockRawTransport {
        fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
            Box::pin(async move {
                println!("Mock transport sending: {:02X?}", data);
                Ok(())
            })
        }

        fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
            Box::pin(async move {
                let response_bytes = self
                    .response_queue
                    .lock()
                    .map_err(|_| std::io::Error::other("Failed to lock response queue"))?
                    .pop_front()
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::UnexpectedEof,
                            "No response in queue",
                        )
                    })?;

                println!("Mock transport received: {:02X?}", response_bytes);
                Ok(response_bytes)
            })
        }

        fn is_connected(&self) -> bool {
            self.connected
        }

        fn description(&self) -> &str {
            "Mock Raw Transport"
        }
    }

    /// Example wrapper that tracks command count
    struct CommandCountingTransport {
        inner: MockRawTransport,
        command_count: Arc<Mutex<usize>>,
    }

    impl CommandCountingTransport {
        fn new() -> Self {
            Self {
                inner: MockRawTransport::new(),
                command_count: Arc::new(Mutex::new(0)),
            }
        }

        fn queue_standard_response(&self) {
            self.inner.queue_standard_response();
        }

        fn command_count(&self) -> usize {
            *self.command_count.lock().unwrap()
        }
    }

    impl fmt::Debug for CommandCountingTransport {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("CommandCountingTransport")
                .field("command_count", &self.command_count())
                .finish()
        }
    }

    impl RawTransport for CommandCountingTransport {
        fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
            Box::pin(async move {
                // Increment command count
                if let Ok(mut count) = self.command_count.lock() {
                    *count += 1;
                }
                // Delegate to inner transport
                self.inner.send(data).await
            })
        }

        fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
            Box::pin(async move { self.inner.receive().await })
        }

        fn is_connected(&self) -> bool {
            self.inner.is_connected()
        }

        fn description(&self) -> &str {
            "Command Counting Mock Transport"
        }
    }

    /// Example of a logging raw transport wrapper
    struct LoggingRawTransport<T: RawTransport> {
        inner: T,
        log_prefix: String,
    }

    impl<T: RawTransport> LoggingRawTransport<T> {
        fn new(inner: T, log_prefix: String) -> Self {
            Self { inner, log_prefix }
        }
    }

    impl<T: RawTransport> fmt::Debug for LoggingRawTransport<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("LoggingRawTransport")
                .field("prefix", &self.log_prefix)
                .finish()
        }
    }

    impl<T: RawTransport> RawTransport for LoggingRawTransport<T> {
        fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
            Box::pin(async move {
                println!("{}: Sending data: {:02X?}", self.log_prefix, data);
                self.inner.send(data).await
            })
        }

        fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
            Box::pin(async move {
                let data = self.inner.receive().await?;
                println!("{}: Received data: {:02X?}", self.log_prefix, data);
                Ok(data)
            })
        }

        fn is_connected(&self) -> bool {
            self.inner.is_connected()
        }

        fn description(&self) -> &str {
            "Logging Raw Transport"
        }
    }

    pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
        println!("=== Custom RawTransport Example ===\n");

        // Example 1: Basic mock transport with ViscaTransport wrapper
        println!("1. Mock RawTransport Example");
        println!("----------------------------");

        let mock_raw = MockRawTransport::new();
        mock_raw.queue_standard_response(); // For power on
        mock_raw.queue_standard_response(); // For home

        let visca_transport = ViscaTransport::new(mock_raw);
        let camera = Camera::<PTZOpticsG2>::new(visca_transport);

        camera.power_on().await?;
        println!("Power on command sent (mock)");

        camera.home().await?;
        println!("Home command sent (mock)\n");

        // Example 2: Logging wrapper around raw transport
        println!("2. Logging RawTransport Wrapper Example");
        println!("----------------------------------------");

        let base_raw = MockRawTransport::new();
        base_raw.queue_standard_response(); // For zoom in

        let logging_raw = LoggingRawTransport::new(base_raw, "[CAMERA-01]".to_string());
        let visca_with_logging = ViscaTransport::new(logging_raw);
        let camera_with_logging = Camera::<PTZOpticsG2>::new(visca_with_logging);

        camera_with_logging.zoom_in().await?;
        println!("Zoom command sent with logging\n");

        // Example 3: Command counting transport
        println!("3. Command Counting Transport Example");
        println!("-------------------------------------");

        let counting_transport = CommandCountingTransport::new();
        counting_transport.queue_standard_response(); // For zoom stop
        counting_transport.queue_standard_response(); // For zoom out

        let visca_counting = ViscaTransport::new(counting_transport);
        let counting_camera = Camera::<PTZOpticsG2>::new(visca_counting);

        // Send some commands
        counting_camera.zoom_stop().await?;
        counting_camera.zoom_out().await?;

        println!("Commands sent: 2 (expected)");

        println!("\n=== Benefits of New RawTransport API ===");
        println!("• Simple 4-method interface: send, receive, is_connected, description");
        println!("• All VISCA protocol logic handled by ViscaTransport wrapper");
        println!("• Easy to implement custom transports (serial, USB, network, etc.)");
        println!("• Clean separation between I/O and protocol logic");
        println!("• Composable with wrapper patterns for logging, retries, etc.");

        println!("\n=== Custom Transport Example Complete ===");

        Ok(())
    }
}

#[cfg(feature = "async")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move { async_example::main().await })
}
