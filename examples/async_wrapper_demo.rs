//! Example demonstrating the async wrapper for blocking transports.
//!
//! This example shows how to:
//! - Use native async transports with TransportBuilder
//! - Convert blocking transports to async using the AsyncWrapper
//! - Compare native async vs wrapped blocking performance
//! - Share transports across async tasks
//!
//! **Context**: This example requires tokio runtime and demonstrates BOTH:
//! - Native async transports via .build_async() (preferred for async code)
//! - Wrapped blocking transports via .build_async_wrapper() (for migration/compatibility)
//!
//! Run with: `cargo run --example async_wrapper_demo --features rt-tokio`

#[cfg(not(feature = "rt-tokio"))]
fn main() {
    eprintln!("This example requires the 'rt-tokio' feature.");
    eprintln!("Run with: cargo run --example async_wrapper_demo --features rt-tokio");
}

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::transport::{builder::TransportBuilder, AsyncTransport, AsyncWrapperExt};

    use std::{env, time::Duration};

    env_logger::init();

    let address = env::var("CAMERA_ADDRESS").unwrap_or_else(|_| "192.168.0.110:5678".to_string());

    println!("Async Wrapper Demo");
    println!("==================");
    println!("Connecting to camera at: {address}");
    println!();

    // Method 1: Native async transport with TransportBuilder
    println!("Method 1: Native async transport with TransportBuilder");
    println!("-------------------------------------------------------");
    {
        match TransportBuilder::tokio_tcp()
            .address(&address)
            .connect_timeout(Duration::from_secs(5))
            .build_async()
            .await
        {
            Ok(async_transport) => {
                println!("✓ Created native async TCP transport via builder");
                println!("  (No wrapping needed - this is a true async transport)");

                demonstrate_native_async_transport(async_transport, "Native async").await?;
            }
            Err(e) => {
                println!("✗ Failed to connect: {e}");
                println!("  (This is expected if no camera is connected)");
            }
        }
    }

    println!();

    // Method 2: Native async UDP transport
    println!("Method 2: Native async UDP transport");
    println!("-------------------------------------");
    {
        match TransportBuilder::tokio_udp()
            .address(&address)
            .build_async()
            .await
        {
            Ok(async_transport) => {
                println!("✓ Created native async UDP transport via builder");
                println!("  (This is a true async UDP transport)");

                demonstrate_native_async_transport(async_transport, "Native UDP").await?;
            }
            Err(e) => {
                println!("✗ Failed to connect: {e}");
                println!("  (This is expected if no camera is connected)");
            }
        }
    }

    println!();

    // Method 3: Using the builder with async wrapper
    println!("Method 3: Using the builder pattern");
    println!("------------------------------------");
    {
        match TransportBuilder::tcp()
            .address(&address)
            .connect_timeout(Duration::from_secs(10))
            .read_timeout(Duration::from_secs(2))
            .tcp_nodelay(true)
            .max_retries(3)
            .build_async_wrapper()
        {
            Ok(async_transport) => {
                println!("✓ Built async-wrapped TCP transport with custom config");
                println!("  - Connect timeout: 10s");
                println!("  - Read timeout: 2s");
                println!("  - TCP nodelay: enabled");
                println!("  - Max retries: 3");

                demonstrate_boxed_async_transport(async_transport, "Builder pattern").await?;
            }
            Err(e) => {
                println!("✗ Failed to build transport: {e}");
                println!("  (This is expected if no camera is connected)");
            }
        }
    }

    println!();

    // Method 4: Sharing wrapped transports across tasks
    println!("Method 4: Sharing across async tasks");
    println!("-------------------------------------");
    {
        use std::sync::Arc;

        match TransportBuilder::tcp()
            .address(&address)
            .max_retries(5)
            .build()
        {
            Ok(blocking_transport) => {
                let shared_transport = Arc::new(blocking_transport);
                let async_transport = shared_transport.as_async();
                println!("✓ Created shared async-wrapped transport via builder");

                let transport1 = async_transport.clone();
                let transport2 = async_transport.clone();

                let task1 = tokio::spawn(async move {
                    println!("  Task 1: Sending power inquiry...");
                    if let Err(e) = transport1.send(b"\x81\x09\x04\x00\xFF").await {
                        println!("  Task 1: Send failed: {e}");
                    } else {
                        println!("  Task 1: ✓ Sent successfully");
                    }
                });

                let task2 = tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    println!("  Task 2: Sending zoom inquiry...");
                    if let Err(e) = transport2.send(b"\x81\x09\x04\x47\xFF").await {
                        println!("  Task 2: Send failed: {e}");
                    } else {
                        println!("  Task 2: ✓ Sent successfully");
                    }
                });

                let _ = tokio::try_join!(task1, task2);
                println!("✓ Both tasks completed");
            }
            Err(e) => {
                println!("✗ Failed to connect: {e}");
                println!("  (This is expected if no camera is connected)");
            }
        }
    }

    println!();
    println!("Demo completed!");
    println!();
    println!("Key takeaways:");
    println!("- Blocking transports can be easily used in async contexts");
    println!("- The wrapper handles thread pool execution transparently");
    println!("- Multiple wrapper creation methods for different use cases");
    println!("- Wrapped transports can be shared across async tasks");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
async fn demonstrate_native_async_transport(
    transport: impl grafton_visca::transport::AsyncTransport,
    method_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::time::Instant;

    println!("  Testing native async operations with {method_name}...");

    let start = Instant::now();
    match transport.send(b"\x81\x09\x04\x00\xFF").await {
        Ok(_) => {
            println!("  ✓ Sent power inquiry in {:?}", start.elapsed());
        }
        Err(e) => {
            println!("  ✗ Send failed: {e}");
            return Ok(());
        }
    }

    match tokio::time::timeout(std::time::Duration::from_secs(1), transport.recv()).await {
        Ok(Ok(response)) => {
            println!("  ✓ Received response: {:02X?}", response.as_ref());
        }
        Ok(Err(e)) => {
            println!("  ✗ Receive error: {e}");
        }
        Err(_) => {
            println!("  ✗ Receive timeout (no camera connected?)");
        }
    }

    Ok(())
}

#[cfg(feature = "rt-tokio")]
async fn demonstrate_boxed_async_transport(
    transport: grafton_visca::transport::AsyncWrapper<
        Box<dyn grafton_visca::transport::BlockingTransport>,
    >,
    method_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::transport::AsyncTransport;
    use std::time::Instant;

    println!("  Testing boxed async transport with {method_name}...");

    let start = Instant::now();
    match transport.send(b"\x81\x09\x04\x47\xFF").await {
        Ok(_) => {
            println!("  ✓ Sent zoom inquiry in {:?}", start.elapsed());
        }
        Err(e) => {
            println!("  ✗ Send failed: {e}");
            return Ok(());
        }
    }

    match tokio::time::timeout(std::time::Duration::from_secs(1), transport.recv()).await {
        Ok(Ok(response)) => {
            println!("  ✓ Received response: {:02X?}", response.as_ref());
        }
        Ok(Err(e)) => {
            println!("  ✗ Receive error: {e}");
        }
        Err(_) => {
            println!("  ✗ Receive timeout (no camera connected?)");
        }
    }

    Ok(())
}
