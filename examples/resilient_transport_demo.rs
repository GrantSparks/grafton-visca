//! Example demonstrating the ResilientTransport for automatic retry and reconnection
//!
//! This example shows how to wrap any transport with resilient behavior to handle
//! network issues gracefully.

#[cfg(feature = "async-client")]
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    transport::{
        resilient::{ResilienceConfig, ResilienceEvent, ResilientTransport},
        unified::UnifiedTransport,
        Transport,
    },
    types::Degrees,
    Error,
};
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Configure resilient behavior
    let config = ResilienceConfig {
        max_retries: 5,
        initial_retry_delay: Duration::from_millis(100),
        max_retry_delay: Duration::from_secs(5),
        backoff_factor: 2.0,
        max_reconnect_attempts: 10,
        reconnect_delay: Duration::from_secs(2),
        health_check_interval: Some(Duration::from_secs(30)),
        operation_timeout: Duration::from_secs(5),
    };

    // Create the base transport
    let base_transport = UnifiedTransport::create_udp("192.168.1.100:1259").await?;

    // Wrap with resilient transport
    let resilient = ResilientTransport::new(
        base_transport,
        config,
        Box::new(|| {
            // Factory function to recreate transport on failure
            Box::pin(async move {
                UnifiedTransport::create_udp("192.168.1.100:1259")
                    .await
                    .map(|t| Box::new(t) as Box<dyn Transport>)
            })
        }),
    );

    // Set up event callback to monitor resilience events
    resilient.set_event_callback(Arc::new(|event| match event {
        ResilienceEvent::OperationSucceeded { retries } => {
            if retries > 0 {
                println!("Operation succeeded after {} retries", retries);
            }
        }
        ResilienceEvent::OperationFailed { attempts, error } => {
            println!("Operation failed after {} attempts: {}", attempts, error);
        }
        ResilienceEvent::Reconnected { attempts } => {
            println!("Reconnected after {} attempts", attempts);
        }
        ResilienceEvent::ReconnectionFailed { attempts, error } => {
            println!("Reconnection failed after {} attempts: {}", attempts, error);
        }
        ResilienceEvent::HealthCheckPassed => {
            println!("Health check passed");
        }
        ResilienceEvent::HealthCheckFailed { error } => {
            println!("Health check failed: {}", error);
        }
    }));

    // Create camera with resilient transport
    let mut camera: Camera<PTZOpticsG2> = Camera::new(resilient);

    // Normal camera operations - resilient transport handles failures transparently
    println!("Moving camera to home position...");
    camera.home().await?;

    // Simulate some operations that might fail
    println!("\nPerforming camera operations...");
    for i in 1..=5 {
        println!("Operation {}", i);
        match camera.get_position().await {
            Ok((pan, tilt)) => {
                let Degrees(pan_deg) = pan;
                let Degrees(tilt_deg) = tilt;
                println!("  Position: pan={:.1}°, tilt={:.1}°", pan_deg, tilt_deg);
            }
            Err(e) => println!("  Failed to get position: {}", e),
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Get statistics
    let stats = resilient.stats();
    println!("\nResilience Statistics:");
    println!("  Total operations: {}", stats.total_operations);
    println!("  First try successes: {}", stats.first_try_successes);
    println!("  Retried successes: {}", stats.retry_successes);
    println!("  Total failures: {}", stats.failures);
    println!("  Total retries: {}", stats.total_retries);
    println!(
        "  Successful reconnections: {}",
        stats.successful_reconnections
    );
    println!("  Failed reconnections: {}", stats.failed_reconnections);

    println!("\nDemo completed!");
    Ok(())
}

#[cfg(not(feature = "async-client"))]
fn main() {
    println!("This example requires the 'async-client' feature to be enabled.");
    println!("Run with: cargo run --example resilient_transport_demo --features async-client");
}
