//! Quickstart example using async-std runtime
//!
//! This example demonstrates basic async camera control using async-std runtime.
//!
//! Run with:
//! ```bash
//! cargo run --example quickstart_async_asyncstd --features rt-async-std
//! ```

#![cfg(feature = "rt-async-std")]

use grafton_visca::runtime_adapters::async_std::TcpTransport;
use grafton_visca::transport::AsyncTransport;
use std::error::Error;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Connect to camera
    println!("Connecting to camera at 192.168.0.110:5678...");
    let mut transport = TcpTransport::connect("192.168.0.110:5678").await?;
    
    // Send a simple power on command
    let power_on = vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
    println!("Sending power on command: {:02X?}", power_on);
    transport.send(&power_on).await?;
    
    // Receive response
    let response = transport.recv().await?;
    println!("Received response: {:02X?}", response.as_ref());
    
    // Send a power inquiry command
    let power_inquiry = vec![0x81, 0x09, 0x04, 0x00, 0xFF];
    println!("Sending power inquiry: {:02X?}", power_inquiry);
    transport.send(&power_inquiry).await?;
    
    // Receive response
    let response = transport.recv().await?;
    println!("Received response: {:02X?}", response.as_ref());
    
    println!("async-std example completed successfully!");
    Ok(())
}