//! Demonstrates the transport builder pattern for flexible configuration.
//!
//! This example shows how to create transport builders with custom configurations
//! without actually establishing connections.
//!
//! Run with: cargo run --example transport_builder_demo --features rt-tokio

#[cfg(not(feature = "async"))]
use grafton_visca::transport::builder::TransportBuilder;
#[cfg(not(feature = "async"))]
use grafton_visca::transport::RetryConfig;
#[cfg(any(not(feature = "async"), feature = "rt-tokio"))]
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Transport Builder Pattern Demo");
    println!("==============================");
    println!("Note: This demo shows the builder API without establishing actual connections.\n");

    // Example set A: Blocking TCP/UDP builders (only when not using async feature)
    #[cfg(not(feature = "async"))]
    {
        // Example 1: Simple TCP transport builder
        println!("Example 1: Simple TCP transport builder");
        let _simple_tcp = TransportBuilder::tcp().address("192.168.0.110:5678");
        println!("  Created builder for TCP at 192.168.0.110:5678");
        println!("  Would connect with: .build()\n");

        // Example 2: TCP transport builder with custom timeouts
        println!("Example 2: TCP transport builder with custom timeouts");
        let _timeout_tcp = TransportBuilder::tcp()
            .address("camera.local:5678")
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(2))
            .write_timeout(Duration::from_secs(2));
        println!("  Created builder with:");
        println!("    - Connect timeout: 10s");
        println!("    - Read timeout: 2s");
        println!("    - Write timeout: 2s\n");

        // Example 3: UDP transport builder with retry configuration
        println!("Example 3: UDP transport builder with retry configuration");
        let _retry_udp = TransportBuilder::udp()
            .address("192.168.0.110:5678")
            .max_retries(5)
            .retry_delay(Duration::from_millis(500))
            .max_retry_duration(Duration::from_secs(30))
            .exponential_backoff(true);
        println!("  Created builder with:");
        println!("    - Max retries: 5");
        println!("    - Retry delay: 500ms");
        println!("    - Max retry duration: 30s");
        println!("    - Exponential backoff: enabled\n");

        // Example 4: TCP transport builder with all options
        println!("Example 4: TCP transport builder with all options");
        let _full_tcp = TransportBuilder::tcp()
            .address("192.168.0.110:5678")
            .timeout(Duration::from_secs(3)) // Set all timeouts at once
            .tcp_nodelay(true) // Disable Nagle's algorithm
            .ttl(64) // Set Time To Live
            .max_retries(10);
        println!("  Created builder with:");
        println!("    - All timeouts: 3s");
        println!("    - TCP nodelay: enabled");
        println!("    - TTL: 64");
        println!("    - Max retries: 10\n");
    }

    // Example 5: Using a custom RetryConfig (blocking-only)
    #[cfg(not(feature = "async"))]
    {
        println!("Example 5: Using a custom RetryConfig");
        let retry_config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            exponential_backoff: false,
        };
        let _custom_retry = TransportBuilder::udp()
            .address("192.168.0.110:5678")
            .retry_config(retry_config);
        println!("  Created builder with custom RetryConfig:");
        println!("    - Max retries: 3");
        println!("    - Base retry delay: 100ms");
        println!("    - Max retry duration: 10s");
        println!("    - Exponential backoff: disabled\n");
    }

    // Example 6: Demonstrating the fluent API (blocking-only)
    #[cfg(not(feature = "async"))]
    {
        println!("Example 6: Demonstrating the fluent API");
        let _fluent = TransportBuilder::tcp()
            .address("192.168.0.110:5678")
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(2))
            .write_timeout(Duration::from_secs(2))
            .tcp_nodelay(true)
            .ttl(64)
            .max_retries(5)
            .retry_delay(Duration::from_millis(200))
            .exponential_backoff(true);
        println!("  Created builder with fluent API chaining:");
        println!("    - Connect timeout: 5s");
        println!("    - Read/Write timeout: 2s");
        println!("    - TCP nodelay: enabled");
        println!("    - TTL: 64");
        println!("    - Max retries: 5");
        println!("    - Retry delay: 200ms");
        println!("    - Exponential backoff: enabled\n");
    }

    // Example 7: Uniform async transport API (requires rt-tokio feature)
    #[cfg(feature = "rt-tokio")]
    {
        use grafton_visca::transport::Transport;
        println!("Example 7: Uniform async transport API (rt-tokio feature)");

        // Example 7a: Uniform TCP transport builder
        println!("  7a. Uniform TCP transport:");
        let _async_tcp = Transport::tcp()
            .address("192.168.0.110:5678")
            .connect_timeout(Duration::from_secs(10))
            .tcp_nodelay(true);
        println!("     Created uniform TCP builder");
        println!("     Would connect with: .connect().await (runtime auto-selected)");

        // Example 7b: Uniform UDP transport builder
        println!("\n  7b. Uniform UDP transport:");
        let _async_udp = Transport::udp()
            .address("192.168.0.110:5678")
            .ttl(64)
            .max_retries(5);
        println!("     Created uniform UDP builder");
        println!("     Would connect with: .connect().await (runtime auto-selected)");

        // Example 7c: Runtime auto-selection
        println!("\n  7c. Runtime auto-selection:");
        println!("     Transport::tcp().connect().await automatically selects:");
        println!("     - tokio when rt-tokio feature is enabled");
        println!("     - async-std when rt-async-std feature is enabled");
        println!("     - smol when rt-smol feature is enabled");
        println!("     No need to mention runtime in application code!");
        println!();
    }

    #[cfg(not(feature = "rt-tokio"))]
    {
        println!("Example 7: Async transport builders");
        println!("  (Skipped - requires 'rt-tokio' feature)");
        println!("  Run with: cargo run --example transport_builder_demo --features rt-tokio\n");
    }

    println!("\n✅ All transport builder examples completed successfully!");

    Ok(())
}
