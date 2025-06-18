//! Demonstrates using ResilientTransport with async transports (fix for issue #58)
//!
//! This example shows how to create a resilient transport wrapper around
//! AsyncTcpTransport or AsyncUdpTransport to get automatic retry and
//! reconnection capabilities.

#[cfg(all(feature = "tokio", feature = "async-client"))]
use grafton_visca::{
    camera::{Camera, profiles::PTZOpticsG2},
    command::{
        zoom::ZoomCommand,
        pan_tilt::PanTiltCommand,
    },
    transport::{
        resilient::{ResilientTransport, ResilienceConfig, ResilienceEvent},
        AsyncTcpTransport,
        AsyncUdpTransport,
    },
    Error as ViscaError,
};
use std::sync::Arc;
use std::time::Duration;

#[cfg(all(feature = "tokio", feature = "async-client"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Resilient Async Transport Demo");
    println!("==============================");
    println!("This example demonstrates using ResilientTransport with async transports.");
    println!("It provides automatic retry and reconnection on network failures.\n");

    // Camera IP address
    let camera_ip = "192.168.1.100:52381";

    // Configure resilience behavior
    let mut config = ResilienceConfig::default();
    config.max_retries = 5;
    config.initial_retry_delay = Duration::from_millis(500);
    config.max_retry_delay = Duration::from_secs(10);
    config.reconnect_delay = Duration::from_secs(2);
    config.max_reconnect_attempts = 10;

    println!("Resilience Configuration:");
    println!("  Max retries: {}", config.max_retries);
    println!("  Initial retry delay: {:?}", config.initial_retry_delay);
    println!("  Max retry delay: {:?}", config.max_retry_delay);
    println!("  Reconnect delay: {:?}", config.reconnect_delay);
    println!("  Max reconnect attempts: {}\n", config.max_reconnect_attempts);

    // Example 1: Resilient TCP Transport
    println!("Example 1: Creating Resilient TCP Transport");
    println!("-------------------------------------------");

    // Create the initial TCP transport
    let tcp_transport = match AsyncTcpTransport::new(camera_ip).await {
        Ok(transport) => {
            println!("✓ Connected to camera via TCP at {}", camera_ip);
            transport
        }
        Err(e) => {
            println!("✗ Failed to connect to camera: {}", e);
            println!("  Make sure the camera is powered on and accessible.");
            return Ok(());
        }
    };

    // Create a factory function for reconnection
    let tcp_factory = {
        let camera_ip = camera_ip.to_string();
        move || {
            let ip = camera_ip.clone();
            futures::executor::block_on(async move {
                AsyncTcpTransport::new(&ip)
                    .await
                    .map_err(|e| ViscaError::Io(e))
            })
        }
    };

    // Create the resilient transport with event monitoring
    let mut tcp_resilient = ResilientTransport::new(
        tcp_transport,
        tcp_factory,
        config,
    );

    // Set up event callback to monitor resilience events
    let event_callback = Arc::new(|event: ResilienceEvent| {
        match event {
            ResilienceEvent::OperationSucceeded { retries } => {
                if retries > 0 {
                    println!("  ↻ Operation succeeded after {} retries", retries);
                }
            }
            ResilienceEvent::OperationFailed { attempts, error } => {
                println!("  ✗ Operation failed after {} attempts: {}", attempts, error);
            }
            ResilienceEvent::Reconnected { attempts } => {
                println!("  ↻ Reconnected after {} attempts", attempts);
            }
            ResilienceEvent::ReconnectionFailed { attempts, error } => {
                println!("  ✗ Reconnection failed after {} attempts: {}", attempts, error);
            }
            ResilienceEvent::HealthCheckPassed => {
                println!("  ✓ Health check passed");
            }
            ResilienceEvent::HealthCheckFailed { error } => {
                println!("  ✗ Health check failed: {}", error);
            }
        }
    });

    tcp_resilient.set_event_callback(event_callback.clone());

    // Create a camera with the resilient transport
    let mut tcp_camera = Camera::<PTZOpticsG2>::new(tcp_resilient.clone());

    println!("\nTesting TCP resilient transport:");

    // Test basic operations
    println!("  → Sending zoom in command...");
    let zoom_in_cmd = ZoomCommand::ZoomInStandard;
    match tcp_camera.send_raw_async(&zoom_in_cmd).await {
        Ok(_) => println!("  ✓ Zoom in command sent successfully"),
        Err(e) => println!("  ✗ Failed to send zoom in: {}", e),
    }

    tokio::time::sleep(Duration::from_secs(1)).await;

    println!("  → Sending zoom out command...");
    let zoom_out_cmd = ZoomCommand::ZoomOutStandard;
    match tcp_camera.send_raw_async(&zoom_out_cmd).await {
        Ok(_) => println!("  ✓ Zoom out command sent successfully"),
        Err(e) => println!("  ✗ Failed to send zoom out: {}", e),
    }

    // Get statistics from the resilient transport
    let stats = tcp_resilient.stats();
    println!("\nTCP Transport Statistics:");
    println!("  Total operations: {}", stats.total_operations);
    println!("  Success rate: {:.1}%", stats.success_rate());
    println!("  Average retries: {:.2}", stats.average_retries());

    // Example 2: Resilient UDP Transport
    println!("\n\nExample 2: Creating Resilient UDP Transport");
    println!("-------------------------------------------");

    // Create the initial UDP transport
    let udp_transport = match AsyncUdpTransport::new(camera_ip).await {
        Ok(transport) => {
            println!("✓ Created UDP transport for {}", camera_ip);
            transport
        }
        Err(e) => {
            println!("✗ Failed to create UDP transport: {}", e);
            return Ok(());
        }
    };

    // Create a factory function for UDP reconnection
    let udp_factory = {
        let camera_ip = camera_ip.to_string();
        move || {
            let ip = camera_ip.clone();
            futures::executor::block_on(async move {
                AsyncUdpTransport::new(&ip)
                    .await
                    .map_err(|e| ViscaError::Io(e))
            })
        }
    };

    // Create the resilient UDP transport
    let mut udp_resilient = ResilientTransport::new(
        udp_transport,
        udp_factory,
        config,
    );

    udp_resilient.set_event_callback(event_callback);

    // Create a camera with the resilient UDP transport
    let mut udp_camera = Camera::<PTZOpticsG2>::new(udp_resilient.clone());

    println!("\nTesting UDP resilient transport:");

    // Test basic operations
    println!("  → Sending PTZ home command...");
    let ptz_home_cmd = PanTiltCommand::Home;
    match udp_camera.send_raw_async(&ptz_home_cmd).await {
        Ok(_) => println!("  ✓ PTZ home command sent successfully"),
        Err(e) => println!("  ✗ Failed to send PTZ home: {}", e),
    }

    // Example 3: Demonstrating Clone capability
    println!("\n\nExample 3: Demonstrating Clone Capability");
    println!("-----------------------------------------");

    // Clone the UDP resilient transport
    let cloned_transport = udp_resilient.clone();
    let mut cloned_camera = Camera::<PTZOpticsG2>::new(cloned_transport);

    println!("✓ Successfully cloned resilient transport");

    // Use both cameras concurrently
    println!("\nTesting concurrent usage of cloned transports:");

    let zoom_in = ZoomCommand::ZoomInStandard;
    let zoom_out = ZoomCommand::ZoomOutStandard;

    let camera1_future = async {
        println!("  Camera 1 → Sending zoom in...");
        match cloned_camera.send_raw_async(&zoom_in).await {
            Ok(_) => println!("  Camera 1 ✓ Zoom in sent"),
            Err(e) => println!("  Camera 1 ✗ Failed: {}", e),
        }
    };

    let camera2_future = async {
        println!("  Camera 2 → Sending zoom out...");
        match udp_camera.send_raw_async(&zoom_out).await {
            Ok(_) => println!("  Camera 2 ✓ Zoom out sent"),
            Err(e) => println!("  Camera 2 ✗ Failed: {}", e),
        }
    };

    // Execute both operations concurrently
    tokio::join!(camera1_future, camera2_future);

    println!("\n✓ Resilient async transport demo completed successfully!");
    println!("\nKey benefits demonstrated:");
    println!("  • AsyncTcpTransport and AsyncUdpTransport now implement Clone");
    println!("  • ResilientTransport can wrap async transports");
    println!("  • Automatic retry and reconnection on failures");
    println!("  • Event monitoring for observability");
    println!("  • Concurrent usage with cloned transports");

    Ok(())
}

#[cfg(not(all(feature = "tokio", feature = "async-client")))]
fn main() {
    println!("This example requires the 'tokio' and 'async-client' features.");
    println!("Run with: cargo run --example resilient_async_transport_demo --features tokio,async-client");
}