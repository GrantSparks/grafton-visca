//! Multi-camera control using CameraPool.
//!
//! This example shows how to control multiple cameras efficiently,
//! replacing manual Arc<Mutex<Camera>> patterns with CameraPool.

use grafton_visca::{
    camera::{
        profiles::{G2PresetId, PTZOpticsG2},
        units::Degrees,
    },
    camera_pool::{CameraInfo, CameraPool, PoolConfig},
    transport::AsyncUdpTransport,
};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Multi-Camera Control Demo ===");
    println!("Managing multiple PTZ cameras with CameraPool\n");

    // Configure the pool
    let pool_config = PoolConfig {
        health_check_interval: Duration::from_secs(30),
        auto_remove_unhealthy: true,
        max_idle_time: Some(Duration::from_secs(600)), // 10 minutes
        max_cameras: Some(8),                          // Support up to 8 cameras
    };

    let pool = CameraPool::<PTZOpticsG2>::new(pool_config);

    // Define our camera setup
    let camera_configs = vec![
        ("main", "Main Stage Camera", "192.168.1.100:52381"),
        ("left", "Stage Left Camera", "192.168.1.101:52381"),
        ("right", "Stage Right Camera", "192.168.1.102:52381"),
        ("audience", "Audience Camera", "192.168.1.103:52381"),
    ];

    // Add cameras to the pool
    println!("Adding cameras to pool:");
    for (id, name, address) in camera_configs {
        let info = CameraInfo::new(id)
            .with_name(name)
            .with_location("Main Auditorium")
            .with_metadata("ip", address);

        let transport = match AsyncUdpTransport::new(address).await {
            Ok(t) => Box::new(t),
            Err(e) => {
                println!("  ✗ Failed to create transport for {}: {}", name, e);
                continue;
            }
        };

        match pool.add_camera_async(info, transport).await {
            Ok(_) => println!("  ✓ Added: {} ({})", name, id),
            Err(e) => println!("  ✗ Failed to add {}: {}", name, e),
        }
    }

    // Run health check on all cameras
    println!("\nRunning health check on all cameras:");
    let health_results = pool.health_check_all();
    for (id, is_healthy) in health_results {
        println!(
            "  {}: {}",
            id,
            if is_healthy {
                "✓ Healthy"
            } else {
                "✗ Unhealthy"
            }
        );
    }

    // Example 1: Initialize all cameras to home position
    println!("\nExample 1: Moving all cameras to home position...");
    let home_results = pool
        .with_all_cameras_async(|camera| async move { camera.home().await })
        .await;

    for (id, result) in home_results {
        match result {
            Ok(_) => println!("  ✓ {} moved to home", id),
            Err(e) => println!("  ✗ {} failed: {}", id, e),
        }
    }

    // Example 2: Set different positions for each camera
    println!("\nExample 2: Setting unique positions for each camera...");
    let positions: HashMap<&str, (f32, f32)> = HashMap::from([
        ("main", (0.0, 0.0)),         // Center
        ("left", (-45.0, 10.0)),      // Left side, slightly up
        ("right", (45.0, 10.0)),      // Right side, slightly up
        ("audience", (180.0, -20.0)), // Rear facing, down
    ]);

    for (camera_id, (pan, tilt)) in positions {
        let result = pool
            .with_camera_async(camera_id, |camera| async move {
                camera.set_position(Degrees(pan), Degrees(tilt)).await
            })
            .await;

        match result {
            Ok(_) => println!("  ✓ {} moved to ({:.1}°, {:.1}°)", camera_id, pan, tilt),
            Err(e) => println!("  ✗ {} positioning failed: {}", camera_id, e),
        }
    }

    // Example 3: Save current positions as presets
    println!("\nExample 3: Saving current positions as preset 1...");
    let preset_results = pool
        .with_all_cameras_async(|camera| async move {
            camera.set_preset(G2PresetId::new(1).unwrap()).await
        })
        .await;

    let successful = preset_results.values().filter(|r| r.is_ok()).count();
    println!(
        "  ✓ Saved preset 1 on {}/{} cameras",
        successful,
        preset_results.len()
    );

    // Example 4: Coordinated movement sequence
    println!("\nExample 4: Executing coordinated movement sequence...");

    // Phase 1: All cameras look center
    println!("  Phase 1: All cameras to center...");
    let _ = pool
        .with_all_cameras_async(|camera| async move {
            camera.set_position(Degrees(0.0), Degrees(0.0)).await
        })
        .await;
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Phase 2: Fan out pattern
    println!("  Phase 2: Fan out to corners...");
    let fan_positions = HashMap::from([
        ("main", (0.0, -30.0)),     // Look down
        ("left", (-90.0, 0.0)),     // Look far left
        ("right", (90.0, 0.0)),     // Look far right
        ("audience", (180.0, 0.0)), // Look back
    ]);

    for (id, (pan, tilt)) in fan_positions {
        let _ = pool
            .with_camera_async(id, |camera| async move {
                camera.set_position(Degrees(pan), Degrees(tilt)).await
            })
            .await;
    }
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Phase 3: Return to saved positions (preset 1)
    println!("  Phase 3: Return to saved positions...");
    let _ = pool
        .with_all_cameras_async(|camera| async move {
            camera.recall_preset(G2PresetId::new(1).unwrap()).await
        })
        .await;

    // Get final statistics
    println!("\nFinal Pool Statistics:");
    let stats = pool.get_all_stats();
    println!("  Total cameras: {}", stats.len());
    for stat in stats {
        println!(
            "  {}: {} successful, {} failed operations",
            stat.info.id, stat.successful_ops, stat.failed_ops
        );
    }

    // Perform maintenance
    println!("\nPerforming pool maintenance...");
    let report = pool.maintenance();
    if report.total_removed() > 0 {
        println!(
            "  Removed {} unhealthy, {} stale cameras",
            report.unhealthy_removed.len(),
            report.stale_removed.len()
        );
    } else {
        println!("  All cameras healthy and active");
    }

    println!("\nDemo complete!");
    Ok(())
}

// Benefits over manual Arc<Mutex<Camera>> approach:
// 1. Automatic health monitoring and cleanup
// 2. Built-in statistics tracking
// 3. Concurrent operations on all cameras
// 4. Cleaner API with less boilerplate
// 5. Automatic connection management
