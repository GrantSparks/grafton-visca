//! Example demonstrating async auto-reconnecting transport functionality.

#[cfg(feature = "async")]
use grafton_visca::{
    command::{PanTiltCommand, PowerCommand, ZoomCommand},
    AsyncConnectionManagement, AsyncReconnectingTransport, AsyncTcpTransport, AsyncUdpTransport,
    AsyncViscaTransport, ReconnectionConfig, ViscaError,
};
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

    // Concurrent operations test
    println!("\n2. Testing concurrent operations (simulating high load):");

    use tokio::task::JoinSet;
    let mut tasks = JoinSet::new();

    // Note: In real usage, you'd need to use Arc<Mutex<>> or similar for shared transport access
    // This is simplified for demonstration

    println!("  (This would require proper synchronization in real usage)");

    // Final health check
    match transport.is_healthy().await {
        Ok(true) => println!("\n✓ Final health check: Connection is healthy"),
        Ok(false) => println!("\n✗ Final health check: Connection is not healthy"),
        Err(e) => println!("\n✗ Error during final health check: {}", e),
    }

    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    println!("This example requires the 'async' feature to be enabled.");
    println!("Run with: cargo run --example async_reconnecting --features async");
}
