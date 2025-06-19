//! Example demonstrating timing patterns with the Camera API.
//!
//! This example shows how to:
//! - Measure command execution times
//! - Handle long-running operations
//! - Demonstrate retry patterns
//!
//! Note: The Camera API doesn't have built-in timeout support.
//! This example shows patterns for timing operations.

#[path = "udp_transport.rs"]
mod udp_transport;
mod common;
use common::blocking::UdpTransport;

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, units::Degrees, Camera},
    command::pan_tilt::PanTiltDirection,
    transport::BlockingAdapter,
    Error,
};
use std::time::{Duration, Instant};

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== VISCA Command Timing Patterns ===\n");
    println!("This example demonstrates:");
    println!("- Measuring command execution times");
    println!("- Handling long-running operations");
    println!("- Implementing retry patterns\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    demonstrate_camera_timing(&camera_addr)?;

    Ok(())
}

fn demonstrate_camera_timing(camera_addr: &str) -> Result<(), Error> {
    use grafton_visca::camera::profiles::G2PresetId;

    println!("Connecting to camera at {}...", camera_addr);
    let udp_transport = UdpTransport::new(camera_addr)?;
    let camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(udp_transport));

    println!("Note: The Camera API doesn't have built-in timeout support.");
    println!("These examples show execution timing patterns.\n");

    // Demonstrate commands with timing
    println!("1. Quick Commands (Inquiries):");
    println!("   These commands should complete quickly\n");

    // Note: Direct power inquiry method not available in Camera API
    // Power on command (will succeed if already on)
    let start = Instant::now();
    match block_on(camera.power_on()) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Power on command completed in {:?}", elapsed);
        }
        Err(e) => println!("   ✗ Power on failed: {}", e),
    }

    // Position inquiry
    let start = Instant::now();
    match block_on(camera.get_position()) {
        Ok((pan, tilt)) => {
            let elapsed = start.elapsed();
            println!(
                "   ✓ Position inquiry completed in {:?}: pan={:?}, tilt={:?}",
                elapsed, pan, tilt
            );
        }
        Err(e) => println!("   ✗ Position inquiry failed: {}", e),
    }

    // Zoom position inquiry
    let start = Instant::now();
    match block_on(camera.get_zoom_position()) {
        Ok(zoom) => {
            let elapsed = start.elapsed();
            println!("   ✓ Zoom inquiry completed in {:?}: {}", elapsed, zoom);
        }
        Err(e) => println!("   ✗ Zoom inquiry failed: {}", e),
    }

    println!();

    // Wait for initialization
    std::thread::sleep(Duration::from_secs(2));

    // Movement with timing
    println!("\n2. Movement Commands:");
    println!("   These commands may take longer to acknowledge\n");

    let start = Instant::now();
    match block_on(camera.move_continuous(PanTiltDirection::Left, 15, 0)) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Start movement completed in {:?}", elapsed);

            // Move for 2 seconds
            std::thread::sleep(Duration::from_secs(2));

            // Stop movement
            let stop_start = Instant::now();
            match block_on(camera.stop()) {
                Ok(_) => {
                    let stop_elapsed = stop_start.elapsed();
                    println!("   ✓ Stop movement completed in {:?}", stop_elapsed);
                }
                Err(e) => println!("   ✗ Stop movement failed: {}", e),
            }
        }
        Err(e) => println!("   ✗ Start movement failed: {}", e),
    }

    // Preset operations
    println!("\n3. Preset Operations:");
    println!("   These commands may take significant time\n");

    let preset_id = G2PresetId::new(2)?;

    // Save preset
    let start = Instant::now();
    match block_on(camera.set_preset(preset_id)) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Save preset completed in {:?}", elapsed);
        }
        Err(e) => println!("   ✗ Save preset failed: {}", e),
    }

    // Move away
    block_on(camera.set_position(Degrees(0.0), Degrees(0.0)))?;
    std::thread::sleep(Duration::from_secs(1));

    // Recall preset
    let start = Instant::now();
    match block_on(camera.recall_preset(preset_id)) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Recall preset completed in {:?}", elapsed);
            println!("   Note: Preset recall can take 30+ seconds on some cameras");
        }
        Err(e) => println!("   ✗ Recall preset failed: {}", e),
    }

    // 4. Retry pattern demonstration
    println!("\n4. Retry Pattern:");
    println!("   Demonstrating retry with backoff\n");

    let max_attempts = 3;
    let mut attempt = 0;
    let mut backoff = Duration::from_millis(100);

    loop {
        attempt += 1;
        println!("   Attempt {}/{}", attempt, max_attempts);

        let start = Instant::now();
        match block_on(camera.get_focus_position()) {
            Ok(focus) => {
                let elapsed = start.elapsed();
                println!(
                    "   ✓ Success on attempt {} in {:?}: focus position = {}",
                    attempt, elapsed, focus
                );
                break;
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!("   ✗ Attempt {} failed after {:?}: {}", attempt, elapsed, e);

                if attempt >= max_attempts {
                    println!("   Max attempts reached, giving up");
                    break;
                }

                println!("   Waiting {:?} before retry...", backoff);
                std::thread::sleep(backoff);
                backoff *= 2; // Exponential backoff
            }
        }
    }

    println!("\n5. Summary:");
    println!("   - Camera API provides type-safe, profile-aware control");
    println!("   - Commands complete at different speeds based on type");
    println!("   - Implement retry patterns at the application level");
    println!("   - For timeout control, consider custom transport wrappers");

    Ok(())
}
