//! TCP transport example showing low-level transport usage.
//!
//! This example demonstrates using the TCP transport directly.
//! For most use cases, prefer using the Camera API instead.

#[cfg(feature = "tokio")]
use grafton_visca::{prelude::r#async::*, transport::tokio::Tcp};
#[cfg(feature = "tokio")]
use std::time::Duration;

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example tcp_transport --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== TCP Transport Example ===");

    // Create TCP transport directly
    let addr = "192.168.0.110:5678";
    match Tcp::connect_timeout(addr, Duration::from_secs(5)).await {
        Ok(transport) => {
            println!("✓ TCP transport created for {addr}");

            // Create a camera using the transport
            let camera = PTZOpticsG2Cam::new(transport);

            // Send a VISCA command using camera methods
            match camera.zoom_stop().await {
                Ok(_) => {
                    println!("✓ Zoom stop command sent successfully!");
                }
                Err(e) => {
                    println!("✗ Command failed: {e}");
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to create TCP transport: {e}");
            println!("Note: Make sure a VISCA camera is available at {addr}.");
        }
    }

    println!("\n=== TCP Transport Features ===");
    println!("• Interior mutability - use &self instead of &mut self");
    println!("• Automatic VISCA protocol handling");
    println!("• Built-in connection management");
    println!("• Configurable timeouts");
    println!("• Works with the Camera API for high-level control");

    Ok(())
}
