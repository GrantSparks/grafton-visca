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

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use std::{io, time::Duration};

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use grafton_visca::{
    command::response::Response,
    Command, Error,
};

#[cfg(feature = "blocking-client")]
use grafton_visca::transport::BlockingTransport;

#[cfg(feature = "async-client")]
use grafton_visca::transport::{Transport, TransportFuture};

/// Trait representing a serial port for dependency injection
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
pub trait SerialPort: Send + Sync + std::fmt::Debug {
    fn write(&mut self, data: &[u8]) -> io::Result<usize>;
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()>;
}

/// Mock serial port for demonstration purposes
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
#[derive(Debug)]
struct MockSerialPort {
    timeout: Duration,
}

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
impl SerialPort for MockSerialPort {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        println!("Serial write: {:02X?}", data);
        Ok(data.len())
    }

    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        // Simulate a simple ACK response
        let response = [0x90, 0x40, 0xFF];
        let len = response.len().min(buffer.len());
        buffer[..len].copy_from_slice(&response[..len]);
        println!("Serial read: {:02X?}", &buffer[..len]);
        Ok(len)
    }

    fn set_timeout(&mut self, timeout: Duration) -> io::Result<()> {
        self.timeout = timeout;
        println!("Serial timeout set to: {:?}", timeout);
        Ok(())
    }
}

/// Serial transport for VISCA communication
#[cfg(feature = "blocking-client")]
#[derive(Debug)]
pub struct SerialTransport {
    port: Box<dyn SerialPort>,
    camera_address: u8,
    timeout: Duration,
}

#[cfg(feature = "blocking-client")]
impl SerialTransport {
    /// Creates a new serial transport
    ///
    /// # Arguments
    /// * `port` - The serial port to use
    /// * `camera_address` - VISCA address of the camera (1-7)
    ///
    /// # Errors
    /// Returns an error if the port cannot be configured
    pub fn new(mut port: Box<dyn SerialPort>, camera_address: u8) -> io::Result<Self> {
        // Set a reasonable timeout for serial operations
        port.set_timeout(Duration::from_millis(500))?;

        Ok(Self {
            port,
            camera_address,
            timeout: Duration::from_millis(500),
        })
    }

    /// Creates a new serial transport with custom timeout
    pub fn with_timeout(
        mut port: Box<dyn SerialPort>,
        camera_address: u8,
        timeout: Duration,
    ) -> io::Result<Self> {
        port.set_timeout(timeout)?;

        Ok(Self {
            port,
            camera_address,
            timeout,
        })
    }
}

#[cfg(feature = "blocking-client")]
impl BlockingTransport for SerialTransport {
    fn send_command_blocking(&mut self, command: &dyn Command) -> Result<Response, Error> {
        let mut bytes = command.to_bytes()?;

        // Set the camera address in the command
        if !bytes.is_empty() && bytes[0] == 0x81 {
            bytes[0] = 0x80 | self.camera_address;
        }

        // Send command
        self.port.write_all(&bytes).map_err(Error::Io)?;

        // Read response
        let mut buffer = [0u8; 256];
        let mut response = Vec::new();

        loop {
            match self.port.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    for &byte in &buffer[..n] {
                        response.push(byte);
                        // Check for end of VISCA frame
                        if byte == 0xFF && response.len() >= 3 && response[0] == 0x90 {
                            // Parse the response
                            let response_type = response[1] & 0xF0;
                            return match response_type {
                                0x40 => Ok(Response::Ack),
                                0x50 => {
                                    if response.len() == 3 {
                                        Ok(Response::Completion)
                                    } else {
                                        // For demo purposes, just return completion for data responses
                                        // In a real implementation, you'd parse the specific response type
                                        Ok(Response::Completion)
                                    }
                                }
                                0x60 => {
                                    if response.len() >= 4 {
                                        let error = Error::from_code(response[2]);
                                        Ok(Response::Error(error))
                                    } else {
                                        Err(Error::InvalidResponseFormat)
                                    }
                                }
                                _ => Err(Error::InvalidResponseFormat),
                            };
                        }
                    }
                }
                Err(e) => return Err(Error::Io(e)),
            }
        }

        Err(Error::CommandTimeout {
            duration: self.timeout,
            command: "serial_command".to_string(),
        })
    }
}

