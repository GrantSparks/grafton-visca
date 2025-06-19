//! Example demonstrating how to implement a serial port transport for VISCA
//!
//! This example shows how to create a transport that communicates with VISCA
//! cameras over RS-232/RS-422 serial connections. This is useful for:
//! - Older VISCA cameras that don't support IP
//! - Direct serial connections for reliability
//! - Daisy-chained camera setups
//!
//! Note: This is a demonstration of the transport interface. For a production
//! implementation, you would need to add the `serialport` crate as a dependency.

use grafton_visca::{
    command::response::{Response, ResponseType},
    transport::{BlockingTransport, Transport, TransportFuture},
    Command, Error,
};
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Mock serial port for demonstration
///
/// In a real implementation, you would use the `serialport` crate:
/// ```ignore
/// use serialport::{SerialPort, SerialPortSettings};
/// ```
pub trait SerialPort: Read + Write + Send {
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()>;
}

/// Mock implementation of a serial port
struct MockSerialPort {
    read_buffer: Vec<u8>,
    write_buffer: Vec<u8>,
    timeout: Duration,
}

impl MockSerialPort {
    fn new() -> Self {
        Self {
            read_buffer: Vec::new(),
            write_buffer: Vec::new(),
            timeout: Duration::from_secs(1),
        }
    }

    /// Simulate a response being available
    fn simulate_response(&mut self, data: Vec<u8>) {
        self.read_buffer.extend(data);
    }
}

impl Read for MockSerialPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = std::cmp::min(buf.len(), self.read_buffer.len());
        if len == 0 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "No data available",
            ));
        }

        buf[..len].copy_from_slice(&self.read_buffer[..len]);
        self.read_buffer.drain(0..len);
        Ok(len)
    }
}

impl Write for MockSerialPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.write_buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl SerialPort for MockSerialPort {
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.timeout = timeout;
        Ok(())
    }
}

/// Connection statistics for tracking transport performance
#[derive(Debug, Default)]
pub struct ConnectionStats {
    commands_sent: usize,
    responses_received: usize,
    errors: usize,
}

impl ConnectionStats {
    fn new() -> Self {
        Self::default()
    }

    fn record_sent(&mut self, _bytes: usize) {
        self.commands_sent += 1;
    }

    fn record_received(&mut self, _bytes: usize) {
        self.responses_received += 1;
    }

    fn record_error(&mut self) {
        self.errors += 1;
    }
}

/// Blocking serial transport for VISCA communication
///
/// This transport handles:
/// - Serial port communication
/// - VISCA framing (commands end with 0xFF)
/// - Response correlation (serial VISCA doesn't have sockets)
pub struct SerialTransport {
    port: Box<dyn SerialPort>,
    stats: ConnectionStats,
    camera_address: u8,
}

impl std::fmt::Debug for SerialTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SerialTransport")
            .field("stats", &self.stats)
            .field("camera_address", &self.camera_address)
            .finish()
    }
}

impl SerialTransport {
    /// Create a new serial transport
    ///
    /// In a real implementation:
    /// ```ignore
    /// pub fn new(port_name: &str, baud_rate: u32, camera_address: u8) -> io::Result<Self> {
    ///     let port = serialport::new(port_name, baud_rate)
    ///         .timeout(Duration::from_secs(1))
    ///         .open()?;
    ///     Ok(Self {
    ///         port: Box::new(port),
    ///         stats: ConnectionStats::new(),
    ///         camera_address,
    ///     })
    /// }
    /// ```
    pub fn new(mut port: Box<dyn SerialPort>, camera_address: u8) -> io::Result<Self> {
        // Set a reasonable timeout for serial operations
        port.set_timeout(Duration::from_millis(500))?;

        Ok(Self {
            port,
            stats: ConnectionStats::new(),
            camera_address,
        })
    }

    /// Returns the connection statistics
    pub const fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Send a command over serial
    fn send_command_bytes(&mut self, command: &dyn Command) -> Result<(), Error> {
        let mut bytes = command.to_bytes()?;

        // Replace generic address with specific camera address
        if !bytes.is_empty() && bytes[0] == 0x81 {
            bytes[0] = 0x80 | self.camera_address;
        }

        log::debug!("Sending serial command: {:02X?}", bytes);

        self.port.write_all(&bytes).map_err(Error::Io)?;
        self.port.flush().map_err(Error::Io)?;
        self.stats.record_sent(bytes.len());

        Ok(())
    }

