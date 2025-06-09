//! Demo of the Phase B unified client implementation.
//!
//! This example shows how the new unified `Client` works in both
//! blocking and async contexts.

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use grafton_visca::command::{Power, PowerCommand, ZoomCommand};
#[cfg(any(feature = "blocking-client", feature = "async-client"))]
use grafton_visca::{Client, Error};

#[cfg(feature = "blocking-client")]
fn blocking_example() -> Result<(), Error> {
    println!("=== Blocking Example ===");

    // Create a blocking client
    let client = Client::connect_udp("192.168.1.100:5678")?;

    // Send commands using the blocking façade
    println!("Powering on camera...");
    let _response = client.send(&PowerCommand { power: Power::On })?;

    println!("Zooming in...");
    client.send(&ZoomCommand::ZoomInStandard)?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn async_example() -> Result<(), Error> {
    println!("=== Async Example ===");

    // Create an async client
    let client = Client::connect_udp_async("192.168.1.100:5678").await?;

    // Send commands using the async interface
    println!("Powering on camera...");
    client
        .send_async(&PowerCommand { power: Power::On })
        .await?;

    println!("Zooming in...");
    client.send_async(&ZoomCommand::ZoomInStandard).await?;

    // Check camera health
    let is_healthy = client.is_healthy().await?;
    println!("Camera healthy: {is_healthy}");

    Ok(())
}

#[cfg(all(feature = "blocking-client", feature = "async-client"))]
async fn mixed_example() -> Result<(), Error> {
    println!("=== Mixed Blocking/Async Example ===");

    // Create a blocking client but use it in async context
    let client = Client::connect_udp("192.168.1.100:5678")?;

    // Can use blocking API even inside async function
    println!("Using blocking API in async context...");
    client.send(&PowerCommand { power: Power::On })?;

    // Can also use async API on the same client
    println!("Using async API...");
    client.send_async(&ZoomCommand::ZoomOutStandard).await?;

    Ok(())
}

#[cfg(any(feature = "blocking-client", feature = "async-client"))]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    #[cfg(feature = "blocking-client")]
    blocking_example().unwrap_or_else(|e| eprintln!("Blocking example error: {e}"));

    #[cfg(feature = "async-client")]
    async_example()
        .await
        .unwrap_or_else(|e| eprintln!("Async example error: {e}"));

    #[cfg(all(feature = "blocking-client", feature = "async-client"))]
    mixed_example()
        .await
        .unwrap_or_else(|e| eprintln!("Mixed example error: {e}"));

    Ok(())
}

#[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
fn main() {
    println!(
        "This example requires at least one of the 'blocking-client' or 'async-client' features."
    );
    println!("Try: cargo run --example phase_b_unified_demo --features blocking-client");
}
