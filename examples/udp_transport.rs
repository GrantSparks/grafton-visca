//! UDP transport example showing low-level transport usage.
//!
//! This example demonstrates using the UDP transport directly.
//! For most use cases, prefer using the Camera API instead.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::methods::ZoomOps, profiles::GenericVisca, transport::tokio::Udp, Camera,
};

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example udp_transport --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== UDP Transport Example ===");

    // Create UDP transport directly
    let addr = "192.168.1.100:52381";
    match Udp::connect(addr).await {
        Ok(transport) => {
            println!("✓ UDP transport created for {}", addr);

            // Create a camera using the transport
            let camera = Camera::<GenericVisca, _>::new(transport);

            // Send a VISCA command using high-level API
            match camera.zoom_stop().await {
                Ok(_) => {
                    println!("✓ Zoom stop command sent successfully!");
                }
                Err(e) => {
                    println!("✗ Command failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to create UDP transport: {}", e);
            println!("Note: Make sure a VISCA camera is available at {}.", addr);
        }
    }

    println!("\n=== UDP Transport Features ===");
    println!("• Interior mutability - use &self instead of &mut self");
    println!("• Automatic VISCA protocol handling");
    println!("• Simple bind and connect");
    println!("• Works with the Camera API for high-level control");
    println!("• Lower latency than TCP for real-time control");

    Ok(())
}
