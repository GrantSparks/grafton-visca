//! Example demonstrating managing multiple cameras with async connections using the Camera API.
//!
//! This example shows how to:
//! - Connect to multiple cameras asynchronously
//! - Control multiple cameras concurrently
//! - Manage camera connections efficiently
//! - Perform coordinated multi-camera operations

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, units::ViscaUnits, Camera, CameraProfile},
    command::pan_tilt::PanTiltDirection,
    transport::AsyncUdpTransport,
    Error,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example async_connection_pool --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Async Multi-Camera Management with Camera API ===\n");

    // Example 1: Managing multiple cameras
    demo_multi_camera_management().await?;

    println!("\n=== Concurrent Camera Control ===\n");
    demo_concurrent_control().await?;

    println!("\n=== Coordinated Camera Movement ===\n");
    demo_coordinated_movement().await?;

    Ok(())
}

// Type alias for our camera pool
type CameraPool = Arc<Mutex<HashMap<String, Camera<PTZOpticsG2>>>>;

#[cfg(feature = "async-client")]
async fn demo_multi_camera_management() -> Result<(), Error> {
    // Define camera configurations
    let cameras = vec![
        ("cam1", "192.168.1.100:5678", "Front Camera"),
        ("cam2", "192.168.1.101:5678", "Side Camera"),
        ("cam3", "192.168.1.102:5678", "Rear Camera"),
    ];

    // Connect to all cameras
    let mut connections = HashMap::new();

    for (id, addr, name) in &cameras {
        println!("Connecting to {} ({})...", name, addr);
        match AsyncUdpTransport::new(addr).await {
            Ok(transport) => {
                let camera = Camera::<PTZOpticsG2>::new(transport);
                println!("✓ Connected to {}", name);
                connections.insert(id.to_string(), (camera, name.to_string()));
            }
            Err(e) => {
                println!("✗ Failed to connect to {}: {}", name, e);
            }
        }
    }

    // Power on all cameras
    println!("\nPowering on all cameras...");
    let mut power_futures = Vec::new();

    for (id, (camera, _)) in &mut connections {
        let id_clone = id.clone();
        let power_fut = async move {
            let result = camera.power_on().await;
            (id_clone, result)
        };
        power_futures.push(power_fut);
    }

    let results = futures_util::future::join_all(power_futures).await;

    for (id, result) in results {
        if let Some((_, name)) = connections.get(&id) {
            match result {
                Ok(_) => println!("  ✓ {} powered on", name),
                Err(e) => println!("  ✗ {} power on failed: {}", name, e),
            }
        }
    }

    // Move all cameras to home position
    println!("\nMoving all cameras to home position...");
    let mut home_futures = Vec::new();

    for (id, (camera, _)) in &mut connections {
        let id_clone = id.clone();
        let home_fut = async move {
            let result = camera.home().await;
            (id_clone, result)
        };
        home_futures.push(home_fut);
    }

    let results = futures_util::future::join_all(home_futures).await;

    for (id, result) in results {
        if let Some((_, name)) = connections.get(&id) {
            match result {
                Ok(_) => println!("  ✓ {} moved to home", name),
                Err(e) => println!("  ✗ {} home command failed: {}", name, e),
            }
        }
    }

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_concurrent_control() -> Result<(), Error> {
    // For demo purposes, we'll simulate multiple cameras at the same address
    // In real usage, these would be different addresses
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Create multiple camera connections
    let cameras: CameraPool = Arc::new(Mutex::new(HashMap::new()));

    for i in 1..=3 {
        match AsyncUdpTransport::new(&camera_addr).await {
            Ok(transport) => {
                let camera = Camera::<PTZOpticsG2>::new(transport);
                cameras.lock().await.insert(format!("cam{}", i), camera);
            }
            Err(e) => {
                eprintln!("Failed to create camera {}: {}", i, e);
            }
        }
    }

    println!(
        "Controlling {} cameras concurrently...\n",
        cameras.lock().await.len()
    );

    // Control all cameras simultaneously
    let mut tasks = vec![];

    for i in 1..=3 {
        let cameras_clone = cameras.clone();
        let cam_id = format!("cam{}", i);

        let task = tokio::spawn(async move {
            println!("[{}] Starting control sequence", cam_id);

            let mut cameras_lock = cameras_clone.lock().await;
            if let Some(camera) = cameras_lock.get_mut(&cam_id) {
                // Power on
                match camera.power_on().await {
                    Ok(_) => println!("[{}] ✓ Powered on", cam_id),
                    Err(e) => println!("[{}] ✗ Power on failed: {}", cam_id, e),
                }

                sleep(Duration::from_millis(500)).await;

                // Pan sequence
                for direction in [PanTiltDirection::Left, PanTiltDirection::Right] {
                    match camera.move_continuous(direction, 10, 0).await {
                        Ok(_) => println!("[{}] ✓ Panned {:?}", cam_id, direction),
                        Err(e) => println!("[{}] ✗ Pan failed: {}", cam_id, e),
                    }

                    sleep(Duration::from_secs(1)).await;
                }

                // Stop movement
                let _ = camera.stop().await;

                // Zoom
                match camera.zoom_in().await {
                    Ok(_) => println!("[{}] ✓ Zoomed in", cam_id),
                    Err(e) => println!("[{}] ✗ Zoom failed: {}", cam_id, e),
                }

                sleep(Duration::from_secs(1)).await;

                // Return home
                match camera.home().await {
                    Ok(_) => println!("[{}] ✓ Returned home", cam_id),
                    Err(e) => println!("[{}] ✗ Home failed: {}", cam_id, e),
                }

                println!("[{}] Control sequence complete", cam_id);
            }
        });

        tasks.push(task);
    }

    // Wait for all tasks to complete
    for task in tasks {
        let _ = task.await;
    }

    println!("\nAll concurrent operations complete!");
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_coordinated_movement() -> Result<(), Error> {
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("Setting up coordinated camera movement...");

    // Connect to cameras
    let transport1 = AsyncUdpTransport::new(&camera_addr).await?;
    let transport2 = AsyncUdpTransport::new(&camera_addr).await?;
    let transport3 = AsyncUdpTransport::new(&camera_addr).await?;

    let mut cam1 = Camera::<PTZOpticsG2>::new(transport1);
    let mut cam2 = Camera::<PTZOpticsG2>::new(transport2);
    let mut cam3 = Camera::<PTZOpticsG2>::new(transport3);

    // Move all cameras to starting positions
    println!("\nPhase 1: Moving to starting positions");

    // Note: The Camera API uses typed units for positions
    let start_positions = vec![
        (&mut cam1, ViscaUnits(-1000), ViscaUnits(0), "Camera 1"),
        (&mut cam2, ViscaUnits(0), ViscaUnits(500), "Camera 2"),
        (&mut cam3, ViscaUnits(1000), ViscaUnits(0), "Camera 3"),
    ];

    let mut position_futures = Vec::new();
    for (cam, pan, tilt, name) in start_positions {
        let name = name.to_string();
        let fut = async move {
            let result = cam.set_position_units(pan, tilt).await;
            (name, result)
        };
        position_futures.push(fut);
    }

    let results = futures_util::future::join_all(position_futures).await;
    for (name, result) in results {
        match result {
            Ok(_) => println!("  ✓ {} in position", name),
            Err(e) => println!("  ✗ {} positioning failed: {}", name, e),
        }
    }

    sleep(Duration::from_secs(3)).await;

    // Coordinated sweep
    println!("\nPhase 2: Coordinated sweep");

    // All cameras pan right together
    let sweep_futures = vec![
        cam1.move_continuous(PanTiltDirection::Right, 5, 0),
        cam2.move_continuous(PanTiltDirection::Right, 5, 0),
        cam3.move_continuous(PanTiltDirection::Right, 5, 0),
    ];

    futures_util::future::join_all(sweep_futures).await;
    println!("  ✓ All cameras sweeping right");

    sleep(Duration::from_secs(3)).await;

    // Stop all cameras
    let stop_futures = vec![cam1.stop(), cam2.stop(), cam3.stop()];

    futures_util::future::join_all(stop_futures).await;
    println!("  ✓ All cameras stopped");

    // Return to home
    println!("\nPhase 3: Return to home positions");

    let home_futures = vec![cam1.home(), cam2.home(), cam3.home()];

    futures_util::future::join_all(home_futures).await;
    println!("  ✓ All cameras returning home");

    println!("\nCoordinated movement demo complete!");

    // Bonus: Demonstrate profile-aware operations
    println!("\n=== Camera Profile Information ===");
    let profile = cam1.profile();
    println!("Camera model: {}", PTZOpticsG2::MODEL_NAME);
    println!("Pan range: {:?} units", PTZOpticsG2::PAN_RANGE);
    println!("Tilt range: {:?} units", PTZOpticsG2::TILT_RANGE);
    println!("Max pan speed: {}", PTZOpticsG2::MAX_PAN_SPEED);
    println!("Max tilt speed: {}", PTZOpticsG2::MAX_TILT_SPEED);

    // Convert to degrees for display
    let pan_deg_range = profile.pan_units_to_degrees(*PTZOpticsG2::PAN_RANGE.start())
        ..=profile.pan_units_to_degrees(*PTZOpticsG2::PAN_RANGE.end());
    let tilt_deg_range = profile.tilt_units_to_degrees(*PTZOpticsG2::TILT_RANGE.start())
        ..=profile.tilt_units_to_degrees(*PTZOpticsG2::TILT_RANGE.end());
    println!("Pan range in degrees: {:?}", pan_deg_range);
    println!("Tilt range in degrees: {:?}", tilt_deg_range);

    Ok(())
}
