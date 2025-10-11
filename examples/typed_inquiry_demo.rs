//! Example demonstrating typed inquiry responses using high-level API methods.
//!
//! This example shows how the high-level API methods provide strongly-typed
//! responses for inquiry commands, eliminating manual parsing.
//!
//! Run with:
//! ```bash
//! cargo run --example typed_inquiry_demo
//! ```

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{camera::Connect, profiles::GenericVisca, Error};

#[cfg(not(feature = "mode-async"))]
use std::time::Duration;

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt::try_init();

    let address = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.1.100".to_string());
    println!("Connecting to camera at {address}...");

    let camera = Connect::open_tcp_blocking::<GenericVisca>(&address)?;

    println!("\n=== High-Level API Inquiry Demo ===\n");

    println!("Checking power status...");
    match camera.power_state() {
        Ok(power_on) => {
            let status = if power_on { "ON" } else { "OFF" };
            println!("  Power: {status}");
        }
        Err(e) => println!("  Power inquiry failed: {e}"),
    }

    // NOTE: Pan/Tilt position inquiry would be available in async mode

    println!("\nChecking zoom position...");
    match camera.zoom_position() {
        Ok(zoom_pos) => {
            let raw_value = zoom_pos.value();
            let zoom_percentage = (raw_value as f32 / 0x4000 as f32) * 100.0;
            println!("  Zoom: 0x{raw_value:04X} ({zoom_percentage:.1}%)");
        }
        Err(e) => println!("  Zoom inquiry failed: {e}"),
    }

    // NOTE: Focus and System inquiries would be available in async mode

    println!("\nDemonstrating control methods...");

    println!("Zooming in...");
    if let Err(e) = camera.zoom_tele(None) {
        println!("  Zoom command failed: {e}");
    } else {
        std::thread::sleep(Duration::from_millis(500));
        let _ = camera.zoom_stop();
        println!("  Zoom completed");
    }

    println!("\n=== Demo Complete ===");
    Ok(())
}

#[cfg(feature = "mode-async")]
fn main() {
    eprintln!("This example requires blocking mode. Build without async features.");
    eprintln!("Try: cargo run --example typed_inquiry_demo --no-default-features");
}
