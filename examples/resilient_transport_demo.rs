//! Example demonstrating the ResilientTransport for automatic retry and reconnection
//!
//! This example shows how to wrap any transport with resilient behavior to handle
//! network issues gracefully. It demonstrates both sync and async factory patterns.

#[cfg(feature = "async-client")]
use grafton_visca::{
    camera::units::Degrees,
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

    // Example 1: Using async factory with AsyncTcpTransport
    println!("Example 1: Async factory with AsyncTcpTransport");
    let tcp_addr = "192.168.1.100:1259";
    let tcp_transport = AsyncTcpTransport::new(tcp_addr).await?;

    // Create resilient transport with async factory
    let resilient_tcp = ResilientTransport::new_async(
        tcp_transport,
        move || async move {
            // Async factory function
            println!("Creating new async TCP transport...");
            AsyncTcpTransport::new(tcp_addr)
                .await
                .map_err(|e| grafton_visca::Error::Io(e))
        },
        config.clone(),
    );

    // Example 2: Using async factory with AsyncUdpTransport
    println!("\nExample 2: Async factory with AsyncUdpTransport");
    let udp_addr = "192.168.1.100:52381";
    let udp_transport = AsyncUdpTransport::new(udp_addr).await?;

    // Create resilient transport with async factory
    let resilient_udp = ResilientTransport::new_async(
        udp_transport,
        move || async move {
            // Async factory function
            println!("Creating new UDP transport...");
            AsyncUdpTransport::new(udp_addr)
                .await
                .map_err(|e| grafton_visca::Error::Io(e))
        },
        config.clone(),
    );

    // Example 3: Using async factory with AsyncTcpTransport
    println!("\nExample 3: Async factory with AsyncTcpTransport");
    let async_tcp_addr = "192.168.1.100:1259";
    let async_tcp_transport = AsyncTcpTransport::new(async_tcp_addr).await?;

    // Create resilient transport with async factory
    let mut resilient_async_tcp = ResilientTransport::new_async(
        async_tcp_transport,
        move || async move {
            println!("Creating new async TCP transport...");
            AsyncTcpTransport::new(async_tcp_addr)
                .await
                .map_err(|e| grafton_visca::Error::Io(e))
        },
        config,
    );

    // Set up event callback to monitor resilience events
    resilient_async_tcp.set_event_callback(Arc::new(|event| match event {
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

    // Create camera with resilient transport (using async TCP for demo)
    let camera: Camera<PTZOpticsG2> = Camera::new(resilient_async_tcp);

    // Normal camera operations - resilient transport handles failures transparently
    println!("\nMoving camera to home position...");
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

    // Note: In a real application, you would keep a reference to the resilient transport
    // to access statistics. For this demo, we'll show the pattern with the TCP transport.
    println!("\nShowing statistics pattern with TCP transport:");
    let stats = resilient_tcp.stats();
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
