//! Unified transport demonstration showing the interior mutability API.
//!
//! This example demonstrates how the new transport abstractions with interior
//! mutability enable cleaner APIs and better concurrent usage patterns.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::profiles::GenericVisca,
    transport::tokio::{Tcp, Udp},
    Camera,
};
#[cfg(feature = "tokio")]
use std::time::Duration;

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
    match Tcp::connect_timeout("192.168.1.100:5678", Duration::from_secs(5)).await {
        Ok(transport) => {
            println!("   ✓ TCP transport created");

            // Create camera with the transport
            let camera = Camera::<GenericVisca, _>::new(transport);

            // Use camera methods - note we use &camera, not &mut camera
            use grafton_visca::camera::methods::ZoomAsyncExt;
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
    match Udp::connect("192.168.1.100:52381").await {
        Ok(transport) => {
            println!("   ✓ UDP transport created");

            // Create camera with the transport
            let camera = Camera::<GenericVisca, _>::new(transport);

            // Use camera methods - using &camera (interior mutability)
            use grafton_visca::camera::methods::ZoomAsyncExt;
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
