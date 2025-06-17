//! Example demonstrating concurrent async command execution with grafton-visca
//!
//! This example shows how to:
//! - Connect to a camera using async API with the new `Camera<P>` system
//! - Send multiple commands concurrently using camera methods
//! - Handle the two-socket limitation gracefully
//! - Process responses asynchronously
//! - Maximize throughput with concurrent operations

use grafton_visca::{
    camera::profiles::PTZOpticsG2, command::pan_tilt::PanTiltDirection,
    transport::AsyncUdpTransport, Camera, Error,
};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example async_concurrent --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line or environment
    let camera_addr = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("CAMERA_IP").ok())
        .unwrap_or_else(|| "192.168.0.100:5678".to_string());

    println!("Connecting to camera at {}...", camera_addr);
    let transport = AsyncUdpTransport::new(&camera_addr).await?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    // Example 1: Sequential commands with timing
    println!("\n=== Sequential Command Execution ===");
    let start = Instant::now();

    // Start moving the camera
    camera
        .move_continuous(PanTiltDirection::UpRight, 0x10, 0x10)
        .await?;
    let move_time = start.elapsed();

    // Start zooming
    camera.zoom_in().await?;
    let zoom_time = start.elapsed();

    println!("Move command completed in {:?}", move_time);
    println!("Zoom command completed in {:?}", zoom_time);
    println!("Total time: {:?}", start.elapsed());

    // Wait a moment then stop both
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Stop movement and zoom
    let stop_start = Instant::now();
    camera.stop().await?;
    camera.zoom_stop().await?;
    println!("Stop commands completed in {:?}", stop_start.elapsed());

    // Example 2: True concurrent operations using Arc<Mutex<Camera>>
    println!("\n=== True Concurrent Operations ===");

    // Wrap camera in Arc<Mutex> for concurrent access
    let camera = Arc::new(Mutex::new(camera));
    let start = Instant::now();

    // Create concurrent tasks that can access the camera
    let camera1 = Arc::clone(&camera);
    let move_task = tokio::spawn(async move {
        let mut cam = camera1.lock().await;
        cam.move_continuous(PanTiltDirection::Right, 0x08, 0).await
    });

    let camera2 = Arc::clone(&camera);
    let zoom_task = tokio::spawn(async move {
        let mut cam = camera2.lock().await;
        cam.zoom_in().await
    });

    // Wait for both tasks to complete
    let (move_result, zoom_result) = tokio::join!(move_task, zoom_task);

    println!("Concurrent operations completed in {:?}", start.elapsed());
    println!(
        "Move result: {:?}",
        move_result
            .map_err(|e| e.to_string())
            .map(|r| r.map_err(|e| e.to_string()))
            .unwrap_or_else(Err)
    );
    println!(
        "Zoom result: {:?}",
        zoom_result
            .map_err(|e| e.to_string())
            .map(|r| r.map_err(|e| e.to_string()))
            .unwrap_or_else(Err)
    );

    // Wait and then stop
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Stop operations concurrently
    let camera1 = Arc::clone(&camera);
    let stop_move = tokio::spawn(async move {
        let mut cam = camera1.lock().await;
        cam.stop().await
    });

    let camera2 = Arc::clone(&camera);
    let stop_zoom = tokio::spawn(async move {
        let mut cam = camera2.lock().await;
        cam.zoom_stop().await
    });

    let _ = tokio::join!(stop_move, stop_zoom);

    // Example 3: Complex concurrent sequence with inquiries
    println!("\n=== Complex Concurrent Sequence ===");

    // For inquiry commands, we need to access the transport directly
    // Save preset position using a type-safe preset ID
    let preset_task = {
        let camera = Arc::clone(&camera);
        tokio::spawn(async move {
            let mut cam = camera.lock().await;
            use grafton_visca::camera::profiles::G2PresetId;
            cam.set_preset(G2PresetId::new(1).unwrap()).await
        })
    };

    // Use the camera's inquiry methods
    let inquiry_task = {
        let camera = Arc::clone(&camera);
        tokio::spawn(async move {
            let mut cam = camera.lock().await;
            // Use camera's built-in inquiry methods
            cam.get_zoom_position().await
        })
    };

    let (preset_result, inquiry_result) = tokio::join!(preset_task, inquiry_task);

    match preset_result {
        Ok(Ok(_)) => println!("Preset saved successfully"),
        Ok(Err(e)) => println!("Failed to save preset: {}", e),
        Err(e) => println!("Task failed: {}", e),
    }

    match inquiry_result {
        Ok(Ok(zoom_pos)) => {
            println!("Current zoom position: 0x{:04X}", zoom_pos);
        }
        Ok(Err(e)) => println!("Failed to query zoom position: {}", e),
        Err(e) => println!("Inquiry task failed: {}", e),
    }

    // Example 4: Maximizing throughput with many operations
    println!("\n=== Maximum Throughput Test ===");
    let start = Instant::now();

    // Create many concurrent tasks
    let mut tasks = Vec::new();
    for i in 0..10 {
        let camera = Arc::clone(&camera);
        let task = tokio::spawn(async move {
            let start = Instant::now();
            let mut cam = camera.lock().await;
            // Alternate between different commands
            let result = if i % 2 == 0 {
                cam.zoom_in().await
            } else {
                cam.zoom_out().await
            };
            let duration = start.elapsed();
            drop(cam); // Release lock immediately
            (result, duration)
        });
        tasks.push(task);
    }

    // Wait for all tasks to complete
    let results = futures_util::future::join_all(tasks).await;

    let elapsed = start.elapsed();
    let successful = results
        .iter()
        .filter(|r| r.as_ref().map(|(res, _)| res.is_ok()).unwrap_or(false))
        .count();

    println!("Sent 10 commands in {:?}", elapsed);
    println!("Successful: {}/10", successful);
    println!("Average time per command: {:?}", elapsed / 10);

    // Stop any ongoing zoom
    camera.lock().await.zoom_stop().await?;

    // Example 5: Movement coordination
    println!("\n=== Coordinated Movement ===");

    // Move to home while setting focus to auto
    let home_task = {
        let camera = Arc::clone(&camera);
        tokio::spawn(async move {
            let mut cam = camera.lock().await;
            cam.home().await
        })
    };

    let focus_task = {
        let camera = Arc::clone(&camera);
        tokio::spawn(async move {
            let mut cam = camera.lock().await;
            cam.focus_auto().await
        })
    };

    let (home_result, focus_result) = tokio::join!(home_task, focus_task);

    println!(
        "Home command: {:?}",
        home_result.map(|r| r.is_ok()).unwrap_or(false)
    );
    println!(
        "Auto focus: {:?}",
        focus_result.map(|r| r.is_ok()).unwrap_or(false)
    );

    // Wait for movement to complete
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Now do a complex movement pattern
    println!("\nExecuting movement pattern...");

    // Pan right while zooming in - true concurrent execution
    let pan_task = {
        let camera = Arc::clone(&camera);
        tokio::spawn(async move {
            let mut cam = camera.lock().await;
            cam.move_continuous(PanTiltDirection::Right, 0x08, 0).await
        })
    };

    let zoom_task = {
        let camera = Arc::clone(&camera);
        tokio::spawn(async move {
            let mut cam = camera.lock().await;
            // Use variable zoom if supported by profile
            cam.zoom_in().await
        })
    };

    let (pan_result, zoom_result) = tokio::join!(pan_task, zoom_task);

    if let Err(e) = pan_result {
        println!("Pan task failed: {}", e);
    }
    if let Err(e) = zoom_result {
        println!("Zoom task failed: {}", e);
    }

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Stop all movement concurrently
    let stop_pan_task = {
        let camera = Arc::clone(&camera);
        tokio::spawn(async move {
            let mut cam = camera.lock().await;
            cam.stop().await
        })
    };

    let stop_zoom_task = {
        let camera = Arc::clone(&camera);
        tokio::spawn(async move {
            let mut cam = camera.lock().await;
            cam.zoom_stop().await
        })
    };

    let (stop_pan_result, stop_zoom_result) = tokio::join!(stop_pan_task, stop_zoom_task);

    if let Err(e) = stop_pan_result {
        println!("Stop pan task failed: {}", e);
    }
    if let Err(e) = stop_zoom_result {
        println!("Stop zoom task failed: {}", e);
    }

    println!("\nConcurrent operations demo completed!");
    println!("\nKey points demonstrated:");
    println!("- Sequential operations for timing measurements");
    println!("- True concurrent execution using Arc<Mutex<Camera>>");
    println!("- Accessing transport directly for raw commands");
    println!("- Type-safe preset IDs with camera profiles");
    println!("- Concurrent task management with proper error handling");

    Ok(())
}
