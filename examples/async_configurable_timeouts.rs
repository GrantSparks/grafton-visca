//! Example demonstrating timeout handling with async Camera API operations.
//!
//! This example shows how to:
//! - Handle timeouts in async operations with the Camera API
//! - Use tokio's timeout utilities with Camera commands
//! - Implement custom timeout logic for different command types
//! - Recover from timeout errors gracefully
//!
//! Note: The Camera API doesn't have built-in per-command timeout configuration.
//! Timeouts are handled at the transport level or using tokio::time::timeout.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, units::Degrees, Camera},
    command::pan_tilt::PanTiltDirection,
    transport::AsyncUdpTransport,
    Error,
};
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

    println!("=== VISCA Async Timeout Handling with Camera API ===\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Create camera with async transport
    println!("Connecting to camera at {}...", camera_addr);
    let transport = AsyncUdpTransport::new(&camera_addr).await?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    // Demonstrate different timeout scenarios
    demonstrate_quick_timeout(&mut camera).await?;
    demonstrate_movement_timeout(&mut camera).await?;
    demonstrate_preset_timeout(&mut camera).await?;
    demonstrate_timeout_recovery(&mut camera).await?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demonstrate_quick_timeout(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
    println!("1. Quick Commands with Short Timeout:");
    println!("   Using 1 second timeout for status checks\n");

    let quick_timeout = Duration::from_secs(1);

    // Power on with timeout
    match timeout(quick_timeout, camera.power_on()).await {
        Ok(Ok(_)) => {
            println!("   ✓ Power on command succeeded");
        }
        Ok(Err(e)) => {
            println!("   ✗ Power on command failed: {}", e);
        }
        Err(_) => {
            println!("   ✗ Power on command timed out after {:?}", quick_timeout);
        }
    }

    // Home position with timeout
    match timeout(quick_timeout, camera.home()).await {
        Ok(Ok(_)) => {
            println!("   ✓ Home command succeeded");
        }
        Ok(Err(e)) => {
            println!("   ✗ Home command failed: {}", e);
        }
        Err(_) => {
            println!("   ✗ Home command timed out after {:?}", quick_timeout);
        }
    }

    println!();
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demonstrate_movement_timeout(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
    println!("2. Movement Commands with Medium Timeout:");
    println!("   Using 5 second timeout for movement commands\n");

    let movement_timeout = Duration::from_secs(5);

    // Start pan/tilt movement
    match timeout(
        movement_timeout,
        camera.move_continuous(PanTiltDirection::Right, 10, 0),
    )
    .await
    {
        Ok(Ok(_)) => {
            println!("   ✓ Movement command started successfully");

            // Let it move for a bit
            tokio::time::sleep(Duration::from_secs(2)).await;

            // Stop movement
            match timeout(movement_timeout, camera.stop()).await {
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
async fn demonstrate_preset_timeout(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
    println!("3. Preset Commands with Long Timeout:");
    println!("   Using 30 second timeout for preset operations\n");

    let preset_timeout = Duration::from_secs(30);

    // Create a preset ID
    use grafton_visca::camera::profiles::G2PresetId;
    let preset = G2PresetId::new(1)?;

    // Recall preset (which may take time to complete movement)
    let start = std::time::Instant::now();
    match timeout(preset_timeout, camera.recall_preset(preset)).await {
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
async fn demonstrate_timeout_recovery(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
    println!("4. Timeout Recovery Strategies:");
    println!("   Demonstrating retry logic with exponential backoff\n");

    // Retry configuration
    let max_retries = 3;
    let initial_timeout = Duration::from_millis(500);

    // Try to set position with retries
    let target_pan = Degrees(45.0);
    let target_tilt = Degrees(15.0);

    for attempt in 1..=max_retries {
        let current_timeout = initial_timeout * attempt as u32;
        println!(
            "   Attempt {}/{} with timeout {:?}",
            attempt, max_retries, current_timeout
        );

        match timeout(
            current_timeout,
            camera.set_position(target_pan, target_tilt),
        )
        .await
        {
            Ok(Ok(_)) => {
                println!("   ✓ Success on attempt {}: moved to position", attempt);
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

    // Demonstrate alternative: AsyncTcpTransport has hardcoded 10s timeout
    println!("\n5. Transport-Level Timeout Notes:");
    println!("   AsyncTcpTransport uses a hardcoded 10-second timeout");
    println!("   For custom timeouts, wrap operations with tokio::time::timeout");

    #[cfg(feature = "async-client")]
    {
        use grafton_visca::transport::AsyncTcpTransport;

        // AsyncTcpTransport has a fixed 10s timeout
        match AsyncTcpTransport::new("192.168.1.100:5678").await {
            Ok(transport) => {
                println!("   ✓ Created TCP transport (10s timeout)");
                let mut tcp_camera = Camera::<PTZOpticsG2>::new(transport);

                // For custom timeout, wrap the operation
                let custom_timeout = Duration::from_secs(30);
                match timeout(custom_timeout, tcp_camera.power_on()).await {
                    Ok(Ok(_)) => println!("   ✓ TCP camera powered on"),
                    Ok(Err(e)) => println!("   ✗ TCP camera error: {}", e),
                    Err(_) => println!("   ✗ Operation timed out after {:?}", custom_timeout),
                }
            }
            Err(e) => {
                println!("   ✗ Failed to create TCP transport: {}", e);
            }
        }
    }

    println!("\n   Note: For configurable transport-level timeouts, you would need");
    println!("   to use the Client API or implement a custom transport wrapper.");

    println!();
    Ok(())
}
