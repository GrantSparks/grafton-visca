//! Connection pool example demonstrating multi-camera management.
//!
//! This example shows how to use the `ViscaConnectionPool` to manage
//! multiple cameras with the new unified `ViscaClient` architecture.

#[cfg(not(feature = "blocking-client"))]
fn main() {
    println!("This example requires the 'blocking-client' feature to be enabled.");
    println!("Try running with: cargo run --example connection_pool --features blocking-client");
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        command::pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        command::power::{Power, PowerCommand},
        command::zoom::ZoomCommand,
        connection_pool::{CameraInfo, ConnectionType, PoolConfig, ViscaConnectionPool},
    };
    use std::time::Duration;

    env_logger::init();

    println!("=== VISCA Connection Pool Example (v0.5.0) ===\n");

    // Configure the connection pool
    let config = PoolConfig {
        health_check_interval: Duration::from_secs(30),
        auto_remove_unhealthy: true,
        max_idle_time: Some(Duration::from_secs(120)),
    };

    // Create the connection pool
    let pool = ViscaConnectionPool::new(config);

    // Add multiple cameras to the pool
    println!("Adding cameras to the pool...");

    // Camera 1: Front camera
    match pool.add_camera(
        "front",
        "192.168.1.100:5678",
        ConnectionType::Udp,
        CameraInfo {
            id: "front".to_string(),
            name: Some("Front Stage Camera".to_string()),
            model: Some("PTZOptics G2".to_string()),
            location: Some("Main Stage Front".to_string()),
        },
    ) {
        Ok(()) => println!("✓ Added front camera"),
        Err(e) => println!("✗ Failed to add front camera: {e}"),
    }

    // Camera 2: Rear camera
    match pool.add_camera(
        "rear",
        "192.168.1.101:5678",
        ConnectionType::Udp,
        CameraInfo {
            id: "rear".to_string(),
            name: Some("Rear Camera".to_string()),
            model: Some("PTZOptics G2".to_string()),
            location: Some("Back of Auditorium".to_string()),
        },
    ) {
        Ok(()) => println!("✓ Added rear camera"),
        Err(e) => println!("✗ Failed to add rear camera: {e}"),
    }

    // Camera 3: Side camera using TCP
    match pool.add_camera(
        "side",
        "192.168.1.102:5678",
        ConnectionType::Tcp,
        CameraInfo {
            id: "side".to_string(),
            name: Some("Side Camera".to_string()),
            model: Some("PTZOptics G2".to_string()),
            location: Some("Stage Right".to_string()),
        },
    ) {
        Ok(()) => println!("✓ Added side camera (TCP)"),
        Err(e) => println!("✗ Failed to add side camera: {e}"),
    }

    // List all cameras
    println!("\nCameras in pool:");
    for camera_id in pool.list_cameras() {
        println!("  - {camera_id}");
    }

    // Power on all cameras
    println!("\nPowering on cameras...");
    let power_on = PowerCommand { power: Power::On };
    for camera_id in pool.list_cameras() {
        match pool.execute_command(&camera_id, &power_on) {
            Ok(_) => println!("  {camera_id} - Powered on"),
            Err(e) => println!("  {camera_id} - Power on failed: {e}"),
        }
    }

    // Control multiple cameras
    println!("\nControlling cameras...");

    // Pan all cameras to the left
    let pan_left = PanTiltCommand::Move {
        direction: PanTiltDirection::Left,
        pan_speed: PanSpeed::new(10).unwrap(),
        tilt_speed: TiltSpeed::new(0).unwrap(),
    };

    for camera_id in pool.list_cameras() {
        match pool.execute_command(&camera_id, &pan_left) {
            Ok(_) => println!("  {camera_id} - Panning left"),
            Err(e) => println!("  {camera_id} - Error: {e}"),
        }
    }

    // Wait a bit
    std::thread::sleep(Duration::from_secs(2));

    // Stop all cameras
    let stop = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0).unwrap(),
        tilt_speed: TiltSpeed::new(0).unwrap(),
    };
    for camera_id in pool.list_cameras() {
        let _ = pool.execute_command(&camera_id, &stop);
    }

    // Zoom in on the front camera only
    if pool.list_cameras().contains(&"front".to_string()) {
        println!("\nZooming front camera...");
        let zoom_in = ZoomCommand::TeleStandard;
        pool.execute_command("front", &zoom_in)?;

        std::thread::sleep(Duration::from_secs(1));

        let zoom_stop = ZoomCommand::Stop;
        pool.execute_command("front", &zoom_stop)?;
    }

    // Check health of all cameras
    println!("\nChecking camera health...");
    let health_results = pool.health_check_all();
    for (camera_id, is_healthy) in health_results {
        println!(
            "  {} - {}",
            camera_id,
            if is_healthy { "Healthy" } else { "Unhealthy" }
        );
    }

    // Get statistics for all cameras
    println!("\nCamera statistics:");
    for stats in pool.get_all_stats() {
        println!(
            "  {} ({}):",
            stats.info.id,
            stats.info.name.as_deref().unwrap_or("Unknown")
        );
        println!("    - Healthy: {}", stats.is_healthy);
        println!("    - Last used: {:?} ago", stats.last_used.elapsed());
        if let Some(location) = &stats.info.location {
            println!("    - Location: {location}");
        }
    }

    // Remove unhealthy cameras if configured
    let removed = pool.remove_unhealthy();
    if removed.is_empty() {
        println!("\nNo unhealthy cameras to remove");
    } else {
        println!("\nRemoved unhealthy cameras: {removed:?}");
    }

    // Remove a specific camera
    if pool.list_cameras().contains(&"side".to_string()) {
        println!("\nRemoving side camera...");
        if let Some(info) = pool.remove_camera("side") {
            println!(
                "Removed camera: {} at {}",
                info.name.unwrap_or_else(|| "Unknown".to_string()),
                info.location.unwrap_or_else(|| "Unknown".to_string())
            );
        }
    }

    // Final camera list
    println!("\nFinal cameras in pool:");
    for camera_id in pool.list_cameras() {
        println!("  - {camera_id}");
    }

    Ok(())
}

