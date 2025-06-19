//! Example of implementing a custom transport using the new RawTransport API.
//!
//! This shows how simple it is to create a new transport type - you only need
//! to implement the basic send/receive operations. All VISCA protocol logic
//! is handled by the library.

use std::{
    collections::VecDeque,
    pin::Pin,
    sync::{Arc, Mutex},
};

#[cfg(feature = "async-client")]
use grafton_visca::{
    command::zoom::ZoomCommand,
    transport::{core::RawTransport, TransportSession},
    Error,
};

/// A mock transport that simulates network communication with predefined responses.
/// This demonstrates how simple it is to implement a custom transport.
#[cfg(feature = "async-client")]
#[derive(Debug, Clone)]
pub struct MockRawTransport {
    /// Simulated response queue
    responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
    /// Description for logging
    description: String,
    /// Track sent commands for verification
    sent_commands: Arc<Mutex<Vec<Vec<u8>>>>,
}

#[cfg(feature = "async-client")]
impl MockRawTransport {
    /// Create a new mock transport with predefined responses.
    pub fn new() -> Self {
        let mut responses = VecDeque::new();

        // Add some typical VISCA responses
        responses.push_back(vec![0x90, 0x40, 0xFF]); // ACK for socket 0
        responses.push_back(vec![0x90, 0x50, 0xFF]); // Completion for socket 0

        Self {
            responses: Arc::new(Mutex::new(responses)),
            description: "Mock transport for testing".to_string(),
            sent_commands: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a response to the queue.
    pub fn add_response(&self, response: Vec<u8>) {
        if let Ok(mut responses) = self.responses.lock() {
            responses.push_back(response);
        }
    }

    /// Get all sent commands for verification.
    pub fn sent_commands(&self) -> Vec<Vec<u8>> {
        self.sent_commands.lock().unwrap().clone()
    }
}

#[cfg(feature = "async-client")]
impl RawTransport for MockRawTransport {
    fn send<'a>(
        &'a mut self,
        data: &'a [u8],
    ) -> Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>> {
        Box::pin(async move {
            // Store the sent command
            if let Ok(mut commands) = self.sent_commands.lock() {
                commands.push(data.to_vec());
            }

            println!("Mock transport sent: {:02X?}", data);
            Ok(())
        })
    }

    fn receive<'a>(
        &'a mut self,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + Send + 'a>> {
        Box::pin(async move {
            // Get next response from queue
            if let Ok(mut responses) = self.responses.lock() {
                if let Some(response) = responses.pop_front() {
                    println!("Mock transport received: {:02X?}", response);
                    return Ok(response);
                }
            }

            // No more responses - simulate timeout
            Err(Error::CommandTimeout {
                duration: std::time::Duration::from_millis(100),
                command: "mock_receive".to_string(),
            })
        })
    }

    fn is_connected(&self) -> bool {
        true // Mock is always "connected"
    }

    fn description(&self) -> &str {
        &self.description
    }
}

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature to be enabled.");
    eprintln!("Run with: cargo run --example custom_transport_simple --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Custom Transport Implementation Demo ===\n");

    // Create a mock transport
    let mock_raw = MockRawTransport::new();

    // Wrap it in a transport session (this handles all VISCA protocol logic)
    let mut transport = TransportSession::new(mock_raw.clone());

    println!("Created custom mock transport: {}", transport.description());
    println!("Transport connected: {}", transport.is_connected());

    println!("\nSending zoom stop command...");

    // Send a command - all VISCA protocol logic is handled automatically
    let command = ZoomCommand::Stop;
    match transport.send_command(&command).await {
        Ok(response) => {
            println!("✓ Command successful!");
            println!("  Response: {:?}", response);
        }
        Err(e) => {
            println!("✗ Command failed: {}", e);
        }
    }

    // Show what was actually sent
    let sent = mock_raw.sent_commands();
    println!("\nCommands sent by transport:");
    for (i, cmd) in sent.iter().enumerate() {
        println!("  {}: {:02X?}", i + 1, cmd);
    }

    println!("\n=== Custom Transport Benefits ===");
    println!("• Only needed to implement send() and receive() methods");
    println!("• All VISCA socket management handled automatically");
    println!("• All response parsing and correlation handled automatically");
    println!("• Same API as TCP/UDP/Serial transports");
    println!("• Can be wrapped in ChannelTransport for thread safety");

    println!("\n=== Implementation Stats ===");
    println!("Custom transport implementation: ~50 lines");
    println!("VISCA protocol logic reused: ~400 lines from library");
    println!("Total functionality: Equivalent to 450+ line single-purpose implementation");

    Ok(())
}
