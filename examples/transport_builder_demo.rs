//! Demonstrates the transport builder pattern for flexible configuration.
//!
//! This example shows how to create transports with custom configurations
//! using the builder pattern.

use grafton_visca::transport::builder::TransportBuilder;
use grafton_visca::transport::RetryConfig;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Simple TCP transport with defaults
    println!("Creating TCP transport with defaults...");
    let _simple_tcp = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .build()?;
    println!("✓ Simple TCP transport created");

    // Example 2: TCP transport with custom timeouts
    println!("\nCreating TCP transport with custom timeouts...");
    let _timeout_tcp = TransportBuilder::tcp()
        .address("camera.local:5678")
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(2))
        .write_timeout(Duration::from_secs(2))
        .build()?;
    println!("✓ TCP transport with custom timeouts created");

    // Example 3: UDP transport with retry configuration
    println!("\nCreating UDP transport with retry configuration...");
    let _retry_udp = TransportBuilder::udp()
        .address("192.168.0.110:5678")
        .max_retries(5)
        .retry_delay(Duration::from_millis(500))
        .max_retry_duration(Duration::from_secs(30))
        .exponential_backoff(true)
        .build()?;
    println!("✓ UDP transport with retry configuration created");

    // Example 4: TCP transport with all options
    println!("\nCreating TCP transport with all options...");
    let _full_tcp = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .timeout(Duration::from_secs(3)) // Set all timeouts at once
        .tcp_nodelay(true) // Disable Nagle's algorithm
        .ttl(64) // Set Time To Live
        .max_retries(10)
        .build()?;
    println!("✓ Fully configured TCP transport created");

    // Example 5: Using a custom RetryConfig
    println!("\nCreating transport with custom RetryConfig...");
    let retry_config = RetryConfig {
        max_retries: 3,
        base_retry_delay: Duration::from_millis(100),
        max_retry_duration: Duration::from_secs(10),
        exponential_backoff: false,
    };

    let _custom_retry = TransportBuilder::udp()
        .address("192.168.0.110:5678")
        .retry_config(retry_config)
        .build()?;
    println!("✓ Transport with custom RetryConfig created");

    // Example 6: Demonstrating the fluent API
    println!("\nDemonstrating fluent API...");
    let _fluent = TransportBuilder::tcp()
        .address("192.168.0.110:5678")
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(2))
        .write_timeout(Duration::from_secs(2))
        .tcp_nodelay(true)
        .ttl(64)
        .max_retries(5)
        .retry_delay(Duration::from_millis(200))
        .exponential_backoff(true)
        .build()?;
    println!("✓ Transport created with fluent API");

    println!("\n✅ All transport builder examples completed successfully!");

    Ok(())
}
