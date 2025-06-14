//! Example demonstrating health check and connection monitoring with blocking API.
//!
//! This example shows how to:
//! - Check camera connection health
//! - Monitor connection status
//! - Handle connection failures
//! - Use both UDP and TCP transports

#[cfg(feature = "blocking-client")]
use grafton_visca::command::power::Power;
#[cfg(feature = "blocking-client")]
use grafton_visca::command::{InquiryCommand, PowerCommand, Response};
#[cfg(feature = "blocking-client")]
use grafton_visca::{Client, Error};
#[cfg(feature = "blocking-client")]
use std::thread;
#[cfg(feature = "blocking-client")]
use std::time::Duration;

#[cfg(not(feature = "blocking-client"))]
fn main() {
    eprintln!("This example requires the 'blocking-client' feature.");
    eprintln!("Run with: cargo run --example health_check --features blocking-client");
}

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line or use default
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    println!("=== VISCA Health Check Example ===\n");

    // Test UDP connection
    test_udp_health(&camera_addr)?;

    println!("\n{}\n", "=".repeat(50));

    // Test TCP connection
    test_tcp_health(&camera_addr)?;

    Ok(())
}

#[cfg(feature = "blocking-client")]
fn test_udp_health(camera_addr: &str) -> Result<(), Error> {
    println!("Testing UDP connection to {}...", camera_addr);

    // Try to connect
    let client = match Client::connect_udp(camera_addr) {
        Ok(c) => {
            println!("✓ UDP connection established");
            c
        }
        Err(e) => {
            println!("✗ Failed to connect via UDP: {}", e);
            return Ok(());
        }
    };

    // Check initial health
    println!("\nInitial health check:");
    match client.is_healthy_blocking() {
        Ok(true) => println!("✓ Camera is responding to commands"),
        Ok(false) => println!("✗ Camera is not responding"),
        Err(e) => println!("✗ Error checking health: {}", e),
    }

    // Send some commands to generate activity
    println!("\nSending test commands...");

    // Power inquiry
    match client.send(&InquiryCommand::Power) {
        Ok(Response::InquiryResponse(resp)) => {
            println!("✓ Power inquiry succeeded: {:?}", resp);
        }
        Ok(Response::Ack) => println!("✓ Command acknowledged"),
        Ok(Response::Completion) => println!("✓ Command completed"),
        Ok(Response::Error(e)) => println!("✗ Camera returned error: {:?}", e),
        Ok(Response::Unknown(data)) => println!("? Unknown response: {:?}", data),
        Err(e) => println!("✗ Power inquiry failed: {}", e),
    }

    // Power on command
    match client.send(&PowerCommand { power: Power::On }) {
        Ok(_) => println!("✓ Power on command sent"),
        Err(e) => println!("✗ Power on command failed: {}", e),
    }

    // Periodic health checks
    println!("\nPerforming periodic health checks...");
    for i in 1..=5 {
        thread::sleep(Duration::from_secs(2));

        print!("Health check #{}: ", i);
        match client.is_healthy_blocking() {
            Ok(true) => println!("✓ Healthy"),
            Ok(false) => println!("✗ Not healthy"),
            Err(e) => println!("✗ Error: {}", e),
        }
    }

    Ok(())
}

#[cfg(feature = "blocking-client")]
fn test_tcp_health(camera_addr: &str) -> Result<(), Error> {
    println!("Testing TCP connection to {}...", camera_addr);

    // Try to connect
    let client = match Client::connect_tcp(camera_addr) {
        Ok(c) => {
            println!("✓ TCP connection established");
            c
        }
        Err(e) => {
            println!("✗ Failed to connect via TCP: {}", e);
            return Ok(());
        }
    };

    // Check initial health
    println!("\nInitial health check:");
    match client.is_healthy_blocking() {
        Ok(true) => println!("✓ Camera is responding to commands"),
        Ok(false) => println!("✗ Camera is not responding"),
        Err(e) => println!("✗ Error checking health: {}", e),
    }

    // Test rapid health checks
    println!("\nTesting rapid health checks...");
    let start = std::time::Instant::now();
    let mut success_count = 0;
    let mut failure_count = 0;

    for _ in 0..10 {
        match client.is_healthy_blocking() {
            Ok(true) => success_count += 1,
            Ok(false) => failure_count += 1,
            Err(_) => failure_count += 1,
        }
        thread::sleep(Duration::from_millis(100));
    }

    let elapsed = start.elapsed();
    println!("Completed 10 health checks in {:?}", elapsed);
    println!("Success: {}, Failures: {}", success_count, failure_count);

    // Demonstrate health check under load
    println!("\nHealth check while sending commands...");

    // Start a thread that continuously checks health
    let client_clone = client.clone();
    let health_thread = thread::spawn(move || {
        let mut healthy_count = 0;
        let mut check_count = 0;

        for _ in 0..10 {
            check_count += 1;
            if let Ok(true) = client_clone.is_healthy_blocking() {
                healthy_count += 1;
            }
            thread::sleep(Duration::from_millis(500));
        }

        (healthy_count, check_count)
    });

    // Send commands in the main thread
    for i in 1..=5 {
        println!("Sending command batch {}...", i);

        // Send multiple inquiries
        let _ = client.send(&InquiryCommand::Power);
        let _ = client.send(&InquiryCommand::ZoomPosition);
        let _ = client.send(&InquiryCommand::PanTiltPosition);

        thread::sleep(Duration::from_secs(1));
    }

    // Wait for health check thread to complete
    if let Ok((healthy, total)) = health_thread.join() {
        println!(
            "\nBackground health check results: {}/{} healthy",
            healthy, total
        );
    }

    println!("\nHealth check demo completed!");
    Ok(())
}
