//! Example demonstrating the async high-level control API for camera operations.

use grafton_visca::{PanTiltDirection, ViscaClient, ViscaError};
use std::env;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), ViscaError> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <camera_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:5678", args[0]);
        std::process::exit(1);
    }

    // Connect to camera
    let camera_addr = &args[1];
    println!("Connecting to camera at {}...", camera_addr);
    let client = ViscaClient::connect_udp_async(camera_addr).await?;

    println!("\n=== Async Camera Control Demo ===\n");

    // Concurrent Operations Example
    println!("1. Concurrent Operations");
    println!("   - Executing multiple queries concurrently...");

    // Start multiple operations concurrently
    let power_future = client.get_power_state();
    let position_future = client.get_pan_tilt_position();
    let zoom_future = client.get_zoom_position();

    let (power, position, zoom) = tokio::join!(power_future, position_future, zoom_future);

    println!("   - Power: {}", if power? { "ON" } else { "OFF" });
    let (pan, tilt) = position?;
    println!("   - Position: pan={}, tilt={}", pan, tilt);
    println!("   - Zoom: 0x{:04X}", zoom?);

    // Sequential Control Operations
    println!("\n2. Sequential Control Operations");

    println!("   - Moving to home position...");
    client.go_home().await?;
    sleep(Duration::from_secs(3)).await;

    println!("   - Setting up shot 1...");
    client.move_to_position(800, -200, Some((15, 15))).await?;
    client.zoom_to(0x1800).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Saving as preset 10...");
    client.save_preset(10).await?;
    sleep(Duration::from_millis(500)).await;

    println!("   - Setting up shot 2...");
    client.move_to_position(-600, 400, None).await?;
    client.zoom_to(0x3000).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Saving as preset 11...");
    client.save_preset(11).await?;
    sleep(Duration::from_millis(500)).await;

    // Smooth Movement Example
    println!("\n3. Smooth Movement Sequence");

    println!("   - Starting smooth pan...");
    client.start_moving(PanTiltDirection::Right, 8, 0).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Adding tilt movement...");
    client.start_moving(PanTiltDirection::UpRight, 8, 5).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Stopping movement...");
    client.stop_movement().await?;

    // Focus and Zoom Coordination
    println!("\n4. Focus and Zoom Coordination");

    println!("   - Setting manual focus mode...");
    client.set_auto_focus(false).await?;

    println!("   - Zooming in while adjusting focus...");
    let zoom_task = tokio::spawn({
        let client = client.clone();
        async move {
            client.zoom_in(Some(3)).await?;
            sleep(Duration::from_secs(3)).await;
            client.stop_zoom().await
        }
    });

    // Adjust focus while zooming
    sleep(Duration::from_millis(500)).await;
    client.focus_far(Some(2)).await?;
    sleep(Duration::from_secs(2)).await;
    client.stop_focus().await?;

    zoom_task.await.unwrap()?;

    println!("   - Enabling auto-focus...");
    client.set_auto_focus(true).await?;

    // Preset Tour Example
    println!("\n5. Preset Tour");
    println!("   - Starting preset tour between positions 10 and 11...");

    for i in 0..3 {
        println!("   - Tour iteration {}", i + 1);

        client.recall_preset(10).await?;
        sleep(Duration::from_secs(3)).await;

        client.recall_preset(11).await?;
        sleep(Duration::from_secs(3)).await;
    }

    println!("   - Returning home...");
    client.go_home().await?;
    sleep(Duration::from_secs(2)).await;

    // Cleanup
    println!("\n6. Cleanup");
    println!("   - Resetting test presets...");
    client.reset_preset(10).await?;
    client.reset_preset(11).await?;

    println!("\n=== Async Demo Complete ===");
    println!("All async control operations executed successfully!");

    Ok(())
}
