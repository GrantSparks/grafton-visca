//! Example demonstrating managing multiple cameras with async connections.
//!
//! This example shows how to:
//! - Connect to multiple cameras asynchronously
//! - Control multiple cameras concurrently
//! - Manage camera connections efficiently
//! - Perform coordinated multi-camera operations

use grafton_visca::command::{
    pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
    power::{Power, PowerCommand},
    InquiryCommand, Response, ZoomCommand,
};
use grafton_visca::{Client, Error};
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

    println!("=== Async Multi-Camera Management Example ===\n");

    // Example 1: Managing multiple cameras
    demo_multi_camera_management().await?;

    println!("\n=== Concurrent Camera Control ===\n");
    demo_concurrent_control().await?;

    println!("\n=== Coordinated Camera Movement ===\n");
    demo_coordinated_movement().await?;

    Ok(())
}

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
        match Client::connect_udp_async(addr).await {
            Ok(client) => {
                println!("✓ Connected to {}", name);
                connections.insert(id.to_string(), (client, name.to_string()));
            }
            Err(e) => {
                println!("✗ Failed to connect to {}: {}", name, e);
            }
        }
    }

    // Check health of all connections
    println!("\nChecking camera health...");
    for (_id, (client, name)) in &connections {
        match client.is_healthy().await {
            Ok(true) => println!("✓ {} is healthy", name),
            Ok(false) => println!("✗ {} is not responding", name),
            Err(e) => println!("✗ {} health check failed: {}", name, e),
        }
    }

    // Query all cameras
    println!("\nQuerying camera status...");
    let mut query_futures = Vec::new();

    for (id, (client, _)) in &connections {
        let id_clone = id.clone();
        let power_fut = async move {
            let result = client.send_async(&InquiryCommand::Power).await;
            (id_clone, result)
        };
        query_futures.push(power_fut);
    }

    let results = futures_util::future::join_all(query_futures).await;

    for (id, result) in results {
        if let Some((_, name)) = connections.get(&id) {
            match result {
                Ok(Response::InquiryResponse(grafton_visca::InquiryResponse::Power { on })) => {
                    println!("  {} power: {}", name, if on { "ON" } else { "OFF" });
                }
                _ => println!("  {} power query failed", name),
            }
        }
    }

    // Move all cameras to home position
    println!("\nMoving all cameras to home position...");
    let mut home_futures = Vec::new();

    for (id, (client, _)) in &connections {
        let id_clone = id.clone();
        let home_fut = async move {
            let result = client.send_async(&PanTiltCommand::Home).await;
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
    let cameras = Arc::new(Mutex::new(HashMap::new()));

    for i in 1..=3 {
        let client = Client::connect_udp_async(&camera_addr).await?;
        cameras.lock().await.insert(format!("cam{}", i), client);
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

            let cameras_lock = cameras_clone.lock().await;
            if let Some(client) = cameras_lock.get(&cam_id) {
                // Power on
                match client.send_async(&PowerCommand { power: Power::On }).await {
                    Ok(_) => println!("[{}] ✓ Powered on", cam_id),
                    Err(e) => println!("[{}] ✗ Power on failed: {}", cam_id, e),
                }

                sleep(Duration::from_millis(500)).await;

                // Pan sequence
                for direction in [PanTiltDirection::Left, PanTiltDirection::Right] {
                    match client
                        .send_async(&PanTiltCommand::Move {
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

                // Stop movement
                let _ = client
                    .send_async(&PanTiltCommand::Move {
                        direction: PanTiltDirection::Stop,
                        pan_speed: PanSpeed::new(0).unwrap(),
                        tilt_speed: TiltSpeed::new(0).unwrap(),
                    })
                    .await;

                // Zoom
                match client.send_async(&ZoomCommand::ZoomInStandard).await {
                    Ok(_) => println!("[{}] ✓ Zoomed in", cam_id),
                    Err(e) => println!("[{}] ✗ Zoom failed: {}", cam_id, e),
                }

                sleep(Duration::from_secs(1)).await;

                // Return home
                match client.send_async(&PanTiltCommand::Home).await {
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
    let cam1 = Client::connect_udp_async(&camera_addr).await?;
    let cam2 = Client::connect_udp_async(&camera_addr).await?;
    let cam3 = Client::connect_udp_async(&camera_addr).await?;

    // Move all cameras to starting positions
    println!("\nPhase 1: Moving to starting positions");

    let start_positions = vec![
        (cam1.clone(), -1000, 0, "Camera 1"),
        (cam2.clone(), 0, 500, "Camera 2"),
        (cam3.clone(), 1000, 0, "Camera 3"),
    ];

    let mut position_futures = Vec::new();
    for (cam, pan, tilt, name) in start_positions {
        let name = name.to_string();
        let fut = async move {
            let result = match (PanSpeed::new(15), TiltSpeed::new(15)) {
                (Ok(pan_speed), Ok(tilt_speed)) => {
                    cam.send_async(&PanTiltCommand::AbsolutePosition {
                        pan,
                        tilt,
                        pan_speed,
                        tilt_speed,
                    })
                    .await
                }
                _ => Err(Error::InvalidParameter("Invalid speed".to_string())),
            };
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
    let pan_speed = PanSpeed::new(5).unwrap();
    let tilt_speed = TiltSpeed::new(0).unwrap();

    let move_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Right,
        pan_speed,
        tilt_speed,
    };

    let sweep_futures = vec![
        cam1.send_async(&move_cmd),
        cam2.send_async(&move_cmd),
        cam3.send_async(&move_cmd),
    ];

    futures_util::future::join_all(sweep_futures).await;
    println!("  ✓ All cameras sweeping right");

    sleep(Duration::from_secs(3)).await;

    // Stop all cameras
    let stop_speed = PanSpeed::new(0).unwrap();
    let stop_tilt = TiltSpeed::new(0).unwrap();

    let stop_cmd = PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: stop_speed,
        tilt_speed: stop_tilt,
    };

    let stop_futures = vec![
        cam1.send_async(&stop_cmd),
        cam2.send_async(&stop_cmd),
        cam3.send_async(&stop_cmd),
    ];

    futures_util::future::join_all(stop_futures).await;
    println!("  ✓ All cameras stopped");

    // Return to home
    println!("\nPhase 3: Return to home positions");

    let home_cmd = PanTiltCommand::Home;

    let home_futures = vec![
        cam1.send_async(&home_cmd),
        cam2.send_async(&home_cmd),
        cam3.send_async(&home_cmd),
    ];

    futures_util::future::join_all(home_futures).await;
    println!("  ✓ All cameras returning home");

    println!("\nCoordinated movement demo complete!");
    Ok(())
}
