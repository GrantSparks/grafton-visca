//! Example program

//! Example demonstrating timeout handling with blocking operations.
//!
//! This example shows how to:
//! - Use the send_with_timeout method
//! - Implement custom timeout logic for different command types
//! - Handle timeout errors gracefully
//! - Retry commands with increasing timeouts

#[cfg(feature = "blocking-client")]
use grafton_visca::command::{
    pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
    preset::{PresetAction, PresetCommand, PresetNumber},
    InquiryCommand, Response,
};
#[cfg(feature = "blocking-client")]
use grafton_visca::{Client, Error};
#[cfg(feature = "blocking-client")]
use std::time::{Duration, Instant};

#[cfg(not(feature = "blocking-client"))]
fn main() {
    eprintln!("This example requires the 'blocking-client' feature.");
    eprintln!("Run with: cargo run --example configurable_timeouts --features blocking-client");
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== VISCA Timeout Handling Example ===\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Connect to camera
    println!("Connecting to camera at {}...", camera_addr);
    let client = Client::connect_udp(&camera_addr)?;

    // Demonstrate different timeout scenarios
    demonstrate_quick_commands(&client)?;
    demonstrate_movement_commands(&client)?;
    demonstrate_preset_commands(&client)?;
    demonstrate_retry_with_timeout(&client)?;

    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demonstrate_quick_commands(client: &Client) -> Result<(), Error> {
    println!("1. Quick Commands (Inquiries):");
    println!("   These commands should complete quickly\n");

    // Quick timeout for inquiry commands
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

    // Zoom position inquiry
    let start = Instant::now();
    match client.send_with_timeout(&InquiryCommand::ZoomPosition, quick_timeout) {
        Ok(Response::InquiryResponse(resp)) => {
            let elapsed = start.elapsed();
            println!("   ✓ Zoom inquiry completed in {:?}: {:?}", elapsed, resp);
        }
        Ok(_) => println!("   ✓ Zoom inquiry completed but unexpected response"),
        Err(e) => println!("   ✗ Zoom inquiry failed: {}", e),
    }

    println!();
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demonstrate_movement_commands(client: &Client) -> Result<(), Error> {
    println!("2. Movement Commands:");
    println!("   These commands may take longer to acknowledge\n");

    // Medium timeout for movement commands
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
            std::thread::sleep(Duration::from_secs(2));

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
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demonstrate_preset_commands(client: &Client) -> Result<(), Error> {
    println!("3. Preset Commands:");
    println!("   These commands may take significant time to complete\n");

    // Long timeout for preset operations
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

    // Recall preset (this typically takes longer)
    let recall_preset = PresetCommand {
        action: PresetAction::Recall,
        preset_number: PresetNumber::new(1)?,
    };

    let start = Instant::now();
    match client.send_with_timeout(&recall_preset, preset_timeout) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("   ✓ Preset recalled in {:?}", elapsed);
        }
        Err(e) => {
            println!("   ✗ Preset recall failed: {}", e);
        }
    }

    println!();
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn demonstrate_retry_with_timeout(client: &Client) -> Result<(), Error> {
    println!("4. Retry with Increasing Timeouts:");
    println!("   Demonstrating adaptive timeout strategy\n");

    // Command that might need retries
    let command = InquiryCommand::FocusPosition;

    // Retry configuration
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
                return Ok(());
            }
            Ok(_) => {
                println!("   ✓ Command succeeded but unexpected response");
                return Ok(());
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

    println!("   ✗ All retry attempts exhausted");
    println!();
    Ok(())
}
