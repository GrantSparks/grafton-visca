// TODO: Update this example for v0.5.0 - async support is not yet available
fn main() {
    println!("This example needs to be updated for v0.5.0");
    println!("Async support is not yet available in the current version");
}

/*
//! Example demonstrating async connection pool for managing multiple cameras.
//!
//! This example shows how to use the AsyncViscaConnectionPool to manage connections
//! to multiple PTZ cameras asynchronously.

use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        power::{Power, PowerCommand},
        ZoomCommand,
    },
    AsyncCameraInfo, AsyncPoolConfig, AsyncTcpTransport, AsyncUdpTransport,
    AsyncViscaConnectionPool, AsyncViscaTransport, ReconnectionConfig, Error,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Async VISCA Connection Pool Example ===\n");

    // Example 1: Basic async pool
    demo_basic_async_pool().await?;

    println!("\n=== Async Pool with Maintenance Task ===\n");
    demo_maintenance_task().await?;

    println!("\n=== Concurrent Async Control ===\n");
    demo_concurrent_async_control().await?;

    Ok(())
}

async fn demo_basic_async_pool() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the pool
    let pool_config = AsyncPoolConfig {
        reconnection_config: ReconnectionConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            backoff_factor: 2.0,
            health_check_interval: Some(Duration::from_secs(30)),
        },
        health_check_interval: Duration::from_secs(60),
        auto_remove_unhealthy: false,
        max_idle_time: Some(Duration::from_secs(300)),
    };

    // Create pool with async UDP transport factory
    let pool = AsyncViscaConnectionPool::new(pool_config, |addr: &str| {
        let addr = addr.to_string();
        async move {
            use std::net::ToSocketAddrs;
            let socket_addr = addr
                .to_socket_addrs()
                .map_err(Error::Io)?
                .next()
                .ok_or_else(|| Error::InvalidParameter("Invalid address".to_string()))?;
            AsyncUdpTransport::new(socket_addr).await
        }
    });

    // Add cameras to the pool
    let cameras = vec![
        ("cam1", "192.168.1.100:1259", "Front Camera", "Main Stage"),
        ("cam2", "192.168.1.101:1259", "Side Camera", "Stage Left"),
        ("cam3", "192.168.1.102:1259", "Rear Camera", "Back Wall"),
    ];

    for (id, addr, name, location) in cameras {
        let info = AsyncCameraInfo {
            id: id.to_string(),
            name: Some(name.to_string()),
            model: Some("PTZOptics G2".to_string()),
            location: Some(location.to_string()),
        };

        match pool.add_camera(id, addr, info).await {
            Ok(()) => println!("✓ Added camera '{}' at {}", name, addr),
            Err(e) => println!("✗ Failed to add camera '{}': {}", name, e),
        }
    }

    // List all cameras
    println!("\nCameras in pool:");
    for camera_id in pool.list_cameras().await {
        println!("  - {}", camera_id);
    }

    // Control each camera
    println!("\nControlling cameras:");
    for camera_id in pool.list_cameras().await {
        match pool.get_connection(&camera_id).await {
            Ok(conn) => {
                let mut transport = conn.transport().await;

                // Power on
                match transport
                    .send_command(&PowerCommand { power: Power::On })
                    .await
                {
                    Ok(_) => println!("  ✓ {} powered on", camera_id),
                    Err(e) => println!("  ✗ {} power on failed: {}", camera_id, e),
                }

                // Home position
                match transport.send_command(&PanTiltCommand::Home).await {
                    Ok(_) => println!("  ✓ {} moved to home position", camera_id),
                    Err(e) => println!("  ✗ {} home command failed: {}", camera_id, e),
                }
            }
            Err(e) => println!("  ✗ Failed to get connection for {}: {}", camera_id, e),
        }
    }

    // Get pool statistics
    println!("\nPool Statistics:");
    for stats in pool.get_all_stats().await {
        println!(
            "\n  Camera: {} ({})",
            stats.info.id,
            stats.info.name.as_deref().unwrap_or("Unknown")
        );
        println!("    Healthy: {}", stats.is_healthy);
        let snapshot = stats.connection_stats.snapshot();
        println!("    Commands sent: {}", snapshot.commands_sent);
        println!("    Errors: {}", snapshot.error_count);
    }

    Ok(())
}

async fn demo_maintenance_task() -> Result<(), Box<dyn std::error::Error>> {
    let pool_config = AsyncPoolConfig {
        reconnection_config: ReconnectionConfig::default(),
        health_check_interval: Duration::from_secs(5),
        auto_remove_unhealthy: true,
        max_idle_time: Some(Duration::from_secs(30)),
    };

    // Create pool with TCP transport
    let pool = Arc::new(AsyncViscaConnectionPool::new(pool_config, |addr: &str| {
        let addr = addr.to_string();
        async move {
            use std::net::ToSocketAddrs;
            let socket_addr = addr
                .to_socket_addrs()
                .map_err(Error::Io)?
                .next()
                .ok_or_else(|| Error::InvalidParameter("Invalid address".to_string()))?;
            AsyncTcpTransport::new(socket_addr).await
        }
    }));

    // Add cameras
    for i in 1..=2 {
        let info = AsyncCameraInfo {
            id: format!("tcp_cam{}", i),
            name: Some(format!("TCP Camera {}", i)),
            model: None,
            location: None,
        };
        pool.add_camera(
            format!("tcp_cam{}", i),
            &format!("192.168.1.10{}:5678", i),
            info,
        )
        .await?;
    }

    // Start maintenance task
    let maintenance_handle = pool.start_maintenance_task();
    println!("Started background maintenance task");

    // Simulate usage
    println!("\nSimulating camera usage...");
    for _ in 0..3 {
        // Use first camera
        if let Ok(conn) = pool.get_connection("tcp_cam1").await {
            let mut transport = conn.transport().await;
            let _ = transport.send_command(&ZoomCommand::WideStandard).await;
            println!("  Used tcp_cam1");
        }

        sleep(Duration::from_secs(3)).await;

        // Check pool health
        let health = pool.health_check_all().await;
        println!("  Health check: {:?}", health);
    }

    // Let cam2 become stale by not using it
    println!("\nLetting tcp_cam2 become stale...");
    sleep(Duration::from_secs(15)).await;

    // Check which cameras are still in the pool
    println!("\nRemaining cameras after idle timeout:");
    for camera_id in pool.list_cameras().await {
        println!("  - {}", camera_id);
    }

    // Clean up
    maintenance_handle.abort();
    println!("\nMaintenance task stopped");

    Ok(())
}

async fn demo_concurrent_async_control() -> Result<(), Box<dyn std::error::Error>> {
    let pool_config = AsyncPoolConfig::default();
    let pool = Arc::new(AsyncViscaConnectionPool::new(pool_config, |addr: &str| {
        let addr = addr.to_string();
        async move {
            use std::net::ToSocketAddrs;
            let socket_addr = addr
                .to_socket_addrs()
                .map_err(Error::Io)?
                .next()
                .ok_or_else(|| Error::InvalidParameter("Invalid address".to_string()))?;
            AsyncUdpTransport::new(socket_addr).await
        }
    }));

    // Add three cameras
    for i in 1..=3 {
        let info = AsyncCameraInfo {
            id: format!("cam{}", i),
            name: Some(format!("Camera {}", i)),
            model: None,
            location: None,
        };
        pool.add_camera(
            format!("cam{}", i),
            &format!("192.168.1.10{}:1259", i),
            info,
        )
        .await?;
    }

    println!("Controlling multiple cameras concurrently...\n");

    // Control all cameras simultaneously using async tasks
    let mut tasks = vec![];

    for camera_id in ["cam1", "cam2", "cam3"] {
        let pool_clone = pool.clone();
        let cam_id = camera_id.to_string();

        let task = tokio::spawn(async move {
            println!("[{}] Starting control sequence", cam_id);

            match pool_clone.get_connection(&cam_id).await {
                Ok(conn) => {
                    let mut transport = conn.transport().await;

                    // Power on
                    match transport
                        .send_command(&PowerCommand { power: Power::On })
                        .await
                    {
                        Ok(_) => println!("[{}] ✓ Powered on", cam_id),
                        Err(e) => println!("[{}] ✗ Power on failed: {}", cam_id, e),
                    }

                    sleep(Duration::from_millis(500)).await;

                    // Pan left and right
                    for direction in [PanTiltDirection::Left, PanTiltDirection::Right] {
                        match transport
                            .send_command(&PanTiltCommand::Move {
                                direction,
                                pan_speed: PanSpeed::new(10).unwrap(),
                                tilt_speed: TiltSpeed::new(0).unwrap(),
                            })
                            .await
                        {
                            Ok(_) => println!("[{}] ✓ Panned {:?}", cam_id, direction),
                            Err(e) => println!("[{}] ✗ Pan failed: {}", cam_id, e),
                        }

                        sleep(Duration::from_secs(1)).await;
                    }

                    // Zoom
                    match transport.send_command(&ZoomCommand::TeleStandard).await {
                        Ok(_) => println!("[{}] ✓ Zoomed in", cam_id),
                        Err(e) => println!("[{}] ✗ Zoom failed: {}", cam_id, e),
                    }

                    sleep(Duration::from_secs(1)).await;

                    // Return home
                    match transport.send_command(&PanTiltCommand::Home).await {
                        Ok(_) => println!("[{}] ✓ Returned home", cam_id),
                        Err(e) => println!("[{}] ✗ Home failed: {}", cam_id, e),
                    }

                    println!("[{}] Control sequence complete", cam_id);
                }
                Err(e) => println!("[{}] ✗ Failed to get connection: {}", cam_id, e),
            }
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        let _ = task.await;
    }

    println!("\nAll concurrent operations complete!");

    // Final statistics
    println!("\nFinal Pool Statistics:");
    for stats in pool.get_all_stats().await {
        let snapshot = stats.connection_stats.snapshot();
        println!(
            "\n  {}: {} commands, {} errors",
            stats.info.id, snapshot.commands_sent, snapshot.error_count
        );
    }

    // Demonstrate graceful shutdown
    println!("\nShutting down pool...");

    // Remove all cameras
    for camera_id in pool.list_cameras().await {
        pool.remove_camera(&camera_id).await;
    }

    println!("Pool shutdown complete");

    Ok(())
}
*/
