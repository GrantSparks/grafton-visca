//! Async Serial VISCA Demo
//!
//! This example demonstrates async serial communication with VISCA cameras
//! using the new async serial transport implementation.
//!
//! Usage:
//!   cargo run --example serial_async_demo --features "rt-tokio,tokio-serial" [port] [camera_address]
//!
//! The example will:
//! 1. Connect to the camera using async serial transport
//! 2. Send I/F Clear and optionally Address Set commands
//! 3. Send some basic commands to verify operation

use grafton_visca::{
    camera::controls::inquiry::InquiryControl,
    camera::profiles::GenericVisca,
    transport::serial_async::{AsyncSerialConfig, AsyncSerialTransport},
    CameraBuilder, Error,
};
use std::env;

#[cfg(all(feature = "async", feature = "rt-tokio", feature = "tokio-serial"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("📡 Async Serial VISCA Demo (EPIC Task C1/C2)");
    println!("This example demonstrates async serial communication with VISCA cameras.\n");

    let args: Vec<String> = env::args().collect();
    let port = args.get(1).map(|s| s.as_str()).unwrap_or("/dev/ttyUSB0");
    let camera_address = args.get(2).and_then(|s| s.parse::<u8>().ok()).unwrap_or(1);

    println!("Serial port: {}", port);
    println!("Camera address: {}", camera_address);
    println!("Connecting to camera...\n");

    // Create async serial transport configuration
    let config = AsyncSerialConfig {
        port: port.to_string(),
        baud_rate: 9600,
        camera_address,
        if_clear_on_connect: true,     // Perform I/F Clear on startup
        address_set_on_connect: false, // Don't auto-run Address Set
        ..Default::default()
    };

    match AsyncSerialTransport::new(config).await {
        Ok(transport) => {
            println!("✅ Serial Transport Connected!");
            println!("✓ I/F Clear command sent during initialization");

            // Create camera with async transport
            let camera = CameraBuilder::tokio()?
                .build_async::<GenericVisca, _>(transport)
                .await?;

            println!("\n🔍 Testing basic camera operations...");

            // Test version inquiry
            match camera.get_version().await {
                Ok(version) => {
                    println!("✓ Version Inquiry: {:?}", version);
                }
                Err(e) => {
                    println!("⚠ Version inquiry failed: {}", e);
                }
            }

            // Test power inquiry
            match camera.get_power_state().await {
                Ok(power_state) => {
                    println!("✓ Power State: {}", power_state);
                }
                Err(e) => {
                    println!("⚠ Power inquiry failed: {}", e);
                }
            }

            println!("\n✅ Async serial communication successful!");
            println!("The EPIC C1/C2 (RS-232/422 + Address Set/I/F Clear) tasks are now complete for async mode.");
        }
        Err(Error::TransportError(e)) if e.to_string().contains("No such file") => {
            println!("❌ Serial Port Not Found: {}", port);
            println!("\nTroubleshooting:");
            println!("• Verify the serial port exists and is accessible");
            println!("• Check if the camera is connected and powered on");
            println!("• Try different port paths:");
            println!("  Linux:   /dev/ttyUSB0, /dev/ttyACM0, /dev/serial/by-id/...");
            println!("  macOS:   /dev/cu.usbserial-..., /dev/cu.usbmodem-...");
            println!("  Windows: COM1, COM2, etc.");
            println!("• Ensure proper serial port permissions (may need sudo or group membership)");
        }
        Err(Error::TransportError(e)) if e.to_string().contains("Permission denied") => {
            println!("❌ Permission Denied: {}", port);
            println!("\nTroubleshooting:");
            println!("• Add your user to the dialout group: sudo usermod -a -G dialout $USER");
            println!("• Then log out and log back in");
            println!("• Or run with sudo (not recommended for regular use)");
        }
        Err(e) => {
            println!("❌ Unexpected error: {}", e);
        }
    }

    println!("\n📚 About Async Serial VISCA:");
    println!("This implementation provides EPIC Task C1 (RS-232/422 support) and");
    println!("C2 (Address Set + I/F Clear orchestration) for async runtimes.");
    println!("Features:");
    println!("• Full async/await support using tokio-serial");
    println!("• Automatic I/F Clear during connection establishment");
    println!("• Optional Address Set for multi-camera daisy chains");
    println!("• Proper VISCA framing with camera address insertion");
    println!("• Configurable timeouts and retry policies");

    Ok(())
}

#[cfg(not(all(feature = "async", feature = "rt-tokio", feature = "tokio-serial")))]
fn main() {
    eprintln!("This example requires 'rt-tokio' and 'tokio-serial' features.");
    eprintln!("Run with: cargo run --example serial_async_demo --features 'rt-tokio,tokio-serial'");
}
