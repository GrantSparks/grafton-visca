//! Demonstrates the transport builder pattern for flexible configuration.
//!
//! This example shows how to create transport builders with custom configurations
//! without actually establishing connections.
//!
//! Run with:
//! - cargo run --example transport_builder_demo
//! - cargo run --example transport_builder_demo --features runtime-tokio

#[cfg(not(feature = "mode-async"))]
use std::time::Duration;

#[cfg(not(feature = "mode-async"))]
use grafton_visca::transport::{
    BackoffStrategy, NetTransportBuilder, RetryConfig, TcpKeepaliveConfig, Transport,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Transport Builder Pattern Demo");
    println!("==============================");
    println!("Note: This demo shows the builder API without establishing actual connections.\n");

    // Example set A: Blocking TCP/UDP builders (only when not using async feature)
    #[cfg(not(feature = "mode-async"))]
    {
        // Example 1: Simple TCP transport builder
        println!("Example 1: Simple TCP transport builder");
        let _simple_tcp = Transport::tcp().address("192.168.0.110:5678");
        println!("  Created builder for TCP at 192.168.0.110:5678");
        println!("  Would connect with: .build_blocking()\n");

        // Example 2: TCP transport builder with custom timeouts
        println!("Example 2: TCP transport builder with custom timeouts");
        let _timeout_tcp = NetTransportBuilder::tcp()
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
        let _retry_udp = Transport::udp()
            .address("192.168.0.110:5678")
            .max_retries(5)
            .retry_delay(Duration::from_millis(500))
            .max_retry_duration(Duration::from_secs(30))
            .backoff_strategy(BackoffStrategy::Exponential);
        println!("  Created builder with:");
        println!("    - Max retries: 5");
        println!("    - Retry delay: 500ms");
        println!("    - Max retry duration: 30s");
        println!("    - Backoff strategy: Exponential\n");

        // Example 4: TCP transport builder with all options
        println!("Example 4: TCP transport builder with all options");
        let _full_tcp = NetTransportBuilder::tcp()
            .address("192.168.0.110:5678")
            .timeout(Duration::from_secs(3)) // Set all timeouts at once
            .tcp_nodelay(true) // Disable Nagle's algorithm
            .ttl(64) // Set Time To Live
            .tcp_keepalive(TcpKeepaliveConfig::default())
            .max_retries(10);
        println!("  Created builder with:");
        println!("    - All timeouts: 3s");
        println!("    - TCP nodelay: enabled");
        println!("    - TTL: 64");
        println!("    - TCP keepalive: default VISCA long-lived policy");
        println!("    - Max retries: 10\n");
    }

    // Example 5: Using a custom RetryConfig (blocking-only)
    #[cfg(not(feature = "mode-async"))]
    {
        println!("Example 5: Using a custom RetryConfig");
        let retry_config = RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(100),
            max_retry_duration: Duration::from_secs(10),
            backoff_strategy: BackoffStrategy::Constant,
        };
        let _custom_retry = Transport::udp()
            .address("192.168.0.110:5678")
            .retry_config(retry_config);
        println!("  Created builder with custom RetryConfig:");
        println!("    - Max retries: 3");
        println!("    - Base retry delay: 100ms");
        println!("    - Max retry duration: 10s");
        println!("    - Backoff strategy: Constant\n");
    }

    // Example 6: Demonstrating the fluent API (blocking-only)
    #[cfg(not(feature = "mode-async"))]
    {
        println!("Example 6: Demonstrating the fluent API");
        let _fluent = Transport::tcp()
            .address("192.168.0.110:5678")
            .connect_timeout(Duration::from_secs(5))
            .read_timeout(Duration::from_secs(2))
            .write_timeout(Duration::from_secs(2))
            .tcp_nodelay(true)
            .ttl(64)
            .max_retries(5)
            .retry_delay(Duration::from_millis(200))
            .backoff_strategy(BackoffStrategy::Exponential);
        println!("  Created builder with fluent API chaining:");
        println!("    - Connect timeout: 5s");
        println!("    - Read/Write timeout: 2s");
        println!("    - TCP nodelay: enabled");
        println!("    - TTL: 64");
        println!("    - Max retries: 5");
        println!("    - Retry delay: 200ms");
        println!("    - Backoff strategy: Exponential\n");
    }

    // Example 7: Async transport API information
    #[cfg(feature = "mode-async")]
    {
        println!("Example 7: Async Transport API");
        println!("  Blocking transport builders are not available in async mode.");
        println!("  Use CameraConfig + TransportConfig for high-level setup:");
        println!();
        println!("  For Tokio:");
        println!("    use grafton_visca::camera::CameraConfig;");
        println!("    use grafton_visca::profiles::PtzOpticsG2;");
        println!("    use grafton_visca::runtime::TokioRuntime;");
        println!("    use grafton_visca::transport::TransportConfig;");
        println!("    let runtime = TokioRuntime::from_current()?;");
        println!("    let camera = CameraConfig::<PtzOpticsG2>::new()");
        println!("        .address(\"192.168.0.110\")");
        println!("        .transport_config(TransportConfig::default())");
        println!("        .open_async(runtime)");
        println!("        .await?;");
        println!();
        println!("  For advanced BYO-transport flows:");
        println!("    use grafton_visca::CameraBuilder;");
        println!("    use grafton_visca::runtime_adapters::tokio::TcpTransport;");
        println!("    let transport = TcpTransport::connect(\"192.168.0.110:5678\").await?;");
        println!("    let runtime = TokioRuntime::from_current()?;");
        println!("    let camera = CameraBuilder::with_executor(runtime)");
        println!("        .from_transport(transport)");
        println!("        .profile::<PtzOpticsG2>()");
        println!("        .open_async()");
        println!("        .await?;");
        println!();
    }

    #[cfg(not(feature = "mode-async"))]
    {
        println!("Example 7: Async transport builders");
        println!("  (Skipped - requires async mode)");
        println!(
            "  Run with: cargo run --example transport_builder_demo --features runtime-tokio\n"
        );
    }

    println!("\n✅ All transport builder examples completed successfully!");

    Ok(())
}
