#[cfg(not(feature = "blocking-client"))]
fn main() {
    println!("This example requires the 'blocking-client' feature to be enabled.");
    println!();
    println!("The 'blocking-client' feature is required for synchronous TCP/UDP transports");
    println!("and the connection pool functionality.");
    println!();
    println!("Try running with: cargo run --example connection_pool --features blocking-client");
}

#[cfg(feature = "blocking-client")]
use grafton_visca::{
    command::pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
    command::power::{Power, PowerCommand},
    command::zoom::ZoomCommand,
    connection_pool::{CameraInfo, ConnectionPool, ConnectionType, PoolConfig},
};
#[cfg(feature = "blocking-client")]
use std::time::Duration;

#[cfg(feature = "blocking-client")]
fn setup_camera_pool() -> ConnectionPool {
    // Configure the connection pool
    let config = PoolConfig {
        health_check_interval: Duration::from_secs(30),
        auto_remove_unhealthy: true,
        max_idle_time: Some(Duration::from_secs(120)),
    };

    // Create the connection pool
    let pool = ConnectionPool::new(config);

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

    pool
}

#[cfg(feature = "blocking-client")]
fn power_on_cameras(pool: &ConnectionPool) {
    println!("\nPowering on cameras...");
    let power_on = PowerCommand { power: Power::On };
    for camera_id in pool.list_cameras() {
        match pool.execute_command(&camera_id, &power_on) {
            Ok(_) => println!("  {camera_id} - Powered on"),
            Err(e) => println!("  {camera_id} - Power on failed: {e}"),
        }
    }
}

#[cfg(feature = "blocking-client")]
fn demonstrate_camera_control(pool: &ConnectionPool) -> Result<(), Box<dyn std::error::Error>> {
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

    Ok(())
}

#[cfg(feature = "blocking-client")]
fn check_camera_health(pool: &ConnectionPool) {
    println!("\nChecking camera health...");
    let health_results = pool.health_check_all();
    for (camera_id, is_healthy) in health_results {
        println!(
            "  {} - {}",
            camera_id,
            if is_healthy { "Healthy" } else { "Unhealthy" }
        );
    }
}

#[cfg(feature = "blocking-client")]
fn display_camera_stats(pool: &ConnectionPool) {
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
}

#[cfg(feature = "blocking-client")]
fn cleanup_cameras(pool: &ConnectionPool) {
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
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== VISCA Connection Pool Example (v0.5.0) ===\n");

    let pool = setup_camera_pool();

    // List all cameras
    println!("\nCameras in pool:");
    for camera_id in pool.list_cameras() {
        println!("  - {camera_id}");
    }

    power_on_cameras(&pool);
    demonstrate_camera_control(&pool)?;
    check_camera_health(&pool);
    display_camera_stats(&pool);
    cleanup_cameras(&pool);

    // Final camera list
    println!("\nFinal cameras in pool:");
    for camera_id in pool.list_cameras() {
        println!("  - {camera_id}");
    }

    Ok(())
}
