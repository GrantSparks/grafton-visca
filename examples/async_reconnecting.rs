//! Example demonstrating async auto-reconnecting transport functionality.

#[cfg(feature = "async")]
use grafton_visca::{
    command::{PanTiltCommand, PowerCommand, ZoomCommand},
    AsyncConnectionManagement, AsyncReconnectingTransport, AsyncTcpTransport, AsyncUdpTransport,
    AsyncViscaTransport, ConnectionEvent, ConnectionEventCallback, ReconnectionConfig, ViscaError,
};
#[cfg(feature = "async")]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(feature = "async")]
use std::sync::{Arc, Mutex};
#[cfg(feature = "async")]
use std::time::Duration;
#[cfg(feature = "async")]
use tokio::time::sleep;

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Async Auto-Reconnecting Transport Example ===\n");

    // Example with UDP
    demo_async_udp_reconnection().await?;

    println!("\n");

    // Example with TCP
    demo_async_tcp_reconnection().await?;

    println!("\n");

    // Example with event monitoring
    demo_async_event_monitoring().await?;

    Ok(())
}

#[cfg(feature = "async")]
async fn demo_async_udp_reconnection() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;

    let camera_addr: SocketAddr = "192.168.1.100:1259".parse()?;

    // Configure reconnection behavior
    let reconnect_config = ReconnectionConfig {
        max_retries: 5,
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(30),
        backoff_factor: 2.0,
        health_check_interval: Some(Duration::from_secs(30)),
    };

    // Create the async reconnecting transport
    let mut transport = AsyncReconnectingTransport::new(
        move || async move { AsyncUdpTransport::new(camera_addr).await },
        reconnect_config,
    )
    .await?;

    println!("Connected to camera at {} via async UDP", camera_addr);

    // Test basic operations
    println!("\n1. Testing basic operations with auto-reconnection:");

    let power_cmd = PowerCommand::on();
    match transport.send_and_wait(&power_cmd).await {
        Ok(_) => println!("✓ Power on successful"),
        Err(e) => println!("✗ Power on failed: {}", e),
    }

    // Pan/Tilt operations
    println!("\n2. Testing pan/tilt with potential reconnections:");

    for i in 0..3 {
        println!("\nMovement cycle {}:", i + 1);

        // Move right
        match transport.send_and_wait(&PanTiltCommand::Right(8, 8)).await {
            Ok(_) => println!("  ✓ Pan right successful"),
            Err(e) => println!("  ✗ Pan right failed: {}", e),
        }

        sleep(Duration::from_secs(2)).await;

        // Move left
        match transport.send_and_wait(&PanTiltCommand::Left(8, 8)).await {
            Ok(_) => println!("  ✓ Pan left successful"),
            Err(e) => println!("  ✗ Pan left failed: {}", e),
        }

        sleep(Duration::from_secs(2)).await;
    }

    // Health check
    match transport.is_healthy().await {
        Ok(true) => println!("\n✓ Connection is healthy"),
        Ok(false) => println!("\n✗ Connection is not healthy"),
        Err(e) => println!("\n✗ Error checking health: {}", e),
    }

    // Get statistics using the new async methods
    let stats = transport.connection_stats_mut().await;
    let snapshot = stats.snapshot();
    println!("\n3. Connection Statistics:");
    println!("  Commands sent: {}", snapshot.commands_sent);
    println!("  Responses received: {}", snapshot.responses_received);
    println!("  Errors: {}", snapshot.error_count);

    // Get combined stats
    let combined = transport.combined_stats().await;
    let combined_snapshot = combined.snapshot();
    println!("\n4. Combined Statistics (wrapper + transport):");
    println!("  Total commands: {}", combined_snapshot.commands_sent);
    println!("  Total errors: {}", combined_snapshot.error_count);

    Ok(())
}

#[cfg(feature = "async")]
async fn demo_async_tcp_reconnection() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;

    let camera_addr: SocketAddr = "192.168.1.100:5678".parse()?;

    // Configure more aggressive reconnection for TCP
    let reconnect_config = ReconnectionConfig {
        max_retries: 10,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(10),
        backoff_factor: 1.5,
        health_check_interval: Some(Duration::from_secs(20)),
    };

    // Create the async reconnecting transport
    let mut transport = AsyncReconnectingTransport::new(
        move || async move { AsyncTcpTransport::new(camera_addr).await },
        reconnect_config,
    )
    .await?;

    println!("Connected to camera at {} via async TCP", camera_addr);

    // Test zoom operations
    println!("\n1. Testing zoom operations with auto-reconnection:");

    // Zoom in and out multiple times
    for i in 0..3 {
        println!("\nZoom cycle {}:", i + 1);

        // Zoom in
        match transport.send_and_wait(&ZoomCommand::TeleStandard).await {
            Ok(_) => println!("  ✓ Zoom in successful"),
            Err(e) => println!("  ✗ Zoom in failed: {}", e),
        }

        sleep(Duration::from_millis(500)).await;

        // Stop zoom
        match transport.send_and_wait(&ZoomCommand::Stop).await {
            Ok(_) => println!("  ✓ Zoom stop successful"),
            Err(e) => println!("  ✗ Zoom stop failed: {}", e),
        }

        sleep(Duration::from_secs(1)).await;

        // Zoom out
        match transport.send_and_wait(&ZoomCommand::WideStandard).await {
            Ok(_) => println!("  ✓ Zoom out successful"),
            Err(e) => println!("  ✗ Zoom out failed: {}", e),
        }

        sleep(Duration::from_millis(500)).await;

        // Stop zoom
        match transport.send_and_wait(&ZoomCommand::Stop).await {
            Ok(_) => println!("  ✓ Zoom stop successful"),
            Err(e) => println!("  ✗ Zoom stop failed: {}", e),
        }

        sleep(Duration::from_secs(1)).await;
    }

    // Final health check
    match transport.is_healthy().await {
        Ok(true) => println!("\n✓ Final health check: Connection is healthy"),
        Ok(false) => println!("\n✗ Final health check: Connection is not healthy"),
        Err(e) => println!("\n✗ Error during final health check: {}", e),
    }

    Ok(())
}

