//! UDP transport using the new clean API.
//!
//! This example shows how simple UDP transport creation is with the new API.
//! All VISCA protocol logic is handled by the library.

#[cfg(feature = "async")]
use grafton_visca::{command::zoom::ZoomCommand, transport::create};

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Run with: cargo run --example udp_transport --features async");
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== UDP Transport Example ===");

    // Create UDP transport - one simple function call
    match create::udp("192.168.1.100:52381").await {
        Ok(mut transport) => {
            println!("✓ UDP transport created: {}", transport.description());
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
        }
        Err(e) => {
            println!("✗ Failed to create UDP transport: {}", e);
            println!("Note: Make sure a VISCA camera is available at the specified address.");
        }
    }

    println!("\n=== UDP Transport Benefits ===");
    println!("• One-line creation: create::udp(addr)");
    println!("• All VISCA socket management handled automatically");
    println!("• All response parsing handled automatically");
    println!("• Simple send_command() interface");
    println!("• No manual socket correlation needed");

    Ok(())
}
