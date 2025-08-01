//! Unified transport demonstration showing the interior mutability API.
//!
//! This example demonstrates how the new transport abstractions with interior
//! mutability enable cleaner APIs and better concurrent usage patterns.

#[cfg(feature = "tokio")]
use grafton_visca::{
    prelude::r#async::*,
    transport::{TcpTransport, UdpTransport},
};

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example new_transport_demo --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Unified Transport API Demo ===\n");

    // Example 1: TCP transport with Camera API
    println!("1. Creating TCP transport session...");
    match TcpTransport::connect("192.168.1.100:5678").await {
        Ok(transport) => {
            println!("   ✓ TCP transport created");

            // Create camera with the transport
            let camera = PTZOpticsG2Cam::new_async(transport);

            // Use camera methods - note we use &camera, not &mut camera
            match camera.zoom_stop().await {
                Ok(_) => println!("   ✓ Zoom stop command sent"),
                Err(e) => println!("   ✗ Error: {e}"),
            }
        }
        Err(e) => println!("   ✗ Failed to create TCP transport: {e}"),
    }

    println!();

    // Example 2: UDP transport with Camera API
    println!("2. Creating UDP transport session...");
    match UdpTransport::connect("0.0.0.0:0", "192.168.1.100:52381").await {
        Ok(transport) => {
            println!("   ✓ UDP transport created");

            // Create camera with the transport
            let camera = PTZOpticsG2Cam::new_async(transport);

            // Use camera methods - using &camera (interior mutability)
            match camera.zoom_stop().await {
                Ok(_) => println!("   ✓ Zoom stop command sent"),
                Err(e) => println!("   ✗ Error: {e}"),
            }
        }
        Err(e) => println!("   ✗ Failed to create UDP transport: {e}"),
    }

    println!();

    println!();
    println!("=== Key Benefits of Interior Mutability API ===");
    println!("• Methods take &self instead of &mut self - more ergonomic");
    println!("• Enables natural concurrent usage patterns");
    println!("• No need for Arc<Mutex<Camera>> wrapper in user code");
    println!("• All VISCA protocol logic handled internally");
    println!("• Consistent API across all transport types");
    println!("• Transport implementations focus only on I/O");

    println!("\n=== Interior Mutability Design ===");
    println!("• Transport layer uses Arc<Mutex<T>> internally");
    println!("• Camera methods can be called on shared references");
    println!("• Thread-safe by default for async operations");
    println!("• Simplified error handling and response parsing");

    Ok(())
}
