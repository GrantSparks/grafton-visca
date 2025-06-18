//! Simplified reconnecting camera using ResilientTransport.
//!
//! Compare this with async_reconnecting.rs to see how much simpler
//! the new ResilientTransport makes automatic reconnection.
//!
//! NOTE: This example requires a transport that implements Clone.
//! AsyncUdpTransport currently doesn't implement Clone, so this example
//! won't compile until that's fixed or a different transport is used.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    transport::{
        resilient::{ResilienceConfig, ResilienceEvent, ResilientTransport},
        AsyncUdpTransport,
    },
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Resilient Camera Demo ===");
    println!("This replaces ~200 lines of manual retry logic with a simple wrapper!\n");

    // Configure resilience behavior
    let config = ResilienceConfig {
        max_retries: 3,
        initial_retry_delay: Duration::from_millis(250),
        max_retry_delay: Duration::from_secs(10),
        backoff_factor: 2.0,
        max_reconnect_attempts: 10,
        reconnect_delay: Duration::from_secs(2),
        health_check_interval: Some(Duration::from_secs(30)),
        operation_timeout: Duration::from_secs(5),
    };

    // Create resilient transport
    let camera_ip = "192.168.1.100:52381";
    let base_transport = AsyncUdpTransport::new(camera_ip).await?;

    let mut resilient_transport = ResilientTransport::new_async(
        base_transport,
        move || {
            let ip = camera_ip.to_string();
            Box::pin(async move {
                AsyncUdpTransport::new(&ip)
                    .await
                    .map_err(|e| grafton_visca::Error::Io(e))
            })
        },
        config,
    );

    // Add event monitoring
    resilient_transport.set_event_callback(Arc::new(|event| match event {
        ResilienceEvent::OperationSucceeded { retries } if retries > 0 => {
            log::info!("✓ Operation succeeded after {} retries", retries);
        }
        ResilienceEvent::OperationFailed { attempts, error } => {
            log::error!("✗ Operation failed after {} attempts: {}", attempts, error);
        }
        ResilienceEvent::Reconnected { attempts } => {
            log::info!("↻ Successfully reconnected after {} attempts", attempts);
        }
        ResilienceEvent::ReconnectionFailed { attempts, error } => {
            log::error!(
                "✗ Reconnection failed after {} attempts: {}",
                attempts,
                error
            );
        }
        ResilienceEvent::HealthCheckPassed => {
            log::debug!("♥ Health check passed");
        }
        ResilienceEvent::HealthCheckFailed { error } => {
            log::warn!("⚠ Health check failed: {}", error);
        }
        _ => {}
    }));

    // Create camera - all operations will now automatically retry!
    let camera = Camera::<PTZOpticsG2>::new(resilient_transport);

    println!("Camera created with automatic retry and reconnection.");
    println!("Try disconnecting the camera network to see reconnection in action!\n");

    // Normal camera operations - resilience is transparent
    loop {
        println!("Executing camera operations...");

        // Power query - will automatically retry on failure
        match camera.get_power_state().await {
            Ok(is_on) => {
                println!("  Power state: {}", if is_on { "ON" } else { "OFF" });

                if !is_on {
                    println!("  Powering on...");
                    camera.power_on().await?;
                }
            }
            Err(e) => {
                println!("  Failed to query power state: {}", e);
                // Even with retries, some failures may be permanent
                continue;
            }
        }

        // Get position - automatic retry on network issues
        match camera.get_position().await {
            Ok((pan, tilt)) => {
                println!("  Current position: pan={:.1}°, tilt={:.1}°", pan.0, tilt.0);
            }
            Err(e) => {
                println!("  Failed to get position: {}", e);
            }
        }

        // Get zoom - automatic retry on network issues
        match camera.get_zoom_position().await {
            Ok(zoom) => {
                println!("  Zoom position: {}", zoom);
            }
            Err(e) => {
                println!("  Failed to get zoom: {}", e);
            }
        }

        // Print statistics (would need downcast to access ResilientTransport stats)
        println!("\nResilience Statistics:");
        println!("  (Statistics available via ResilientTransport reference)");

        // Wait before next iteration
        println!("\nWaiting 5 seconds before next check...\n");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

// Compare with async_reconnecting.rs which has:
// - Manual exponential backoff implementation
// - Custom retry logic for each operation
// - Manual health check implementation
// - Manual reconnection state machine
// - ~200+ lines of boilerplate code
//
// All replaced by simply wrapping the transport with ResilientTransport!
