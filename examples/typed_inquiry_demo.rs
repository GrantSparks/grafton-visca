//! Example demonstrating typed inquiry responses using high-level API methods.
//!
//! This example shows how the high-level API methods provide strongly-typed
//! responses for inquiry commands, eliminating manual parsing.
//!
//! Run with:
//! ```bash
//! cargo run --example typed_inquiry_demo
//! ```

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::Camera,
    mode::Blocking,
    profiles::GenericVisca,
    transport::blocking::Tcp,
    Error,
    // Import unified traits
    PowerControl,
    ZoomControl,
};
#[cfg(not(feature = "async"))]
use std::time::Duration;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    // Initialize logging
    let _ = tracing_subscriber::fmt::try_init();

    // Build a blocking TCP transport
    let address = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.1.100:5678".to_string());
    println!("Connecting to camera at {address}...");

    let transport = Tcp::connect(&address)?;

    let mut camera = Camera::<Blocking, GenericVisca, _>::new_blocking(transport)?;

    println!("\n=== High-Level API Inquiry Demo ===\n");

    // Power status - using high-level PowerControlBlocking trait
    println!("Checking power status...");
    match camera.power_inquiry() {
        Ok(power_on) => {
            let status = if power_on { "ON" } else { "OFF" };
            println!("  Power: {status}");
        }
        Err(e) => println!("  Power inquiry failed: {e}"),
    }

    // Note: Pan/Tilt position inquiry would be available in async mode
    // For this blocking example, we'll skip it

    // Zoom position - using high-level ZoomControlBlocking trait
    println!("\nChecking zoom position...");
    match camera.zoom_position_inquiry() {
        Ok(zoom_pos) => {
            let raw_value = zoom_pos.value();
            let zoom_percentage = (raw_value as f32 / 0x4000 as f32) * 100.0;
            println!("  Zoom: 0x{:04X} ({:.1}%)", raw_value, zoom_percentage);
        }
        Err(e) => println!("  Zoom inquiry failed: {e}"),
    }

    // Note: Focus and System inquiries would be available in async mode
    // For this blocking example, we'll skip them

    // Demonstrate other high-level methods
    println!("\nDemonstrating control methods...");

    // Try to zoom in slightly
    println!("Zooming in...");
    if let Err(e) = camera.zoom_tele_std() {
        println!("  Zoom command failed: {e}");
    } else {
        // Wait a moment and stop
        std::thread::sleep(Duration::from_millis(500));
        let _ = camera.zoom_stop();
        println!("  Zoom completed");
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example requires blocking mode. Build without async features.");
    eprintln!("Try: cargo run --example typed_inquiry_demo --no-default-features");
}
