//! Example program

//! Example demonstrating timeout handling with async operations.
//!
//! This example shows how to:
//! - Handle timeouts in async operations
//! - Use tokio's timeout utilities with VISCA commands
//! - Implement custom timeout logic for different command types
//! - Recover from timeout errors gracefully

use grafton_visca::command::{
    pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
    preset::{PresetAction, PresetCommand, PresetNumber},
    InquiryCommand, Response,
};
use grafton_visca::{Client, Error};
use std::time::Duration;
use tokio::time::timeout;

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example async_configurable_timeouts --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== VISCA Async Timeout Handling Example ===\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Connect to camera
    println!("Connecting to camera at {}...", camera_addr);
    let client = Client::connect_udp_async(&camera_addr).await?;

    // Demonstrate different timeout scenarios
    demonstrate_quick_timeout(&client).await?;
    demonstrate_movement_timeout(&client).await?;
    demonstrate_preset_timeout(&client).await?;
    demonstrate_timeout_recovery(&client).await?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demonstrate_quick_timeout(client: &Client) -> Result<(), Error> {
    println!("1. Quick Commands with Short Timeout:");
    println!("   Using 1 second timeout for inquiry commands\n");

    // Quick inquiry with short timeout
    let quick_timeout = Duration::from_secs(1);

    // Power inquiry
    match timeout(quick_timeout, client.send_async(&InquiryCommand::Power)).await {
        Ok(Ok(Response::InquiryResponse(resp))) => {
            println!("   ✓ Power inquiry succeeded: {:?}", resp);
        }
        Ok(Ok(Response::Ack)) => {
            println!("   ✓ Power inquiry acknowledged");
        }
        Ok(Ok(Response::Completion)) => {
            println!("   ✓ Power inquiry completed");
        }
        Ok(Ok(Response::Error(e))) => {
            println!("   ✗ Camera returned error: {:?}", e);
        }
        Ok(Ok(Response::Unknown(data))) => {
            println!("   ? Unknown response: {:?}", data);
        }
        Ok(Err(e)) => {
            println!("   ✗ Power inquiry failed: {}", e);
        }
        Err(_) => {
            println!("   ✗ Power inquiry timed out after {:?}", quick_timeout);
        }
    }

    // Position inquiry
    match timeout(
        quick_timeout,
        client.send_async(&InquiryCommand::PanTiltPosition),
    )
    .await
    {
        Ok(Ok(Response::InquiryResponse(resp))) => {
            println!("   ✓ Position inquiry succeeded: {:?}", resp);
        }
        Ok(Ok(_)) => {
            println!("   ✓ Position inquiry completed with unexpected response");
        }
        Ok(Err(e)) => {
            println!("   ✗ Position inquiry failed: {}", e);
        }
        Err(_) => {
            println!("   ✗ Position inquiry timed out after {:?}", quick_timeout);
        }
    }

    println!();
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demonstrate_movement_timeout(client: &Client) -> Result<(), Error> {
    println!("2. Movement Commands with Medium Timeout:");
    println!("   Using 5 second timeout for movement commands\n");

    let movement_timeout = Duration::from_secs(5);

    // Start pan/tilt movement
    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Right,
        pan_speed: PanSpeed::new(10)?,
        tilt_speed: TiltSpeed::new(0)?,
    };

    match timeout(movement_timeout, client.send_async(&move_cmd)).await {
        Ok(Ok(_)) => {
            println!("   ✓ Movement command started successfully");

            // Let it move for a bit
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Stop movement
            let stop_cmd = PanTiltCommand::Move {
                direction: PanTiltDirection::Stop,
                pan_speed: PanSpeed::new(0)?,
                tilt_speed: TiltSpeed::new(0)?,
            };

            match timeout(movement_timeout, client.send_async(&stop_cmd)).await {
                Ok(Ok(_)) => println!("   ✓ Movement stopped"),
                Ok(Err(e)) => println!("   ✗ Stop command failed: {}", e),
                Err(_) => println!("   ✗ Stop command timed out"),
            }
        }
        Ok(Err(e)) => {
            println!("   ✗ Movement command failed: {}", e);
        }
        Err(_) => {
            println!(
                "   ✗ Movement command timed out after {:?}",
                movement_timeout
            );
        }
    }

    println!();
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demonstrate_preset_timeout(client: &Client) -> Result<(), Error> {
    println!("3. Preset Commands with Long Timeout:");
    println!("   Using 30 second timeout for preset operations\n");

    let preset_timeout = Duration::from_secs(30);

    // Recall preset (which may take time to complete movement)
    let preset_cmd = PresetCommand {
        action: PresetAction::Recall,
        preset_number: PresetNumber::new(1)?,
    };

    let start = std::time::Instant::now();
    match timeout(preset_timeout, client.send_async(&preset_cmd)).await {
        Ok(Ok(_)) => {
            let elapsed = start.elapsed();
            println!("   ✓ Preset recalled successfully in {:?}", elapsed);
        }
        Ok(Err(e)) => {
            println!("   ✗ Preset recall failed: {}", e);
        }
        Err(_) => {
            println!("   ✗ Preset recall timed out after {:?}", preset_timeout);
        }
    }

    println!();
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demonstrate_timeout_recovery(client: &Client) -> Result<(), Error> {
    println!("4. Timeout Recovery Strategies:");
    println!("   Demonstrating retry logic with exponential backoff\n");

    // Command that might timeout
    let command = InquiryCommand::ZoomPosition;

    // Retry configuration
    let max_retries = 3;
    let initial_timeout = Duration::from_millis(500);

    for attempt in 1..=max_retries {
        let current_timeout = initial_timeout * attempt as u32;
        println!(
            "   Attempt {}/{} with timeout {:?}",
            attempt, max_retries, current_timeout
        );

        match timeout(current_timeout, client.send_async(&command)).await {
            Ok(Ok(Response::InquiryResponse(resp))) => {
                println!("   ✓ Success on attempt {}: {:?}", attempt, resp);
                return Ok(());
            }
            Ok(Ok(_)) => {
                println!(
                    "   ✓ Command completed on attempt {} with unexpected response",
                    attempt
                );
                return Ok(());
            }
            Ok(Err(e)) => {
                println!("   ✗ Command error on attempt {}: {}", attempt, e);
                if attempt < max_retries {
                    println!("   Retrying...");
                    tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                }
            }
            Err(_) => {
                println!("   ✗ Timeout on attempt {}", attempt);
                if attempt < max_retries {
                    println!("   Retrying with longer timeout...");
                    tokio::time::sleep(Duration::from_millis(100 * attempt as u64)).await;
                }
            }
        }
    }

    println!("   ✗ All retry attempts exhausted");
    println!();
    Ok(())
}
