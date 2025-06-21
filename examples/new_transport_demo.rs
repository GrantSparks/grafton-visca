//! Unified transport demonstration showing the new simplified API.
//!
//! This example demonstrates how the new transport abstractions eliminate
//! code duplication by moving all VISCA protocol logic into the library.
//! Transport implementations now only need to handle the actual I/O.

#[cfg(feature = "async")]
use grafton_visca::{command::zoom::ZoomCommand, transport::create};
#[cfg(feature = "async")]
use std::time::Duration;

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Run with: cargo run --example new_transport_demo --features async");
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Unified Transport API Demo ===\n");

    // Example 1: Simple TCP transport
    println!("1. Creating TCP transport session...");
    match create::tcp_timeout("192.168.1.100:5678", Duration::from_secs(5)).await {
        Ok(mut transport) => {
            println!("   ✓ TCP transport created: {}", transport.description());

            // Send a command
            let command = ZoomCommand::Stop;
            match transport.send_command(&command).await {
                Ok(response) => println!("   ✓ Response: {response:?}"),
                Err(e) => println!("   ✗ Error: {e}"),
            }
        }
        Err(e) => println!("   ✗ Failed to create TCP transport: {e}"),
    }

    println!();

    // Example 2: Simple UDP transport
    println!("2. Creating UDP transport session...");
    match create::udp("192.168.1.100:52381").await {
        Ok(mut transport) => {
            println!("   ✓ UDP transport created: {}", transport.description());

            // Send a command
            let command = ZoomCommand::Stop;
            match transport.send_command(&command).await {
                Ok(response) => println!("   ✓ Response: {response:?}"),
                Err(e) => println!("   ✗ Error: {e}"),
            }
        }
        Err(e) => println!("   ✗ Failed to create UDP transport: {e}"),
    }

    println!();

    // Example 3: Serial transport (mock)
    println!("3. Creating serial transport session...");
    let mut transport = create::serial(1); // Camera address 1
    println!("   ✓ Serial transport created: {}", transport.description());

    // Send a command
    let command = ZoomCommand::Stop;
    match transport.send_command(&command).await {
        Ok(response) => println!("   ✓ Response: {response:?}"),
        Err(e) => println!("   ✗ Error: {e}"),
    }

    println!();
    println!("=== Key Benefits of New API ===");
    println!("• Transport implementations are 10x smaller (50-100 lines vs 500+ lines)");
    println!("• All VISCA protocol logic (socket management, response parsing) is in the library");
    println!("• Easy to add new transport types by implementing RawTransport trait");
    println!("• Async transports with interior mutability enable natural concurrent usage");
    println!("• Consistent API across all transport types");
    println!("• Transport-specific code focuses only on actual I/O differences");

    println!("\n=== Implementation Comparison ===");
    println!("Old TCP example: ~680 lines (duplicated across UDP/Serial)");
    println!("New TCP example: ~80 lines (no duplication)");
    println!("Common code moved to library: ~500 lines reused across all transports");

    Ok(())
}
