//! Example demonstrating auto-reconnecting transport functionality.
//!
//! This example shows how to use the ReconnectingTransport wrapper to handle
//! connection failures gracefully with automatic reconnection.

use grafton_visca::{
    command::{PanTiltCommand, PowerCommand, ZoomCommand},
    ConnectionManagement, ReconnectingTransport, ReconnectionConfig, TcpTransport, UdpTransport,
    ViscaError, ViscaTransport, ViscaTransportExt,
};
use std::io;
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Example 1: Auto-reconnecting UDP transport
    println!("=== Auto-Reconnecting UDP Transport Example ===\n");
    demo_udp_reconnection()?;

    println!("\n=== Auto-Reconnecting TCP Transport Example ===\n");
    demo_tcp_reconnection()?;

    Ok(())
}

fn demo_udp_reconnection() -> Result<(), Box<dyn std::error::Error>> {
    let camera_addr = "192.168.1.100:1259";

    // Configure reconnection behavior
    let reconnect_config = ReconnectionConfig {
        max_retries: 5,
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(30),
        backoff_factor: 2.0,
        health_check_interval: Some(Duration::from_secs(30)),
    };

    // Create the reconnecting transport
    let mut transport = ReconnectingTransport::new(
        || UdpTransport::new(camera_addr).map_err(|e| ViscaError::Io(e)),
        reconnect_config,
    )?;

    println!("Connected to camera at {}", camera_addr);

    // Demonstrate normal operation
    println!("\n1. Normal operation:");
    transport.power_on()?;
    println!("✓ Power on successful");

    transport.home()?;
    println!("✓ Home position set");

    // Check health
    match transport.is_healthy() {
        Ok(true) => println!("✓ Connection is healthy"),
        Ok(false) => println!("✗ Connection is not healthy"),
        Err(e) => println!("✗ Error checking health: {}", e),
    }

    // Simulate commands that might fail due to network issues
    println!("\n2. Sending multiple commands (will auto-reconnect if connection drops):");

    for i in 0..5 {
        println!("\nCommand batch {}:", i + 1);

        // These commands will automatically retry if the connection fails
        match transport.send_and_wait(&ZoomCommand::TeleStandard) {
            Ok(_) => println!("  ✓ Zoom tele command successful"),
            Err(e) => println!("  ✗ Zoom command failed after retries: {}", e),
        }

        thread::sleep(Duration::from_millis(500));

        match transport.send_and_wait(&PanTiltCommand::Left(5, 5)) {
            Ok(_) => println!("  ✓ Pan left command successful"),
            Err(e) => println!("  ✗ Pan command failed after retries: {}", e),
        }

        thread::sleep(Duration::from_secs(1));
    }

    // Get connection stats
    let stats = transport.connection_stats();
    println!("\n3. Connection Statistics:");
    println!("  Commands sent: {}", stats.commands_sent());
    println!("  Responses received: {}", stats.responses_received());
    println!("  Bytes sent: {}", stats.bytes_sent());
    println!("  Bytes received: {}", stats.bytes_received());
    println!("  Errors: {}", stats.error_count());

    Ok(())
}

fn demo_tcp_reconnection() -> Result<(), Box<dyn std::error::Error>> {
    let camera_addr = "192.168.1.100:5678";

    // Configure more aggressive reconnection for TCP
    let reconnect_config = ReconnectionConfig {
        max_retries: 10,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(10),
        backoff_factor: 1.5,
        health_check_interval: Some(Duration::from_secs(20)),
    };

    // Create the reconnecting transport
    let mut transport = ReconnectingTransport::new(
        || TcpTransport::new(camera_addr).map_err(|e| ViscaError::Io(e)),
        reconnect_config,
    )?;

    println!("Connected to camera at {} via TCP", camera_addr);

    // Test with preset operations
    println!("\n1. Testing preset operations with auto-reconnection:");

    // Save current position as preset 1
    match transport.save_preset(1) {
        Ok(_) => println!("✓ Saved preset 1"),
        Err(e) => println!("✗ Failed to save preset: {}", e),
    }

    // Move camera
    transport.send_and_wait(&PanTiltCommand::Right(10, 10))?;
    thread::sleep(Duration::from_secs(2));

    // Save as preset 2
    match transport.save_preset(2) {
        Ok(_) => println!("✓ Saved preset 2"),
        Err(e) => println!("✗ Failed to save preset: {}", e),
    }

    // Recall presets multiple times
    println!("\n2. Recalling presets (will auto-reconnect if needed):");
    for i in 0..3 {
        println!("\nIteration {}:", i + 1);

        match transport.recall_preset(1) {
            Ok(_) => println!("  ✓ Recalled preset 1"),
            Err(e) => println!("  ✗ Failed to recall preset 1: {}", e),
        }

        thread::sleep(Duration::from_secs(3));

        match transport.recall_preset(2) {
            Ok(_) => println!("  ✓ Recalled preset 2"),
            Err(e) => println!("  ✗ Failed to recall preset 2: {}", e),
        }

        thread::sleep(Duration::from_secs(3));
    }

    // Final health check
    match transport.is_healthy() {
        Ok(true) => println!("\n✓ Final health check: Connection is healthy"),
        Ok(false) => println!("\n✗ Final health check: Connection is not healthy"),
        Err(e) => println!("\n✗ Error during final health check: {}", e),
    }

    Ok(())
}
