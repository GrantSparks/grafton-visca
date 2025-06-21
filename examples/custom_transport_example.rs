//! Example demonstrating how to implement a custom transport.
//!
//! This example shows how to implement the RawTransport trait to create
//! custom transport implementations for different protocols or testing.

#![cfg(feature = "async")]

use grafton_visca::{
    camera::{Camera, PTZOpticsG2},
    transport::{RawTransport, TransportFuture, ViscaTransport},
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Example transport that works with any async runtime.
/// This demonstrates how to implement RawTransport without tokio dependencies.
#[derive(Debug)]
struct RuntimeAgnosticTransport {
    /// Mock connection state
    connected: bool,
    /// Buffer for simulating responses
    response_buffer: Arc<Mutex<Vec<Vec<u8>>>>,
    /// Description
    description: String,
}

impl RuntimeAgnosticTransport {
    fn new(description: String) -> Self {
        Self {
            connected: true,
            response_buffer: Arc::new(Mutex::new(vec![
                // Pre-populate with some mock responses
                vec![0x90, 0x40, 0xFF], // ACK
                vec![
                    0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF,
                ], // Position response
                vec![0x90, 0x40, 0xFF], // ACK
            ])),
            description,
        }
    }
}

impl RawTransport for RuntimeAgnosticTransport {
    fn send<'a>(&'a mut self, data: &'a [u8]) -> TransportFuture<'a, ()> {
        // Create a future without using tokio
        let data_copy = data.to_vec();
        Box::pin(async move {
            println!("Sending data: {:02X?}", data_copy);

            // In a real implementation, you would:
            // 1. Use your runtime's I/O primitives
            // 2. Send data over the actual transport
            // 3. Handle errors appropriately

            // Simulate a small delay
            // Note: In a real implementation, you'd use your runtime's sleep function
            std::thread::sleep(Duration::from_millis(10));

            Ok(())
        })
    }

    fn receive(&mut self) -> TransportFuture<'_, Vec<u8>> {
        let buffer = self.response_buffer.clone();

        Box::pin(async move {
            // In a real implementation, you would:
            // 1. Use your runtime's I/O primitives
            // 2. Read from the actual transport
            // 3. Parse VISCA frames properly

            // For this example, return mock responses
            let mut responses = buffer.lock().unwrap();
            if let Some(response) = responses.pop() {
                println!("Receiving data: {:02X?}", response);
                Ok(response)
            } else {
                // Default ACK response
                let default_response = vec![0x90, 0x40, 0xFF];
                println!("Receiving default ACK: {:02X?}", default_response);
                Ok(default_response)
            }
        })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn description(&self) -> &str {
        &self.description
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("This example demonstrates custom transport implementation.");

    // Create our custom transport
    let raw_transport =
        RuntimeAgnosticTransport::new("Custom transport with tokio runtime".to_string());

    let transport = ViscaTransport::new(raw_transport);
    let camera = Camera::<PTZOpticsG2>::new(transport);

    println!("\nUsing custom transport implementation!");
    println!("This transport could be used for:");
    println!("- Testing without real hardware");
    println!("- Implementing new protocols");
    println!("- Adding logging/debugging layers");

    // Test the transport
    camera.power_on().await?;
    println!("Power on command sent through custom transport");

    Ok(())
}