/// Async serial transport for VISCA communication
#[cfg(feature = "async-client")]
#[derive(Debug)]
pub struct AsyncSerialTransport {
    port: Box<dyn SerialPort>,
    camera_address: u8,
}

#[cfg(feature = "async-client")]
impl AsyncSerialTransport {
    /// Creates a new async serial transport
    pub fn new(mut port: Box<dyn SerialPort>, camera_address: u8) -> io::Result<Self> {
        port.set_timeout(Duration::from_millis(500))?;

        Ok(Self {
            port,
            camera_address,
        })
    }
}

#[cfg(feature = "async-client")]
impl Transport for AsyncSerialTransport {
    fn send_command<'a>(&'a mut self, command: &'a dyn Command) -> TransportFuture<'a, Response> {
        Box::pin(async move {
            let mut bytes = command.to_bytes()?;

            // Set the camera address in the command
            if !bytes.is_empty() && bytes[0] == 0x81 {
                bytes[0] = 0x80 | self.camera_address;
            }

            // Send command
            self.port.write_all(&bytes).map_err(Error::Io)?;

            // Read response (simplified for demo)
            let mut buffer = [0u8; 256];
            let response = match self.port.read(&mut buffer) {
                Ok(n) if n >= 3 => {
                    let data = &buffer[..n];
                    if data[0] == 0x90 && data[n - 1] == 0xFF {
                        match data[1] & 0xF0 {
                            0x40 => Response::Ack,
                            0x50 => Response::Completion,
                            0x60 => {
                                if n >= 4 {
                                    Response::Error(Error::from_code(data[2]))
                                } else {
                                    return Err(Error::InvalidResponseFormat);
                                }
                            }
                            _ => return Err(Error::InvalidResponseFormat),
                        }
                    } else {
                        return Err(Error::InvalidResponseFormat);
                    }
                }
                Ok(_) => return Err(Error::InvalidResponseFormat),
                Err(e) => return Err(Error::Io(e)),
            };

            Ok(response)
        })
    }
}

// Helper trait extension for write_all functionality
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
trait WriteAll {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()>;
}

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
impl<T: SerialPort + ?Sized> WriteAll for T {
    fn write_all(&mut self, mut data: &[u8]) -> io::Result<()> {
        while !data.is_empty() {
            match self.write(data) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::WriteZero,
                        "failed to write whole buffer",
                    ))
                }
                Ok(n) => data = &data[n..],
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
fn main() {
    eprintln!("This example requires either the 'blocking-client' or 'async-client' feature to be enabled.");
    eprintln!("Run with: cargo run --example serial_transport --features blocking-client");
    eprintln!("Or:       cargo run --example serial_transport --features async-client");
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Serial Transport Example ===\n");

    // Create a mock serial port for demonstration
    let mock_port = Box::new(MockSerialPort {
        timeout: Duration::from_millis(500),
    });

    // Create transport
    let _transport = SerialTransport::new(mock_port, 1)?;

    println!("Created serial transport for camera address 1");
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
    println!("// Create transport");
    println!("let transport = SerialTransport::new(port, 1)?; // Camera address 1");
    println!();
    println!("// Use with camera");
    println!("let camera = Camera::<PTZOpticsG2>::new(transport);");
    println!("camera.power_on().await?;");
    println!("```");
    println!();
    println!("For more details, see the source code of this example.");
    println!("\n=== Serial Transport Example Complete ===");

    Ok(())
}

#[cfg(all(feature = "async-client", not(feature = "blocking-client")))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Async Serial Transport Example ===\n");

    // Create a mock serial port for demonstration
    let mock_port = Box::new(MockSerialPort {
        timeout: Duration::from_millis(500),
    });

    // Create transport
    let _transport = AsyncSerialTransport::new(mock_port, 1)?;

    println!("Created async serial transport for camera address 1");
    println!("This demonstrates the async transport interface.");
    println!("\n=== Async Serial Transport Example Complete ===");

    Ok(())
}
