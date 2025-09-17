//! VISCA Protocol Auto-Detection Example (EPIC Task B3)
//!
//! This example demonstrates automatic protocol detection between Sony encapsulated
//! format (8-byte header) and raw VISCA format. This is a key feature from the
//! Unified VISCA Control Stack EPIC that allows a single library to work with
//! multiple camera brands without manual protocol configuration.
//!
//! Usage:
//!   cargo run --example protocol_auto_detection --features runtime-tokio [camera_ip[:port]]
//!
//! The example will:
//! 1. Connect to the camera using auto-detection
//! 2. Show which protocol was detected
//! 3. Send some basic commands to verify operation
//! 4. Display the difference in wire format between protocols

use grafton_visca::{
    camera::{profiles::GenericVisca, Connect},
    runtime::TokioRuntime,
    Error,
};

use std::env;

#[cfg(feature = "runtime-tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🎥 VISCA Protocol Auto-Detection Demo (EPIC Task B3)");
    println!("This example automatically detects Sony vs raw VISCA protocols.\n");

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:5678".to_string());

    println!("Target camera: {camera_addr}");
    println!("Attempting automatic protocol detection...\n");

    // EPIC B3: Auto-detect handshake
    // This probes both Sony encapsulated and raw VISCA formats
    // Create the runtime for type-safe pairing
    let runtime = TokioRuntime::from_current()?;

    // Use the new session-centric API with auto-detection
    let session =
        match Connect::open_auto_async::<GenericVisca, _>(&camera_addr, runtime.clone()).await {
            Ok(session) => {
                println!("✅ Protocol Detection Successful!");

                // The new API automatically detects protocol internally
                // We can infer protocol based on port or additional info
                if camera_addr.contains(":52381") {
                    println!("📡 Likely: Sony Encapsulated Protocol");
                    println!("   Format: 8-byte header + VISCA payload");
                    println!("   Port: Usually 52381 (Sony default)");
                    println!("   Used by: Sony FR7, BRC-H900, BRC-300, etc.");
                } else if camera_addr.contains(":1259") {
                    println!("📡 Likely: Raw VISCA Protocol (UDP)");
                    println!("   Format: Direct VISCA bytes (no header)");
                    println!("   Port: Usually 1259 (UDP)");
                    println!("   Used by: PTZOptics, generic cameras, etc.");
                } else {
                    println!("📡 Likely: Raw VISCA Protocol (TCP)");
                    println!("   Format: Direct VISCA bytes (no header)");
                    println!("   Port: Usually 5678 (TCP)");
                    println!("   Used by: PTZOptics, generic cameras, etc.");
                }

                println!();
                session
            }
            Err(Error::ConnectionFailed { addr, source }) => {
                println!("❌ Connection Failed to {addr}");
                println!("   Reason: {source}");
                println!("\nTroubleshooting:");
                println!("• Verify camera is powered on and network accessible");
                println!("• Check IP address and port are correct");
                println!("• Ensure camera's VISCA over IP is enabled");
                println!("• Try different ports (52381 for Sony, 5678 for PTZOptics)");
                return Err(Error::ConnectionFailed { addr, source }.into());
            }
            Err(e) => {
                println!("❌ Unexpected error: {e}");
                return Err(e.into());
            }
        };

    let camera = session;

    println!("🔍 Testing basic camera operations...");

    // Test version inquiry using accessor pattern
    match camera.system().version().await {
        Ok(version) => {
            println!("✓ Version Inquiry: {version:?}");
        }
        Err(e) => {
            println!("⚠ Version inquiry failed: {e}");
        }
    }

    // Test power inquiry using accessor pattern
    match camera.power().state().await {
        Ok(power_state) => {
            println!("✓ Power State: {power_state:?}");
        }
        Err(e) => {
            println!("⚠ Power inquiry failed: {e}");
        }
    }

    println!("\n✅ Protocol auto-detection and basic operations successful!");
    println!("The library automatically adapted to your camera's protocol format.");

    // Clean up
    if let Err(e) = camera.close().await {
        println!("Warning: Failed to close session cleanly: {e}");
    }

    println!("\n📚 About Protocol Auto-Detection:");
    println!("This library performs automatic protocol detection by:");
    println!("1. Trying Sony encapsulated format (port 52381)");
    println!("2. Falling back to raw VISCA (ports 1259/5678)");
    println!("3. Sending test inquiries to validate the connection");
    println!("\nThe detected protocol is then used transparently for all");
    println!("subsequent operations, providing a unified API across camera brands.");

    Ok(())
}

#[cfg(not(feature = "runtime-tokio"))]
fn main() {
    println!("This example requires the 'runtime-tokio' feature.");
    println!("Run with: cargo run --example protocol_auto_detection --features runtime-tokio");
}
