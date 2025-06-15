//! Example demonstrating timeout handling with the Camera API.
//!
//! This example shows how to:
//! - Configure timeouts at the transport level
//! - Implement custom timeout logic for different command types
//! - Handle timeout errors gracefully
//! - Retry commands with increasing timeouts
//!
//! Note: The Camera API doesn't directly support timeouts. This example
//! shows how to work with timeouts using the unified Client API or by
//! implementing custom transport wrappers.

#[cfg(feature = "blocking-client")]
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, units::Degrees, Camera},
    command::{pan_tilt::PanTiltDirection, InquiryCommand, Response},
    transport::{BlockingAdapter, UdpTransport},
    Client, Error,
};
#[cfg(feature = "blocking-client")]
use std::time::{Duration, Instant};

#[cfg(not(feature = "blocking-client"))]
fn main() {
    eprintln!("This example requires the 'blocking-client' feature.");
    eprintln!("Run with: cargo run --example configurable_timeouts --features blocking-client");
}

#[cfg(feature = "blocking-client")]
// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== VISCA Timeout Handling Example ===\n");
    println!("This example demonstrates two approaches:");
    println!("1. Using the unified Client API with send_with_timeout");
    println!("2. Using the Camera API (without built-in timeout support)\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Approach 1: Using unified Client with timeout support
    println!("=== Approach 1: Unified Client API ===\n");
    demonstrate_client_timeouts(&camera_addr)?;

    println!("\n=== Approach 2: Camera API ===\n");
    demonstrate_camera_api(&camera_addr)?;

    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demonstrate_client_timeouts(camera_addr: &str) -> Result<(), Error> {
    use grafton_visca::command::{
        pan_tilt::{PanSpeed, PanTiltCommand, TiltSpeed},
        preset::{PresetAction, PresetCommand, PresetNumber},
    };

    // Connect to camera using unified Client
    println!("Connecting to camera at {}...", camera_addr);
    let client = Client::connect_udp(camera_addr)?;

    // 1. Quick Commands with short timeout
    println!("1. Quick Commands (Inquiries):");
    println!("   These commands should complete quickly\n");

    let quick_timeout = Duration::from_secs(1);

    // Power inquiry
    let start = Instant::now();
    match client.send_with_timeout(&InquiryCommand::Power, quick_timeout) {
        Ok(Response::InquiryResponse(resp)) => {
            let elapsed = start.elapsed();
            println!("   ✓ Power inquiry completed in {:?}: {:?}", elapsed, resp);
        }
        Ok(_) => println!("   ✓ Power inquiry completed but unexpected response"),
        Err(e) => println!("   ✗ Power inquiry failed: {}", e),
    }

    // Position inquiry
    let start = Instant::now();
    match client.send_with_timeout(&InquiryCommand::PanTiltPosition, quick_timeout) {
        Ok(Response::InquiryResponse(resp)) => {
            let elapsed = start.elapsed();
            println!(
                "   ✓ Position inquiry completed in {:?}: {:?}",
                elapsed, resp
            );
        }
        Ok(_) => println!("   ✓ Position inquiry completed but unexpected response"),
        Err(e) => println!("   ✗ Position inquiry failed: {}", e),
    }

    println!();

    // 2. Movement Commands with medium timeout
    println!("2. Movement Commands:");
    println!("   These commands may take longer to acknowledge\n");

    let movement_timeout = Duration::from_secs(5);

    // Start movement
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Right,
        pan_speed: PanSpeed::new(10)?,
        tilt_speed: TiltSpeed::new(0)?,
    };

    let start = Instant::now();
    match client.send_with_timeout(&move_cmd, movement_timeout) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Movement command started in {:?}", elapsed);

            // Let it move for a bit
            std::thread::sleep(Duration::from_secs(1));

            // Stop movement
            let stop_cmd = PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            };

            match client.send_with_timeout(&stop_cmd, movement_timeout) {
                Ok(_) => println!("   ✓ Movement stopped"),
                Err(e) => println!("   ✗ Stop command failed: {}", e),
            }
        }
        Err(e) => {
            println!("   ✗ Movement command failed: {}", e);
        }
    }

    println!();

    // 3. Preset Commands with long timeout
    println!("3. Preset Commands:");
    println!("   These commands may take significant time to complete\n");

    let preset_timeout = Duration::from_secs(30);

    // Save current position as preset
    let save_preset = PresetCommand {
        action: PresetAction::Set,
        preset_number: PresetNumber::new(1)?,
    };

    let start = Instant::now();
    match client.send_with_timeout(&save_preset, preset_timeout) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Preset saved in {:?}", elapsed);
        }
        Err(e) => {
            println!("   ✗ Preset save failed: {}", e);
        }
    }

    println!();

    // 4. Retry with increasing timeouts
    println!("4. Retry with Increasing Timeouts:");
    println!("   Demonstrating adaptive timeout strategy\n");

    let command = InquiryCommand::FocusPosition;
    let timeouts = [
        Duration::from_millis(500),
        Duration::from_secs(1),
        Duration::from_secs(2),
    ];

    for (attempt, &timeout) in timeouts.iter().enumerate() {
        let attempt_num = attempt + 1;
        println!(
            "   Attempt {}/{} with timeout {:?}",
            attempt_num,
            timeouts.len(),
            timeout
        );

        let start = Instant::now();
        match client.send_with_timeout(&command, timeout) {
            Ok(Response::InquiryResponse(resp)) => {
                let elapsed = start.elapsed();
                println!(
                    "   ✓ Success on attempt {} in {:?}: {:?}",
                    attempt_num, elapsed, resp
                );
                break;
            }
            Ok(_) => {
                println!("   ✓ Command succeeded but unexpected response");
                break;
            }
            Err(e) => {
                let elapsed = start.elapsed();
                println!(
                    "   ✗ Attempt {} failed after {:?}: {}",
                    attempt_num, elapsed, e
                );

                if attempt_num < timeouts.len() {
                    println!("   Retrying with longer timeout...");
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    }

    println!();
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demonstrate_camera_api(camera_addr: &str) -> Result<(), Error> {
    use grafton_visca::camera::profiles::G2PresetId;

    // Connect to camera using Camera API
    println!(
        "Connecting to camera at {} using Camera API...",
        camera_addr
    );
    let udp_transport = UdpTransport::new(camera_addr)?;
    let mut camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(udp_transport));

    println!("Camera API Notes:");
    println!("- The Camera API doesn't have built-in timeout support");
    println!("- Timeouts would need to be implemented at the transport level");
    println!("- For timeout support, use the unified Client API instead\n");

    // Demonstrate commands without timeout control
    println!("1. Basic Commands (no timeout control):\n");

    // Power on
    let start = Instant::now();
    match block_on(camera.power_on()) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Power on completed in {:?}", elapsed);
        }
        Err(e) => println!("   ✗ Power on failed: {}", e),
    }

    // Wait for initialization
    std::thread::sleep(Duration::from_secs(2));

    // Move to home
    let start = Instant::now();
    match block_on(camera.home()) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Home command completed in {:?}", elapsed);
        }
        Err(e) => println!("   ✗ Home command failed: {}", e),
    }

    // Set position
    let start = Instant::now();
    match block_on(camera.set_position(Degrees(30.0), Degrees(-10.0))) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Set position completed in {:?}", elapsed);
        }
        Err(e) => println!("   ✗ Set position failed: {}", e),
    }

    // Movement with timing
    println!("\n2. Movement Commands (timed but no timeout):\n");

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
    println!("\n3. Preset Operations (may take significant time):\n");

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

    println!("\n4. Summary:");
    println!("   - Camera API provides type-safe, profile-aware control");
    println!("   - For timeout control, use the unified Client API");
    println!("   - Custom transport wrappers could add timeout support");

    Ok(())
}
