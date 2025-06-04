//! Example demonstrating auto-reconnecting transport functionality.
//!
//! This example shows how to use the ReconnectingTransport wrapper to handle
//! connection failures gracefully with automatic reconnection.

use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, PanTiltCommand, PanTiltDirection, TiltSpeed},
        power::{Power, PowerCommand},
        ZoomCommand,
    },
    ConnectionEvent, ConnectionManagement, ReconnectingTransport, ReconnectionConfig, TcpTransport,
    UdpTransport, ViscaError, ViscaTransport, ViscaTransportExt,
};
use std::io;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Example 1: Auto-reconnecting UDP transport with event callbacks
    println!("=== Auto-Reconnecting UDP Transport Example ===\n");
    demo_udp_reconnection()?;

    println!("\n=== Auto-Reconnecting TCP Transport Example ===\n");
    demo_tcp_reconnection()?;

    println!("\n=== Connection Event Monitoring Example ===\n");
    demo_connection_events()?;

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
        || UdpTransport::new(camera_addr).map_err(ViscaError::Io),
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

        match transport.send_and_wait(&PanTiltCommand::Move {
            direction: PanTiltDirection::Left,
            pan_speed: PanSpeed::new(5).unwrap(),
            tilt_speed: TiltSpeed::new(5).unwrap(),
        }) {
            Ok(_) => println!("  ✓ Pan left command successful"),
            Err(e) => println!("  ✗ Pan command failed after retries: {}", e),
        }

        thread::sleep(Duration::from_secs(1));
    }

    // Get connection stats using the new methods
    let stats = transport.stats_snapshot();
    let snapshot = stats.snapshot();
    println!("\n3. Connection Statistics:");
    println!("  Commands sent: {}", snapshot.commands_sent);
    println!("  Responses received: {}", snapshot.responses_received);
    println!("  Bytes sent: {}", snapshot.bytes_sent);
    println!("  Bytes received: {}", snapshot.bytes_received);
    println!("  Errors: {}", snapshot.error_count);

    // Get combined stats (wrapper + inner transport)
    let combined = transport.combined_stats();
    let combined_snapshot = combined.snapshot();
    println!("\n4. Combined Statistics (wrapper + transport):");
    println!("  Total commands: {}", combined_snapshot.commands_sent);
    println!("  Total errors: {}", combined_snapshot.error_count);

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
        || TcpTransport::new(camera_addr).map_err(ViscaError::Io),
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
    transport.send_and_wait(&PanTiltCommand::Move {
        direction: PanTiltDirection::Right,
        pan_speed: PanSpeed::new(10).unwrap(),
        tilt_speed: TiltSpeed::new(10).unwrap(),
    })?;
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

/// Demonstrates connection event monitoring
fn demo_connection_events() -> Result<(), Box<dyn std::error::Error>> {
    let camera_addr = "192.168.1.100:1259";

    // Track connection events
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    // Configure reconnection with shorter delays for demo
    let reconnect_config = ReconnectionConfig {
        max_retries: 3,
        initial_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(2),
        backoff_factor: 2.0,
        health_check_interval: Some(Duration::from_secs(5)),
    };

    // Create transport with a simulated failing connection
    let fail_count = Arc::new(Mutex::new(0));
    let fail_count_clone = fail_count.clone();

    let mut transport = ReconnectingTransport::new(
        move || {
            let mut count = fail_count_clone.lock().unwrap();
            *count += 1;

            // Simulate connection failures on attempts 2-4
            if *count >= 2 && *count <= 4 {
                Err(ViscaError::Io(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "Simulated connection failure for demo",
                )))
            } else {
                UdpTransport::new(camera_addr).map_err(ViscaError::Io)
            }
        },
        reconnect_config,
    )?;

    // Set up event callback
    transport.set_event_callback(Arc::new(move |event| {
        let mut event_list = events_clone.lock().unwrap();

        // Print event as it happens
        match &event {
            ConnectionEvent::Connected => {
                println!("📡 EVENT: Connection established");
            }
            ConnectionEvent::Disconnected { reason } => {
                println!("🔌 EVENT: Connection lost - {}", reason);
            }
            ConnectionEvent::ReconnectingStarted {
                attempt,
                max_attempts,
            } => {
                println!(
                    "🔄 EVENT: Reconnection attempt {}/{}",
                    attempt, max_attempts
                );
            }
            ConnectionEvent::ReconnectingFailed { attempt, error } => {
                println!(
                    "❌ EVENT: Reconnection attempt {} failed - {}",
                    attempt, error
                );
            }
            ConnectionEvent::ReconnectionExhausted => {
                println!("⛔ EVENT: All reconnection attempts exhausted");
            }
        }

        event_list.push(event);
    }));

    println!("Monitoring connection events...\n");

    // Send commands that will trigger reconnection
    println!("Sending commands (will trigger simulated failures):");

    // This should work
    match transport.send_and_wait(&PowerCommand { power: Power::On }) {
        Ok(_) => println!("✓ Power on successful"),
        Err(e) => println!("✗ Power on failed: {}", e),
    }

    // Force a failure by incrementing count
    *fail_count.lock().unwrap() = 1;

    // This will fail and trigger reconnection
    match transport.send_and_wait(&ZoomCommand::WideStandard) {
        Ok(_) => println!("✓ Zoom command successful"),
        Err(e) => println!("✗ Zoom command failed: {}", e),
    }

    // Wait a bit to see reconnection events
    thread::sleep(Duration::from_secs(1));

    // Try another command (should eventually succeed after reconnection)
    match transport.send_and_wait(&PanTiltCommand::Home) {
        Ok(_) => println!("✓ Home command successful"),
        Err(e) => println!("✗ Home command failed: {}", e),
    }

    // Display event summary
    println!("\n📊 Connection Event Summary:");
    let event_list = events.lock().unwrap();
    println!("Total events recorded: {}", event_list.len());

    let disconnects = event_list
        .iter()
        .filter(|e| matches!(e, ConnectionEvent::Disconnected { .. }))
        .count();
    let reconnect_attempts = event_list
        .iter()
        .filter(|e| matches!(e, ConnectionEvent::ReconnectingStarted { .. }))
        .count();
    let reconnect_failures = event_list
        .iter()
        .filter(|e| matches!(e, ConnectionEvent::ReconnectingFailed { .. }))
        .count();
    let successful_connections = event_list
        .iter()
        .filter(|e| matches!(e, ConnectionEvent::Connected))
        .count();

    println!("  Disconnections: {}", disconnects);
    println!("  Reconnection attempts: {}", reconnect_attempts);
    println!("  Failed attempts: {}", reconnect_failures);
    println!("  Successful connections: {}", successful_connections);

    Ok(())
}
