//! Example demonstrating ResilientTransport without Clone requirement
//!
//! This example shows how the new implementation allows wrapping transports
//! that don't implement Clone, using factory functions for reconnection.

#[cfg(feature = "async-client")]
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    transport::{
        resilient::{ResilienceConfig, ResilienceEvent, ResilientTransport},
        AsyncTcpTransport, AsyncUdpTransport,
    },
};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Camera address
    let camera_addr = "192.168.1.100:52381";

    // Configure resilient behavior
    let config = ResilienceConfig {
        max_retries: 3,
        initial_retry_delay: Duration::from_millis(100),
        max_retry_delay: Duration::from_secs(2),
        backoff_factor: 2.0,
        max_reconnect_attempts: 5,
        reconnect_delay: Duration::from_secs(1),
        ..Default::default()
    };

    // Create initial transport
    let transport = AsyncUdpTransport::new(camera_addr).await?;

    // Create resilient transport with async factory
    // The factory creates a new transport instance when reconnection is needed
    let mut resilient = ResilientTransport::new_async(
        transport,
        move || async move {
            println!("Reconnecting to camera at {}...", camera_addr);
            AsyncUdpTransport::new(camera_addr)
                .await
                .map_err(|e| grafton_visca::Error::Io(e))
        },
        config,
    );

    // Set up event monitoring
    resilient.set_event_callback(Arc::new(|event| {
        match event {
            ResilienceEvent::OperationSucceeded { retries } if retries > 0 => {
                println!("✓ Operation succeeded after {} retries", retries);
            }
            ResilienceEvent::Reconnected { attempts } => {
                println!("✓ Reconnected after {} attempts", attempts);
            }
            ResilienceEvent::OperationFailed { attempts, error } => {
                println!("✗ Operation failed after {} attempts: {}", attempts, error);
            }
            _ => {}
        }
    }));

    // Create camera with resilient transport
    let mut camera: Camera<PTZOpticsG2> = Camera::new(resilient);

    // Perform operations - the resilient transport handles failures transparently
    println!("Testing camera connection...");
    
    match camera.get_power_state().await {
        Ok(power) => println!("Camera power state: {:?}", power),
        Err(e) => println!("Failed to get power state: {}", e),
    }

    // Try some operations that might fail and be retried
    println!("\nPerforming camera operations...");
    
    // Home position
    if let Err(e) = camera.home().await {
        println!("Home command failed: {}", e);
    } else {
        println!("Camera moved to home position");
    }

    // Get position
    match camera.get_position().await {
        Ok((pan, tilt)) => {
            println!("Current position: pan={:?}, tilt={:?}", pan, tilt);
        }
        Err(e) => println!("Failed to get position: {}", e),
    }

    println!("\nDemo completed!");
    Ok(())
}

#[cfg(not(feature = "async-client"))]
fn main() {
    println!("This example requires the 'async-client' feature to be enabled.");
    println!("Run with: cargo run --example resilient_transport_no_clone --features async-client");
}