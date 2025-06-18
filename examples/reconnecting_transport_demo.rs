//! Example demonstrating the ReconnectingTransport wrapper for automatic connection recovery.
//!
//! This example shows how to:
//! - Wrap any transport with automatic reconnection capability
//! - Configure reconnection behavior (retry count, delays, etc.)
//! - Handle connection events
//! - Build resilient camera control applications

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::pan_tilt::PanTiltDirection,
    transport::{AsyncTcpTransport, AsyncUdpTransport, Transport},
    ConnectionEvent, Error, ReconnectingTransport, ReconnectionConfig,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example reconnecting_transport_demo --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== ReconnectingTransport Demo ===\n");

    // Get camera address from command line or environment
    let camera_addr = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("CAMERA_IP").ok())
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    let use_tcp = std::env::args().nth(2).map(|s| s == "tcp").unwrap_or(false);

    // Demonstrate reconnecting transport with both TCP and UDP
    if use_tcp {
        demo_reconnecting_tcp(&camera_addr).await?;
    } else {
        demo_reconnecting_udp(&camera_addr).await?;
    }

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_reconnecting_tcp(camera_addr: &str) -> Result<(), Error> {
    println!("1. TCP Transport with Auto-Reconnection:");
    println!("   Configuring reconnection behavior...\n");

    // Configure reconnection behavior
    let config = ReconnectionConfig {
        max_retries: 5,
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(10),
        backoff_factor: 2.0,
        health_check_interval: Some(Duration::from_secs(30)),
    };

    // Create transport factory
    let addr = camera_addr.to_string();
    let create_transport = move || {
        let addr = addr.clone();
        async move {
            AsyncTcpTransport::new(&addr)
                .await
                .map(|t| Box::new(t) as Box<dyn Transport>)
                .map_err(Error::Io)
        }
    };

    // Create reconnecting transport
    let mut transport = ReconnectingTransport::new(create_transport, config).await?;

    // Set up event callback to monitor connection events
    let event_log = Arc::new(Mutex::new(Vec::new()));
    let event_log_clone = Arc::clone(&event_log);

    transport.set_event_callback(Arc::new(move |event: ConnectionEvent| {
        let event_log = Arc::clone(&event_log_clone);
        tokio::spawn(async move {
            let mut log = event_log.lock().await;
            log.push((std::time::Instant::now(), event.clone()));

            match &event {
                ConnectionEvent::Connected => {
                    println!("   📡 Connection established");
                }
                ConnectionEvent::Disconnected { reason } => {
                    println!("   ❌ Connection lost: {}", reason);
                }
                ConnectionEvent::ReconnectingStarted {
                    attempt,
                    max_attempts,
                } => {
                    println!("   🔄 Reconnection attempt {}/{}", attempt, max_attempts);
                }
                ConnectionEvent::ReconnectingFailed { attempt, error } => {
                    println!("   ⚠️  Attempt {} failed: {}", attempt, error);
                }
                ConnectionEvent::ReconnectionExhausted => {
                    println!("   ❌ All reconnection attempts exhausted");
                }
            }
        });
    }));

    // Create camera with reconnecting transport
    let mut camera = Camera::<PTZOpticsG2>::new(transport);
    println!("   ✓ Camera created with reconnecting transport\n");

    // Perform operations
    demo_camera_operations(&mut camera).await?;

    // Show connection statistics
    // Note: In a real application, you'd keep a reference to the transport
    // to access statistics. This is simplified for the demo.
    println!("\n   Connection Statistics:");
    println!("   - See event log above for connection events");
    println!("   - Transport automatically handled any failures");

    // Display event log
    println!("\n   Connection Event History:");
    let log = event_log.lock().await;
    for (timestamp, event) in log.iter() {
        println!("   [{:?}] {:?}", timestamp, event);
    }

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_reconnecting_udp(camera_addr: &str) -> Result<(), Error> {
    println!("2. UDP Transport with Auto-Reconnection:");
    println!("   Creating resilient UDP connection...\n");

    // Configure with shorter timeouts for UDP
    let config = ReconnectionConfig {
        max_retries: 3,
        initial_delay: Duration::from_millis(200),
        max_delay: Duration::from_secs(2),
        backoff_factor: 1.0, // No exponential backoff
        health_check_interval: Some(Duration::from_secs(15)),
    };

    // Create transport factory
    let addr = camera_addr.to_string();
    let create_transport = move || {
        let addr = addr.clone();
        async move {
            AsyncUdpTransport::new(&addr)
                .await
                .map(|t| Box::new(t) as Box<dyn Transport>)
                .map_err(Error::Io)
        }
    };

    // Create reconnecting transport
    let transport = ReconnectingTransport::new(create_transport, config).await?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    println!("   ✓ UDP camera ready with auto-reconnection\n");

    // Perform operations with simulated interruptions
    demo_resilient_operations(&mut camera).await?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_camera_operations(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
    println!("   Performing basic camera operations:");

    // Power on
    match camera.power_on().await {
        Ok(_) => println!("   ✓ Camera powered on"),
        Err(e) => println!("   ⚠️  Power on failed: {}", e),
    }

    // Move to home
    match camera.home().await {
        Ok(_) => println!("   ✓ Moved to home position"),
        Err(e) => println!("   ⚠️  Home command failed: {}", e),
    }

    // Query position
    match camera.get_position().await {
        Ok((pan_deg, tilt_deg)) => {
            println!(
                "   ✓ Current position: pan={:.1}°, tilt={:.1}°",
                pan_deg.0, tilt_deg.0
            );
        }
        Err(e) => println!("   ⚠️  Position query failed: {}", e),
    }

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_resilient_operations(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Error> {
    println!("   Testing resilient operations:");

    // Test power on
    print!("   - Power on: ");
    match camera.power_on().await {
        Ok(_) => println!("✓ Success"),
        Err(e) => println!("⚠️  Failed (transport will retry): {}", e),
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Test home position
    print!("   - Home position: ");
    match camera.home().await {
        Ok(_) => println!("✓ Success"),
        Err(e) => println!("⚠️  Failed (transport will retry): {}", e),
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Test pan right
    print!("   - Pan right: ");
    match camera.move_continuous(PanTiltDirection::Right, 5, 0).await {
        Ok(_) => println!("✓ Success"),
        Err(e) => println!("⚠️  Failed (transport will retry): {}", e),
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Test stop movement
    print!("   - Stop movement: ");
    match camera.stop().await {
        Ok(_) => println!("✓ Success"),
        Err(e) => println!("⚠️  Failed (transport will retry): {}", e),
    }
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Test query zoom
    print!("   - Query zoom: ");
    match camera.get_zoom_position().await {
        Ok(_) => println!("✓ Success"),
        Err(e) => println!("⚠️  Failed (transport will retry): {}", e),
    }

    println!(
        "\n   Note: Any connection failures were automatically handled by the transport layer"
    );

    Ok(())
}
