//! Serial transport using the new clean API.
//!
//! This example shows how simple serial transport creation is with the new API.
//! All VISCA protocol logic is handled by the library.

#[cfg(feature = "async")]
use grafton_visca::{command::zoom::ZoomCommand, transport::create};

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Run with: cargo run --example serial_transport --features async");
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Serial Transport Example ===");

    // Create serial transport - one simple function call
    let mut transport = create::serial(1); // Camera address 1
    println!("✓ Serial transport created: {}", transport.description());
    println!("✓ Connected: {}", transport.is_connected());

    // Send a VISCA command - all protocol logic handled automatically
    let command = ZoomCommand::Stop;
    match transport.send_command(&command).await {
        Ok(response) => {
            println!("✓ Command sent successfully!");
            println!("  Response: {:?}", response);
        }
        Err(e) => {
            println!("✗ Command failed: {}", e);
        }
    }

    println!("\n=== Serial Transport Benefits ===");
    println!("• One-line creation: create::serial(camera_address)");
    println!("• All VISCA socket management handled automatically");
    println!("• All response parsing handled automatically");
    println!("• Simple send_command() interface");
    println!("• No manual socket correlation needed");

    println!("\n=== Real Serial Usage ===");
    println!("For real serial port usage, implement RawTransport trait:");
    println!("1. Add `serialport` dependency to Cargo.toml");
    println!("2. Implement RawTransport for your SerialPort wrapper");
    println!("3. Use ViscaTransport::new(your_serial_transport)");

    Ok(())
}