/* Old v0.4.0 example code for reference:
use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        power::{Power, PowerCommand},
        ZoomCommand,
    },
    CameraInfo, PoolConfig, ReconnectionConfig, TcpTransport, UdpTransport, ViscaConnectionPool,
    ViscaError, ViscaTransport, ViscaTransportExt,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== VISCA Connection Pool Example ===\n");

    // Example 1: Basic pool with UDP cameras
    demo_basic_pool()?;

    println!("\n=== Pool with TCP Cameras ===\n");
    demo_tcp_pool()?;

    println!("\n=== Pool Health Management ===\n");
    demo_health_management()?;

    println!("\n=== Concurrent Camera Control ===\n");
    demo_concurrent_control()?;

    Ok(())
}

fn demo_basic_pool() -> Result<(), Box<dyn std::error::Error>> {
    // Configure the pool
    let pool_config = PoolConfig {
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

    // Create pool with UDP transport factory
    let pool = ViscaConnectionPool::new(pool_config, |addr| {
        UdpTransport::new(addr).map_err(ViscaError::Io)
    });

    // Add cameras to the pool
    let cameras = vec![
        ("cam1", "192.168.1.100:1259", "Front Camera", "Main Stage"),
        ("cam2", "192.168.1.101:1259", "Side Camera", "Stage Left"),
        ("cam3", "192.168.1.102:1259", "Rear Camera", "Back Wall"),
    ];

    for (id, addr, name, location) in cameras {
        let info = CameraInfo {
            id: id.to_string(),
            name: Some(name.to_string()),
            model: Some("PTZOptics G2".to_string()),
            location: Some(location.to_string()),
        };

        match pool.add_camera(id, addr, info) {
            Ok(()) => println!("✓ Added camera '{}' at {}", name, addr),
            Err(e) => println!("✗ Failed to add camera '{}': {}", name, e),
        }
    }

    // List all cameras
    println!("\nCameras in pool:");
    for camera_id in pool.list_cameras() {
        println!("  - {}", camera_id);
    }

    // Control each camera
    println!("\nControlling cameras:");
    for camera_id in pool.list_cameras() {
        match pool.get_connection(&camera_id) {
            Ok(conn) => {
                let mut transport = conn.transport();

                // Power on
                match transport.send_and_wait(&PowerCommand { power: Power::On }) {
                    Ok(_) => println!("  ✓ {} powered on", camera_id),
                    Err(e) => println!("  ✗ {} power on failed: {}", camera_id, e),
                }

                // Home position
                match transport.home() {
                    Ok(_) => println!("  ✓ {} moved to home position", camera_id),
                    Err(e) => println!("  ✗ {} home command failed: {}", camera_id, e),
                }
            }
            Err(e) => println!("  ✗ Failed to get connection for {}: {}", camera_id, e),
        }
    }

    // Get pool statistics
    println!("\nPool Statistics:");
    for stats in pool.get_all_stats() {
        println!(
            "\n  Camera: {} ({})",
            stats.info.id,
            stats.info.name.as_deref().unwrap_or("Unknown")
        );
        println!("    Healthy: {}", stats.is_healthy);
        println!(
            "    Commands sent: {}",
            stats.connection_stats.snapshot().commands_sent
        );
        println!(
            "    Errors: {}",
            stats.connection_stats.snapshot().error_count
        );
    }

    Ok(())
}

fn demo_tcp_pool() -> Result<(), Box<dyn std::error::Error>> {
    let pool_config = PoolConfig::default();

    // Create pool with TCP transport factory
    let pool = ViscaConnectionPool::new(pool_config, |addr| {
        TcpTransport::new(addr).map_err(ViscaError::Io)
    });

    // Add TCP cameras
    pool.add_camera(
        "tcp_cam1",
        "192.168.1.100:5678",
        CameraInfo {
            id: "tcp_cam1".to_string(),
            name: Some("TCP Camera 1".to_string()),
            model: None,
            location: Some("Studio A".to_string()),
        },
    )?;

    pool.add_camera(
        "tcp_cam2",
        "192.168.1.101:5678",
        CameraInfo {
            id: "tcp_cam2".to_string(),
            name: Some("TCP Camera 2".to_string()),
            model: None,
            location: Some("Studio B".to_string()),
        },
    )?;

    // Perform preset operations
    println!("Saving presets on TCP cameras:");

    for (idx, camera_id) in pool.list_cameras().iter().enumerate() {
        match pool.get_connection(camera_id) {
            Ok(conn) => {
                let mut transport = conn.transport();

                // Save current position as preset
                let preset_num = (idx + 1) as u8;
                match transport.save_preset(preset_num) {
                    Ok(_) => println!("  ✓ {} saved preset {}", camera_id, preset_num),
                    Err(e) => println!("  ✗ {} preset save failed: {}", camera_id, e),
                }
            }
            Err(e) => println!("  ✗ Failed to get {}: {}", camera_id, e),
        }
    }

    Ok(())
}

fn demo_health_management() -> Result<(), Box<dyn std::error::Error>> {
    let pool_config = PoolConfig {
        reconnection_config: ReconnectionConfig::default(),
        health_check_interval: Duration::from_secs(10),
        auto_remove_unhealthy: true,
        max_idle_time: Some(Duration::from_secs(60)),
    };

    let pool = ViscaConnectionPool::new(pool_config, |addr| {
        // Simulate some cameras failing to connect
        if addr.contains("103") || addr.contains("104") {
            Err(ViscaError::InvalidParameter(
                "Simulated connection failure".to_string(),
            ))
        } else {
            UdpTransport::new(addr).map_err(ViscaError::Io)
        }
    });

    // Add mix of healthy and unhealthy cameras
    let cameras = vec![
        ("healthy1", "192.168.1.100:1259"),
        ("healthy2", "192.168.1.101:1259"),
        ("unhealthy1", "192.168.1.103:1259"),
        ("unhealthy2", "192.168.1.104:1259"),
    ];

    for (id, addr) in cameras {
        let info = CameraInfo {
            id: id.to_string(),
            name: Some(id.to_string()),
            model: None,
            location: None,
        };

        match pool.add_camera(id, addr, info) {
            Ok(()) => println!("✓ Added camera '{}'", id),
            Err(e) => println!("✗ Failed to add camera '{}': {}", id, e),
        }
    }

    // Check health of all cameras
    println!("\nHealth Check Results:");
    let health_results = pool.health_check_all();
    for (camera_id, is_healthy) in &health_results {
        println!(
            "  {}: {}",
            camera_id,
            if *is_healthy {
                "✓ Healthy"
            } else {
                "✗ Unhealthy"
            }
        );
    }

    // Remove unhealthy cameras
    println!("\nRemoving unhealthy cameras...");
    let removed = pool.remove_unhealthy();
    for camera_id in removed {
        println!("  - Removed: {}", camera_id);
    }

    println!("\nRemaining cameras:");
    for camera_id in pool.list_cameras() {
        println!("  - {}", camera_id);
    }

    Ok(())
}

fn demo_concurrent_control() -> Result<(), Box<dyn std::error::Error>> {
    let pool_config = PoolConfig::default();
    let pool = ViscaConnectionPool::new(pool_config, |addr| {
        UdpTransport::new(addr).map_err(ViscaError::Io)
    });

    // Add three cameras
    for i in 1..=3 {
        let info = CameraInfo {
            id: format!("cam{}", i),
            name: Some(format!("Camera {}", i)),
            model: None,
            location: None,
        };
        pool.add_camera(
            format!("cam{}", i),
            &format!("192.168.1.10{}:1259", i),
            info,
        )?;
    }

    println!("Controlling multiple cameras concurrently...\n");

    // Control all cameras simultaneously using threads
    let pool_arc = std::sync::Arc::new(pool);
    let mut handles = vec![];

    for camera_id in ["cam1", "cam2", "cam3"] {
        let pool_clone = pool_arc.clone();
        let cam_id = camera_id.to_string();

        let handle = thread::spawn(move || {
            println!("[{}] Starting control sequence", cam_id);

            match pool_clone.get_connection(&cam_id) {
                Ok(conn) => {
                    let mut transport = conn.transport();

                    // Power on
                    match transport.power_on() {
                        Ok(_) => println!("[{}] ✓ Powered on", cam_id),
                        Err(e) => println!("[{}] ✗ Power on failed: {}", cam_id, e),
                    }

                    thread::sleep(Duration::from_millis(500));

                    // Pan left and right
                    for direction in [PanTiltDirection::Left, PanTiltDirection::Right] {
                        match transport.send_and_wait(&PanTiltCommand::Move {
                            direction,
                            pan_speed: PanSpeed::new(10).unwrap(),
                            tilt_speed: TiltSpeed::new(0).unwrap(),
                        }) {
                            Ok(_) => println!("[{}] ✓ Panned {:?}", cam_id, direction),
                            Err(e) => println!("[{}] ✗ Pan failed: {}", cam_id, e),
                        }

                        thread::sleep(Duration::from_secs(1));
                    }

                    // Zoom
                    match transport.send_and_wait(&ZoomCommand::TeleStandard) {
                        Ok(_) => println!("[{}] ✓ Zoomed in", cam_id),
                        Err(e) => println!("[{}] ✗ Zoom failed: {}", cam_id, e),
                    }

                    thread::sleep(Duration::from_secs(1));

                    // Return home
                    match transport.home() {
                        Ok(_) => println!("[{}] ✓ Returned home", cam_id),
                        Err(e) => println!("[{}] ✗ Home failed: {}", cam_id, e),
                    }

                    println!("[{}] Control sequence complete", cam_id);
                }
                Err(e) => println!("[{}] ✗ Failed to get connection: {}", cam_id, e),
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    println!("\nAll concurrent operations complete!");

    // Final statistics
    println!("\nFinal Pool Statistics:");
    for stats in pool_arc.get_all_stats() {
        let snapshot = stats.connection_stats.snapshot();
        println!(
            "\n  {}: {} commands, {} errors",
            stats.info.id, snapshot.commands_sent, snapshot.error_count
        );
    }

    Ok(())
}
*/
