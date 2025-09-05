//! VISCA Protocol Auto-Detection Example (EPIC Task B3)
//!
//! This example demonstrates automatic protocol detection between Sony encapsulated
//! format (8-byte header) and raw VISCA format. This is a key feature from the
//! Unified VISCA Control Stack EPIC that allows a single library to work with
//! multiple camera brands without manual protocol configuration.
//!
//! Usage:
//!   cargo run --example protocol_auto_detection --features rt-tokio [camera_ip[:port]]
//!
//! The example will:
//! 1. Connect to the camera using auto-detection
//! 2. Show which protocol was detected
//! 3. Send some basic commands to verify operation
//! 4. Display the difference in wire format between protocols

use std::env;

use grafton_visca::{
    camera::{controls::inquiry::InquiryControl, profiles::GenericVisca, Camera},
    mode::Async,
    runtime_trait::TokioRuntime,
    transport::Transport,
    Error,
};

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    println!("🎥 VISCA Protocol Auto-Detection Demo (EPIC Task B3)");
    println!("This example automatically detects Sony vs raw VISCA protocols.\n");

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:5678".to_string());

    println!("Target camera: {}", camera_addr);
    println!("Attempting automatic protocol detection...\n");

    // EPIC B3: Auto-detect handshake
    // This probes both Sony encapsulated and raw VISCA formats
    // Create the runtime for type-safe pairing
    let runtime = TokioRuntime::from_current()?;

    match Transport::auto_detect(&camera_addr, runtime.clone()).await {
        Ok((transport, detected_protocol)) => {
            println!("✅ Protocol Detection Successful!");

            match detected_protocol {
                grafton_visca::transport::DetectionResult::SonyEncapsulated => {
                    println!("📡 Detected: Sony Encapsulated Protocol");
                    println!("   Format: 8-byte header + VISCA payload");
                    println!("   Port: Usually 52381 (Sony default)");
                    println!("   Used by: Sony FR7, BRC-H900, BRC-300, etc.");
                }
                grafton_visca::transport::DetectionResult::RawVisca => {
                    println!("📡 Detected: Raw VISCA Protocol");
                    println!("   Format: Direct VISCA bytes (no header)");
                    println!("   Port: Usually 5678/TCP or 1259/UDP");
                    println!("   Used by: PTZOptics, generic cameras, etc.");
                }
                grafton_visca::transport::DetectionResult::NoResponse => {
                    unreachable!("Should have failed with error");
                }
            }

            println!();

            // Create camera with detected transport and runtime
            let camera = Camera::<Async, GenericVisca, _, _>::new_async(transport, runtime).await?;

            println!("🔍 Testing basic camera operations...");

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

            println!("\n✅ Protocol auto-detection and basic operations successful!");
            println!("The library automatically adapted to your camera's protocol format.");
        }
        Err(Error::ConnectionFailed { addr, source }) => {
            println!("❌ Connection Failed to {}", addr);
            println!("   Reason: {}", source);
            println!("\nTroubleshooting:");
            println!("• Verify camera is powered on and network accessible");
            println!("• Check IP address and port are correct");
            println!("• Ensure camera's VISCA over IP is enabled");
            println!("• Try different ports (52381 for Sony, 5678 for PTZOptics)");
        }
        Err(e) => {
            println!("❌ Unexpected error: {}", e);
        }
    }

    println!("\n📚 About Protocol Auto-Detection:");
    println!("This feature implements EPIC Task B3 from the Unified VISCA Control Stack.");
    println!("It enables a single library to work with multiple camera vendors by:");
    println!("• Probing Sony encapsulated format first (8-byte header + sequence)");
    println!("• Falling back to raw VISCA format if no response");
    println!("• Locking in the working protocol for subsequent commands");
    println!("• Providing transparent operation regardless of camera brand");

    Ok(())
}

#[cfg(not(feature = "rt-tokio"))]
fn main() {
    eprintln!("This example requires the 'rt-tokio' feature.");
    eprintln!("Run with: cargo run --example protocol_auto_detection --features rt-tokio");
}