    /// Receive a response from serial
    fn receive_response(&mut self) -> Result<Vec<u8>, Error> {
        let mut buffer = [0u8; 256];
        let mut response = Vec::new();

        // Read until we get a complete VISCA frame (ends with 0xFF)
        loop {
            match self.port.read(&mut buffer) {
                Ok(0) => {
                    self.stats.record_error();
                    return Err(Error::Io(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Serial connection closed",
                    )));
                }
                Ok(size) => {
                    for &byte in &buffer[..size] {
                        response.push(byte);

                        // Check for end of VISCA frame
                        if byte == 0xFF && response.len() >= 3 && response[0] == 0x90 {
                            log::debug!("Received serial response: {:02X?}", response);
                            self.stats.record_received(response.len());
                            return Ok(response);
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if response.is_empty() {
                        self.stats.record_error();
                        return Err(Error::CommandTimeout {
                            duration: Duration::from_secs(1),
                            command: "receive_response".to_string(),
                        });
                    }
                    // Continue reading if we have partial data
                }
                Err(e) => {
                    self.stats.record_error();
                    return Err(Error::Io(e));
                }
            }
        }
    }

    /// Process a response
    fn process_response(&self, response: &[u8]) -> Result<Response, Error> {
        // Validate basic response format
        if response.len() < 3 || response[0] != 0x90 || response[response.len() - 1] != 0xFF {
            return Err(Error::InvalidResponseFormat);
        }

        let response_type = response[1] & 0xF0;

        match response_type {
            // ACK response (serial doesn't use sockets, so we ignore the socket bits)
            0x40 => {
                log::debug!("ACK received");
                Ok(Response::Ack)
            }

            // Completion response
            0x50 => {
                if response.len() == 3 {
                    log::debug!("Completion received");
                    Ok(Response::Completion)
                } else {
                    // Completion with data - for serial, we don't have response type info
                    // In a real implementation, you might track command types
                    log::debug!("Completion with data received");
                    Ok(Response::Completion)
                }
            }

            // Error response
            0x60 => {
                if response.len() >= 4 {
                    let error_code = response[2];
                    let error = grafton_visca::Error::from_code(error_code);
                    log::error!("Error response: {}", error);
                    Ok(Response::Error(error))
                } else {
                    Err(Error::InvalidResponseFormat)
                }
            }

            _ => {
                log::error!("Unknown response type: {:#02X}", response[1]);
                Err(Error::InvalidResponseFormat)
            }
        }
    }
}

impl BlockingTransport for SerialTransport {
    fn send_command_blocking(&mut self, command: &dyn Command) -> Result<Response, Error> {
        // Send command
        self.send_command_bytes(command)?;

        // Wait for responses
        let mut _received_ack = false;

        loop {
            let response_data = self.receive_response()?;
            let response = self.process_response(&response_data)?;

            match response {
                Response::Ack => {
                    _received_ack = true;
                    // Continue waiting for completion
                }
                Response::Completion | Response::Error(_) => {
                    return Ok(response);
                }
                // Other responses
                _ => {
                    return Ok(response);
                }
            }
        }
    }
}

/// Async serial transport wrapper
#[derive(Clone)]
pub struct AsyncSerialTransport {
    inner: Arc<Mutex<SerialTransport>>,
}

impl AsyncSerialTransport {
    /// Create a new async serial transport
    pub fn new(port: Box<dyn SerialPort>, camera_address: u8) -> io::Result<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(SerialTransport::new(port, camera_address)?)),
        })
    }
}

impl Transport for AsyncSerialTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, Response> {
        // Clone the command bytes to avoid lifetime issues
        let command_bytes = match command.to_bytes() {
            Ok(bytes) => bytes,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let response_type = command.response_type();
        let inner_clone = self.inner.clone();

        Box::pin(async move {
            // Use blocking task to avoid blocking the async runtime
            tokio::task::spawn_blocking(move || {
                let mut transport = inner_clone.lock().map_err(|_| Error::InvalidResponse {
                    expected: "Lock".to_string(),
                    actual: vec![],
                })?;

                // Create a minimal command that just returns the bytes we already have
                struct BytesCommand {
                    bytes: Vec<u8>,
                    response_type: Option<ResponseType>,
                }

                impl Command for BytesCommand {
                    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
                        Ok(self.bytes.clone())
                    }

                    fn response_type(&self) -> Option<ResponseType> {
                        self.response_type
                    }

                    fn command_category(&self) -> grafton_visca::timeout::CommandCategory {
                        grafton_visca::timeout::CommandCategory::Quick
                    }
                }

                let cmd = BytesCommand {
                    bytes: command_bytes,
                    response_type,
                };

                transport.send_command_blocking(&cmd)
            })
            .await
            .map_err(|_| Error::InvalidResponse {
                expected: "Task completion".to_string(),
                actual: vec![],
            })?
        })
    }
}

fn main() {
    println!("=== Serial Transport Example ===\n");

    println!("This example demonstrates how to implement a serial transport for VISCA.");
    println!("In a real implementation, you would:");
    println!("1. Add `serialport` as a dependency in Cargo.toml");
    println!("2. Use SerialPort::new() to open a real serial port");
    println!("3. Handle serial-specific settings (baud rate, parity, etc.)");
    println!("\nExample usage:");
    println!();
    println!("```rust");
    println!("// Open serial port");
    println!("let port = serialport::new(\"/dev/ttyUSB0\", 9600)");
    println!("    .timeout(Duration::from_secs(1))");
    println!("    .open()?;");
    println!();
    println!("// Create transport for camera address 1");
    println!("let transport = SerialTransport::new(Box::new(port), 1)?;");
    println!();
    println!("// Use with Camera API");
    println!("let camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(transport));");
    println!("```");
    println!();
    println!("Key differences from network transports:");
    println!("- No socket management (serial is point-to-point)");
    println!("- Camera addressing via command byte modification");
    println!("- Different timeout handling");
    println!("- May need to handle daisy-chaining");

    // Demonstrate with mock
    println!("\n--- Mock Demonstration ---");

    let mut mock_port = MockSerialPort::new();

    // Simulate some responses
    mock_port.simulate_response(vec![0x90, 0x41, 0xFF]); // ACK
    mock_port.simulate_response(vec![0x90, 0x51, 0xFF]); // Completion

    let mut transport = SerialTransport::new(Box::new(mock_port), 1).unwrap();

    // Mock command
    struct TestCommand;
    impl Command for TestCommand {
        fn to_bytes(&self) -> Result<Vec<u8>, Error> {
            Ok(vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF])
        }

        fn response_type(&self) -> Option<ResponseType> {
            None
        }

        fn command_category(&self) -> grafton_visca::timeout::CommandCategory {
            grafton_visca::timeout::CommandCategory::Quick
        }
    }

    match transport.send_command_blocking(&TestCommand) {
        Ok(response) => println!("Received response: {:?}", response),
        Err(e) => println!("Error: {}", e),
    }

    println!("\n=== Serial Transport Example Complete ===");
}
