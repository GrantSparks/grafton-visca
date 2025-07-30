//! Example demonstrating timing patterns with the Camera API.
//!
//! This example shows how to:
//! - Measure command execution times
//! - Handle long-running operations
//! - Demonstrate retry patterns
//!
//! Note: The Camera API doesn't have built-in timeout support.
//! This example shows patterns for timing operations.

#[cfg(feature = "async")]
fn main() {
    eprintln!("This example demonstrates timing patterns with the blocking API.");
    eprintln!("Run without async features: cargo run --example configurable_timeouts");
}

#[cfg(not(feature = "async"))]
use grafton_visca::{
    prelude::blocking::*,
    transport::blocking::Tcp,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    units::Degrees,
    Error, PanTiltDirection, PresetNumber,
};
#[cfg(not(feature = "async"))]
use std::time::{Duration, Instant};

#[cfg(not(feature = "async"))]
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

#[cfg(not(feature = "async"))]
fn demonstrate_camera_timing(camera_addr: &str) -> Result<(), Error> {
    println!("Connecting to camera at {camera_addr}...");
    let transport = Tcp::connect(camera_addr)?;
    let camera = GenericViscaCam::new(transport);

    println!("Note: The Camera API doesn't have built-in timeout support.");
    println!("These examples show execution timing patterns.\n");

    // Demonstrate commands with timing
    println!("1. Quick Commands (Inquiries):");
    println!("   These commands should complete quickly\n");

    // Note: Direct power inquiry method not available in Camera API
    // Power on command (will succeed if already on)
    let start = Instant::now();
    match camera.power_on() {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Power on command completed in {elapsed:?}");
        }
        Err(e) => println!("   ✗ Power on failed: {e}"),
    }

    // Note: The Camera API doesn't have inquiry methods like get_position() or get_zoom_position()
    // These would need to be implemented using the inquiry module
    println!("   Note: Position and zoom inquiries not available in Camera API");

    println!();

    // Wait for initialization
    std::thread::sleep(Duration::from_secs(2));

    // Movement with timing
    println!("\n2. Movement Commands:");
    println!("   These commands may take longer to acknowledge\n");

    let start = Instant::now();
    match (|| {
        camera.pan_tilt_move(
            PanTiltDirection::Left,
            PanSpeed::try_from(15)?,
            TiltSpeed::try_from(0)?,
        )
    })() {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Start movement completed in {elapsed:?}");

            // Move for 2 seconds
            std::thread::sleep(Duration::from_secs(2));

            // Stop movement
            let stop_start = Instant::now();
            match camera.pan_tilt_stop() {
                Ok(_) => {
                    let stop_elapsed = stop_start.elapsed();
                    println!("   ✓ Stop movement completed in {stop_elapsed:?}");
                }
                Err(e) => println!("   ✗ Stop movement failed: {e}"),
            }
        }
        Err(e) => println!("   ✗ Start movement failed: {e}"),
    }

    // Preset operations
    println!("\n3. Preset Operations:");
    println!("   These commands may take significant time\n");

    // Save preset
    let start = Instant::now();
    // Note: PresetNumber doesn't have From<G2PresetId> implementation
    // Using PresetNumber directly
    match (|| camera.preset_set(PresetNumber::new(1)?))() {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Save preset completed in {elapsed:?}");
        }
        Err(e) => println!("   ✗ Save preset failed: {e}"),
    }

    // Move away
    camera.pan_tilt_absolute(Degrees(0.0), Degrees(0.0), SpeedLevel::from(5))?;
    std::thread::sleep(Duration::from_secs(1));

    // Recall preset
    let start = Instant::now();
    match camera.preset_recall(PresetNumber::new(1)?) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Recall preset completed in {elapsed:?}");
            println!("   Note: Preset recall can take 30+ seconds on some cameras");
        }
        Err(e) => println!("   ✗ Recall preset failed: {e}"),
    }

    // 4. Retry pattern demonstration
    println!("\n4. Retry Pattern:");
    println!("   Demonstrating retry with backoff\n");

    let max_attempts = 3;
    let mut attempt = 0;
    let mut backoff = Duration::from_millis(100);

    loop {
        attempt += 1;
        println!("   Attempt {attempt}/{max_attempts}");

        let start = Instant::now();
        // Note: get_focus_position() is not available in Camera API
        // Using a simple command instead for timing demonstration
        match camera.focus_auto() {
            Ok(_) => {
                let elapsed = start.elapsed();
                println!("   ✓ Success on attempt {attempt} in {elapsed:?}: focus set to auto");
                break;
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!("   ✗ Attempt {attempt} failed after {elapsed:?}: {e}");

                if attempt >= max_attempts {
                    println!("   Max attempts reached, giving up");
                    break;
                }

                println!("   Waiting {backoff:?} before retry...");
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
