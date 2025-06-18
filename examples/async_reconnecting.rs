//! Example demonstrating automatic connection recovery with ReconnectingTransport.
//!
//! This example shows real-world usage of the ReconnectingTransport wrapper:
//! - Handling network interruptions gracefully
//! - Monitoring connection health
//! - Building fault-tolerant camera control systems

use grafton_visca::{
    camera::profiles::PTZOpticsG2, command::pan_tilt::PanTiltDirection,
    transport::AsyncTcpTransport, Camera, ConnectionEvent, Error, ReconnectingTransport,
    ReconnectionConfig,
};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::time::{interval, sleep};

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example async_reconnecting --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== Async Auto-Reconnection Example ===\n");

    // Get camera address
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Run demonstrations
    demo_connection_monitoring(&camera_addr).await?;
    demo_continuous_operation(&camera_addr).await?;
    demo_multi_camera_resilience(&camera_addr).await?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_connection_monitoring(camera_addr: &str) -> Result<(), Error> {
    println!("1. Connection Health Monitoring with Auto-Recovery:");
    println!("   Setting up camera with health monitoring...\n");

    // Track connection state
    let is_connected = Arc::new(AtomicBool::new(false));
    let reconnect_count = Arc::new(AtomicU32::new(0));
    let last_error = Arc::new(Mutex::new(None::<String>));

    // Configure with health checks
    let config = ReconnectionConfig {
        max_retries: 10,
        initial_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(30),
        backoff_factor: 2.0,
        health_check_interval: Some(Duration::from_secs(10)),
    };

    // Create transport with connection tracking
    let addr = camera_addr.to_string();
    let mut transport = ReconnectingTransport::new(
        move || {
            let addr = addr.clone();
            async move { AsyncTcpTransport::new(&addr).await.map_err(Error::Io) }
        },
        config,
    )
    .await?;

    // Set up event monitoring
    let is_connected_clone = Arc::clone(&is_connected);
    let reconnect_count_clone = Arc::clone(&reconnect_count);
    let last_error_clone = Arc::clone(&last_error);

    transport.set_event_callback(Arc::new(move |event| {
        let is_connected = Arc::clone(&is_connected_clone);
        let reconnect_count = Arc::clone(&reconnect_count_clone);
        let last_error = Arc::clone(&last_error_clone);

        tokio::spawn(async move {
            match event {
                ConnectionEvent::Connected => {
                    is_connected.store(true, Ordering::SeqCst);
                    println!("   ✅ Connection established");
                }
                ConnectionEvent::Disconnected { reason } => {
                    is_connected.store(false, Ordering::SeqCst);
                    let mut error = last_error.lock().await;
                    *error = Some(reason.clone());
                    println!("   ❌ Connection lost: {}", reason);
                }
                ConnectionEvent::ReconnectingStarted { .. } => {
                    reconnect_count.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
        });
    }));

    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Spawn health monitoring task
    let is_connected_monitor = Arc::clone(&is_connected);
    let reconnect_monitor = Arc::clone(&reconnect_count);
    let monitor_handle = tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(5));
        let start_time = Instant::now();

        loop {
            ticker.tick().await;
            let connected = is_connected_monitor.load(Ordering::SeqCst);
            let reconnects = reconnect_monitor.load(Ordering::SeqCst);
            let uptime = start_time.elapsed();

            println!(
                "   📊 Status: {} | Reconnects: {} | Uptime: {:?}",
                if connected {
                    "🟢 Connected"
                } else {
                    "🔴 Disconnected"
                },
                reconnects,
                uptime
            );

            if uptime > Duration::from_secs(30) {
                break;
            }
        }
    });

    // Perform operations while monitoring runs
    println!("   Starting operations with connection monitoring...\n");

    for i in 1..=6 {
        // Send a simple command to test connection
        match camera.stop().await {
            Ok(_) => {
                println!("   Op {}: Stop command sent successfully", i);
            }
            Err(e) => {
                println!("   Op {}: Failed - {}", i, e);
            }
        }
        sleep(Duration::from_secs(5)).await;
    }

    monitor_handle.abort();
    println!();

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_continuous_operation(camera_addr: &str) -> Result<(), Error> {
    println!("2. Continuous Operation with Automatic Recovery:");
    println!("   Running continuous pan/tilt pattern...\n");

    // Simple reconnection config
    let config = ReconnectionConfig {
        max_retries: 5,
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(5),
        backoff_factor: 2.0,
        health_check_interval: None,
    };

    let addr = camera_addr.to_string();
    let transport = ReconnectingTransport::new(
        move || {
            let addr = addr.clone();
            async move { AsyncTcpTransport::new(&addr).await.map_err(Error::Io) }
        },
        config,
    )
    .await?;

    let camera = Camera::<PTZOpticsG2>::new(transport);

    // Define movement pattern
    let movements = vec![
        (PanTiltDirection::Right, 5, 0, Duration::from_secs(2)),
        (PanTiltDirection::Up, 0, 5, Duration::from_secs(2)),
        (PanTiltDirection::Left, 5, 0, Duration::from_secs(2)),
        (PanTiltDirection::Down, 0, 5, Duration::from_secs(2)),
    ];

    // Run pattern continuously
    let start_time = Instant::now();
    let mut cycle = 0;

    while start_time.elapsed() < Duration::from_secs(20) {
        cycle += 1;
        println!("   🔄 Cycle {}", cycle);

        for (direction, pan_speed, tilt_speed, duration) in &movements {
            // Start movement
            match camera
                .move_continuous(*direction, *pan_speed, *tilt_speed)
                .await
            {
                Ok(_) => print!("   → Moving {:?}... ", direction),
                Err(e) => {
                    println!("   ⚠️  Movement failed: {}", e);
                    continue;
                }
            }

            // Wait
            sleep(*duration).await;

            // Stop movement
            match camera.stop().await {
                Ok(_) => println!("stopped"),
                Err(e) => println!("stop failed: {}", e),
            }
        }
    }

    println!("   ✅ Continuous operation completed\n");
    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_multi_camera_resilience(camera_addr: &str) -> Result<(), Error> {
    println!("3. Multi-Camera System with Resilient Connections:");
    println!("   Simulating control of multiple cameras...\n");

    // Create multiple cameras with different addresses
    let camera_addresses = [
        camera_addr.to_string(),
        camera_addr.replace("100", "101"),
        camera_addr.replace("100", "102"),
    ];

    let config = ReconnectionConfig {
        max_retries: 3,
        initial_delay: Duration::from_millis(200),
        max_delay: Duration::from_secs(2),
        backoff_factor: 1.0,
        health_check_interval: Some(Duration::from_secs(15)),
    };

    // Create cameras with resilient connections
    let mut cameras = Vec::new();
    for (idx, addr) in camera_addresses.iter().enumerate() {
        println!("   Creating camera {} at {}...", idx + 1, addr);

        let addr_clone = addr.clone();
        match ReconnectingTransport::new(
            move || {
                let addr = addr_clone.clone();
                async move { AsyncTcpTransport::new(&addr).await.map_err(Error::Io) }
            },
            config,
        )
        .await
        {
            Ok(transport) => {
                let camera = Camera::<PTZOpticsG2>::new(transport);
                cameras.push(Some(camera));
                println!("   ✓ Camera {} ready", idx + 1);
            }
            Err(e) => {
                cameras.push(None);
                println!("   ✗ Camera {} unavailable: {}", idx + 1, e);
            }
        }
    }

    // Control all available cameras
    println!("\n   Sending commands to all available cameras:");

    for (idx, camera_opt) in cameras.iter_mut().enumerate() {
        if let Some(camera) = camera_opt {
            print!("   Camera {}: ", idx + 1);
            match camera.home().await {
                Ok(_) => println!("✓ Homed"),
                Err(e) => println!("✗ Failed: {}", e),
            }
        }
    }

    // Coordinate movement across cameras
    println!("\n   Coordinating movement across cameras:");
    let mut tasks = Vec::new();

    for (idx, camera_opt) in cameras.into_iter().enumerate() {
        if let Some(camera) = camera_opt {
            let task = tokio::spawn(async move {
                let delay = Duration::from_millis(idx as u64 * 500);
                sleep(delay).await;

                match camera.move_continuous(PanTiltDirection::Right, 10, 0).await {
                    Ok(_) => {
                        sleep(Duration::from_secs(2)).await;
                        let _ = camera.stop().await;
                        Ok(idx + 1)
                    }
                    Err(e) => Err((idx + 1, e)),
                }
            });
            tasks.push(task);
        }
    }

    // Wait for all movements
    for task in tasks {
        match task.await {
            Ok(Ok(cam_num)) => println!("   ✓ Camera {} completed movement", cam_num),
            Ok(Err((cam_num, e))) => println!("   ✗ Camera {} failed: {}", cam_num, e),
            Err(e) => println!("   ✗ Task error: {}", e),
        }
    }

    println!("\n   ✅ Multi-camera demonstration complete");
    Ok(())
}

// Required for the event callback
use tokio::sync::Mutex;
