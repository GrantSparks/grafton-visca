//! Example program

//! Example demonstrating async health check functionality.
//!
//! This example shows how to:
//! - Check camera connection health asynchronously
//! - Perform periodic health checks
//! - Monitor connection status over time
//! - Handle connection failures gracefully

use grafton_visca::{Client, Error};
use std::env;
use tokio::time::{sleep, Duration};

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example async_health_check --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <camera_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:5678", args[0]);
        std::process::exit(1);
    }

    // Connect to camera
    let camera_addr = &args[1];
    println!("Connecting to camera at {}...", camera_addr);

    // Try UDP connection
    println!("\n=== Testing UDP Connection ===");
    match Client::connect_udp_async(camera_addr).await {
        Ok(client) => {
            println!("✓ UDP connection established");

            // Check initial health
            match client.is_healthy().await {
                Ok(true) => println!("✓ Camera is responding to commands"),
                Ok(false) => println!("✗ Camera is not responding"),
                Err(e) => println!("✗ Error checking health: {}", e),
            }

            // Perform periodic health checks
            println!("\nPerforming periodic health checks (every 2 seconds)...");
            for i in 1..=5 {
                sleep(Duration::from_secs(2)).await;
                print!("Health check #{}: ", i);
                match client.is_healthy().await {
                    Ok(true) => println!("✓ Healthy"),
                    Ok(false) => println!("✗ Not healthy"),
                    Err(e) => println!("✗ Error: {}", e),
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to connect via UDP: {}", e);
        }
    }

    // Try TCP connection
    println!("\n=== Testing TCP Connection ===");
    match Client::connect_tcp_async(camera_addr).await {
        Ok(client) => {
            println!("✓ TCP connection established");

            // Check initial health
            match client.is_healthy().await {
                Ok(true) => println!("✓ Camera is responding to commands"),
                Ok(false) => println!("✗ Camera is not responding"),
                Err(e) => println!("✗ Error checking health: {}", e),
            }

            // Test connection resilience
            println!("\nTesting connection resilience...");
            for i in 1..=3 {
                println!("\nRound {}/3:", i);

                // Send multiple health checks in quick succession
                let mut results = Vec::new();
                for j in 1..=5 {
                    let health_result = client.is_healthy().await;
                    results.push(health_result);
                    print!("  Check {}: ", j);
                    match &results[j - 1] {
                        Ok(true) => println!("✓"),
                        Ok(false) => println!("✗"),
                        Err(e) => println!("Error: {}", e),
                    }
                    sleep(Duration::from_millis(100)).await;
                }

                // Summary for this round
                let successful = results.iter().filter(|r| matches!(r, Ok(true))).count();
                let failed = results.iter().filter(|r| matches!(r, Ok(false))).count();
                let errors = results.iter().filter(|r| r.is_err()).count();

                println!(
                    "  Summary: {} successful, {} failed, {} errors",
                    successful, failed, errors
                );

                sleep(Duration::from_secs(1)).await;
            }
        }
        Err(e) => {
            println!("✗ Failed to connect via TCP: {}", e);
        }
    }

    // Demonstrate concurrent health checks with multiple cameras
    println!("\n=== Concurrent Health Check Example ===");
    println!("(This would check multiple cameras if addresses were provided)");

    // Example of how to check multiple cameras concurrently
    let camera_addresses = vec![camera_addr]; // In real usage, this would have multiple addresses

    let mut clients = Vec::new();
    for addr in &camera_addresses {
        match Client::connect_udp_async(addr).await {
            Ok(client) => clients.push((addr.to_string(), client)),
            Err(e) => println!("Failed to connect to {}: {}", addr, e),
        }
    }

    if !clients.is_empty() {
        println!("\nChecking {} camera(s) concurrently...", clients.len());

        // Create health check futures
        let health_futures: Vec<_> = clients
            .iter()
            .map(|(addr, client)| async move { (addr.clone(), client.is_healthy().await) })
            .collect();

        // Execute all health checks concurrently
        let results = futures_util::future::join_all(health_futures).await;

        // Display results
        for (addr, result) in results {
            print!("Camera {}: ", addr);
            match result {
                Ok(true) => println!("✓ Healthy"),
                Ok(false) => println!("✗ Not responding"),
                Err(e) => println!("✗ Error: {}", e),
            }
        }
    }

    println!("\nHealth check demo completed!");
    Ok(())
}