#[cfg(feature = "async")]
async fn demo_async_event_monitoring() -> Result<(), Box<dyn std::error::Error>> {
    use std::net::SocketAddr;

    println!("=== Async Connection Event Monitoring ===");

    let camera_addr: SocketAddr = "192.168.1.100:1259".parse()?;

    // Track connection events
    let events = Arc::new(Mutex::new(Vec::<ConnectionEvent>::new()));
    let events_clone = events.clone();

    // Configure reconnection with shorter delays for demo
    let reconnect_config = ReconnectionConfig {
        max_retries: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(2),
        backoff_factor: 2.0,
        health_check_interval: Some(Duration::from_secs(5)),
    };

    // Create transport with simulated failures
    let fail_count = Arc::new(AtomicUsize::new(0));
    let should_fail = Arc::new(AtomicBool::new(false));
    let fail_count_clone = fail_count.clone();
    let should_fail_clone = should_fail.clone();

    let mut transport = AsyncReconnectingTransport::new(
        move || {
            let fail_count = fail_count_clone.clone();
            let should_fail = should_fail_clone.clone();

            async move {
                let count = fail_count.fetch_add(1, Ordering::SeqCst);

                // Simulate failures on attempts 2-4
                if count >= 2 && count <= 4 {
                    Err(ViscaError::Io(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        "Simulated async connection failure",
                    )))
                } else {
                    AsyncUdpTransport::new(camera_addr).await
                }
            }
        },
        reconnect_config,
    )
    .await?;

    // Set up event callback
    transport.set_event_callback(Arc::new(move |event| {
        let mut event_list = events_clone.lock().unwrap();

        // Print event with emoji indicators
        match &event {
            ConnectionEvent::Connected => {
                println!("🟢 ASYNC EVENT: Connection established");
            }
            ConnectionEvent::Disconnected { reason } => {
                println!("🔴 ASYNC EVENT: Connection lost - {}", reason);
            }
            ConnectionEvent::ReconnectingStarted {
                attempt,
                max_attempts,
            } => {
                println!(
                    "🔄 ASYNC EVENT: Reconnection attempt {}/{}",
                    attempt, max_attempts
                );
            }
            ConnectionEvent::ReconnectingFailed { attempt, error } => {
                println!("❌ ASYNC EVENT: Attempt {} failed - {}", attempt, error);
            }
            ConnectionEvent::ReconnectionExhausted => {
                println!("⛔ ASYNC EVENT: All attempts exhausted");
            }
        }

        event_list.push(event);
    }));

    println!("\nSending commands to trigger connection events...\n");

    // First command should work
    match transport.send_and_wait(&PowerCommand::on()).await {
        Ok(_) => println!("✓ Initial command successful"),
        Err(e) => println!("✗ Initial command failed: {}", e),
    }

    // Force connection failures
    fail_count.store(1, Ordering::SeqCst);
    should_fail.store(true, Ordering::SeqCst);

    // This will trigger reconnection
    match transport.send_and_wait(&ZoomCommand::TeleStandard).await {
        Ok(_) => println!("✓ Command after failure successful"),
        Err(e) => println!("✗ Command after failure failed: {}", e),
    }

    // Allow some time for events
    sleep(Duration::from_secs(1)).await;

    // Try one more command
    match transport.send_and_wait(&PanTiltCommand::Home).await {
        Ok(_) => println!("✓ Final command successful"),
        Err(e) => println!("✗ Final command failed: {}", e),
    }

    // Event summary
    println!("\n📊 Async Event Summary:");
    let event_list = events.lock().unwrap();
    println!("Total events: {}", event_list.len());

    for (i, event) in event_list.iter().enumerate() {
        print!("  {}. ", i + 1);
        match event {
            ConnectionEvent::Connected => println!("Connected"),
            ConnectionEvent::Disconnected { .. } => println!("Disconnected"),
            ConnectionEvent::ReconnectingStarted {
                attempt,
                max_attempts,
            } => {
                println!("Reconnecting {}/{}", attempt, max_attempts)
            }
            ConnectionEvent::ReconnectingFailed { attempt, .. } => {
                println!("Attempt {} failed", attempt)
            }
            ConnectionEvent::ReconnectionExhausted => println!("Exhausted"),
        }
    }

    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("This example requires the 'async' feature to be enabled.");
    println!("Run with: cargo run --example async_reconnecting --features async");
}
