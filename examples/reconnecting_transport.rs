//! Example demonstrating auto-reconnecting transport functionality.
//!
//! This example shows how to use the `ReconnectingTransport` wrapper to handle
//! connection failures gracefully with automatic reconnection.

// TODO: Update this example for v0.5.0 - ReconnectingTransport is not yet available
fn main() {
    println!("This example needs to be updated for v0.5.0");
    println!("ReconnectingTransport is not yet available in the current version");
}

/*
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
    );

    println!("Created reconnecting UDP transport for {}", camera_addr);
    println!("Configuration:");
    println!("  Max retries: {}", reconnect_config.max_retries);
    println!("  Initial delay: {:?}", reconnect_config.initial_delay);
    println!("  Max delay: {:?}", reconnect_config.max_delay);
    println!("  Backoff factor: {}", reconnect_config.backoff_factor);
    println!(
        "  Health check interval: {:?}",
        reconnect_config.health_check_interval
    );

    // Try some commands
    println!("\nTesting commands with automatic reconnection...");

    // Power on
    match transport.send_and_wait(&PowerCommand { power: Power::On }) {
        Ok(_) => println!("✓ Power on command sent successfully"),
        Err(e) => println!("✗ Power on failed: {}", e),
    }

    // Home position
    match transport.home() {
        Ok(_) => println!("✓ Home command sent successfully"),
        Err(e) => println!("✗ Home command failed: {}", e),
    }

    // Simulate network issues by attempting many rapid commands
    println!("\nSimulating rapid command sequence (may trigger reconnection)...");
    for i in 1..=10 {
        thread::sleep(Duration::from_millis(100));

        let direction = if i % 2 == 0 {
            PanTiltDirection::Right
        } else {
            PanTiltDirection::Left
        };

        match transport.send_and_wait(&PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::new(5).unwrap(),
            tilt_speed: TiltSpeed::new(0).unwrap(),
        }) {
            Ok(_) => print!("."),
            Err(_) => print!("!"),
        }
    }
    println!();

    // Stop movement
    let _ = transport.send_and_wait(&PanTiltCommand::Move {
        direction: PanTiltDirection::Stop,
        pan_speed: PanSpeed::new(0).unwrap(),
        tilt_speed: TiltSpeed::new(0).unwrap(),
    });

    // Check connection statistics
    let stats = transport.connection_stats().snapshot();
    println!("\nConnection Statistics:");
    println!("  Commands sent: {}", stats.commands_sent);
    println!("  Responses received: {}", stats.responses_received);
    println!("  Errors: {}", stats.error_count);
    println!("  Reconnection attempts: {}", stats.reconnection_attempts);
    println!("  Successful reconnections: {}", stats.successful_reconnections);

    Ok(())
}

fn demo_tcp_reconnection() -> Result<(), Box<dyn std::error::Error>> {
    let camera_addr = "192.168.1.100:5678";

    // More aggressive reconnection for TCP
    let reconnect_config = ReconnectionConfig {
        max_retries: 10,
        initial_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(60),
        backoff_factor: 1.5,
        health_check_interval: Some(Duration::from_secs(20)),
    };

    let mut transport = ReconnectingTransport::new(
        || TcpTransport::new(camera_addr).map_err(ViscaError::Io),
        reconnect_config,
    );

    println!("Created reconnecting TCP transport for {}", camera_addr);

    // Test zoom commands
    println!("\nTesting zoom commands...");

    for zoom_level in [0x0000, 0x2000, 0x4000, 0x2000, 0x0000] {
        match transport.zoom_to_position(zoom_level) {
            Ok(_) => println!("✓ Zoomed to position 0x{:04X}", zoom_level),
            Err(e) => println!("✗ Zoom failed: {}", e),
        }
        thread::sleep(Duration::from_secs(2));
    }

    Ok(())
}

fn demo_connection_events() -> Result<(), Box<dyn std::error::Error>> {
    let camera_addr = "192.168.1.100:1259";

    // Create a flaky transport factory that fails intermittently
    let failure_counter = Arc::new(Mutex::new(0));
    let failure_counter_clone = failure_counter.clone();

    let transport_factory = move || {
        let mut counter = failure_counter_clone.lock().unwrap();
        *counter += 1;

        // Simulate failures on attempts 3, 4, 7, 8
        if *counter == 3 || *counter == 4 || *counter == 7 || *counter == 8 {
            println!("  [Factory] Simulating connection failure (attempt {})", *counter);
            Err(ViscaError::Io(io::Error::new(
                io::ErrorKind::ConnectionRefused,
                "Simulated connection failure",
            )))
        } else {
            println!("  [Factory] Creating transport (attempt {})", *counter);
            UdpTransport::new(camera_addr).map_err(ViscaError::Io)
        }
    };

    let reconnect_config = ReconnectionConfig {
        max_retries: 3,
        initial_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(5),
        backoff_factor: 2.0,
        health_check_interval: None,
    };

    let mut transport = ReconnectingTransport::new(transport_factory, reconnect_config);

    // Set up event callback to monitor connection events
    let event_log = Arc::new(Mutex::new(Vec::new()));
    let event_log_clone = event_log.clone();

    transport.set_event_callback(Some(Box::new(move |event| {
        let mut log = event_log_clone.lock().unwrap();
        log.push(event.clone());

        match event {
            ConnectionEvent::Connected => println!("  📡 EVENT: Connected"),
            ConnectionEvent::Disconnected(reason) => {
                println!("  ❌ EVENT: Disconnected - {}", reason)
            }
            ConnectionEvent::ReconnectAttempt { attempt, max } => {
                println!("  🔄 EVENT: Reconnect attempt {}/{}", attempt, max)
            }
            ConnectionEvent::ReconnectSuccess => println!("  ✅ EVENT: Reconnect successful"),
            ConnectionEvent::ReconnectFailed => println!("  ❌ EVENT: Reconnect failed"),
            ConnectionEvent::HealthCheckPassed => println!("  ✅ EVENT: Health check passed"),
            ConnectionEvent::HealthCheckFailed(reason) => {
                println!("  ❌ EVENT: Health check failed - {}", reason)
            }
        }
    })));

    println!("Monitoring connection events...\n");

    // Perform operations that will trigger connection events
    for i in 1..=10 {
        println!("\nOperation {}:", i);

        match transport.send_and_wait(&PowerCommand { power: Power::On }) {
            Ok(_) => println!("  ✓ Command successful"),
            Err(e) => println!("  ✗ Command failed: {}", e),
        }

        thread::sleep(Duration::from_millis(500));
    }

    // Display event summary
    println!("\n=== Event Summary ===");
    let events = event_log.lock().unwrap();
    let connected_count = events
        .iter()
        .filter(|e| matches!(e, ConnectionEvent::Connected))
        .count();
    let disconnected_count = events
        .iter()
        .filter(|e| matches!(e, ConnectionEvent::Disconnected(_)))
        .count();
    let reconnect_attempts = events
        .iter()
        .filter(|e| matches!(e, ConnectionEvent::ReconnectAttempt { .. }))
        .count();
    let reconnect_success = events
        .iter()
        .filter(|e| matches!(e, ConnectionEvent::ReconnectSuccess))
        .count();

    println!("Total events: {}", events.len());
    println!("Connected: {}", connected_count);
    println!("Disconnected: {}", disconnected_count);
    println!("Reconnect attempts: {}", reconnect_attempts);
    println!("Successful reconnections: {}", reconnect_success);

    Ok(())
}
*/
