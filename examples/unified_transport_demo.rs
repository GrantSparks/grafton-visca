//! Demonstrates the unified transport API that works for both async and blocking contexts.

#[cfg(not(feature = "async"))]
fn blocking_example() -> Result<(), grafton_visca::Error> {
    use grafton_visca::{
        camera::methods::PowerBlockingExt, profiles::GenericVisca, transport::blocking::Tcp,
        CameraBlocking,
    };

    println!("=== Blocking Transport Example ===");

    // Create a blocking TCP transport with VISCA protocol handling
    let transport = Tcp::connect("192.168.1.100:5678")?;
    let mut camera = CameraBlocking::<GenericVisca, _>::new(transport);

    // Use the high-level camera API
    camera.power_on()?;
    println!("Power on command sent successfully!");

    Ok(())
}

#[cfg(feature = "tokio")]
async fn async_example() -> Result<(), grafton_visca::Error> {
    use grafton_visca::{
        camera::methods::PowerAsyncExt, profiles::GenericVisca, transport::tokio::Tcp, Camera,
    };
    use std::time::Duration;

    println!("=== Async Transport Example ===");

    // Create an async TCP transport with VISCA protocol handling
    let transport = Tcp::connect_timeout("192.168.1.100:5678", Duration::from_secs(5)).await?;
    let camera = Camera::<GenericVisca, _>::new(transport);

    // Use the high-level camera API - same methods as blocking!
    camera.power_on().await?;
    println!("Power on command sent successfully!");

    Ok(())
}

#[cfg(all(feature = "async", not(feature = "tokio")))]
fn main() {
    eprintln!("This example requires either no features (blocking) or tokio feature (async).");
    eprintln!("Run with: cargo run --example unified_transport_demo --features tokio");
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Unified Transport Demo");
    println!("This example shows how to use transports in both async and blocking contexts.\n");

    if let Err(e) = blocking_example() {
        eprintln!("Blocking example error: {}", e);
    }
    println!();

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Unified Transport Demo");
    println!("This example shows how to use transports in both async and blocking contexts.\n");

    if let Err(e) = async_example().await {
        eprintln!("Async example error: {}", e);
    }

    Ok(())
}

// The new transport API has separate blocking and async interfaces
// For transport-agnostic code, you would use either:
// 1. Blocking: transport::blocking::ViscaProtocol<T> where T: BlockingTransport
// 2. Async: transport::ViscaProtocol<T> where T: AsyncTransport
//
// This separation provides cleaner APIs for each use case without
// forcing async overhead on blocking scenarios.
