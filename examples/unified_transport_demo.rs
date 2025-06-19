//! Demonstrates the unified transport API that works for both async and blocking contexts.

mod common;
use grafton_visca::command::{power::Power, PowerCommand};
use grafton_visca::Error;

#[cfg(feature = "blocking-client")]
fn blocking_example() -> Result<(), Error> {
    use common::blocking;

    println!("=== Blocking Transport Example ===");

    // Create a blocking TCP transport with VISCA protocol handling
    let mut transport = blocking::tcp_transport("192.168.1.100:5678")?;

    // Use the transport directly - no adapter needed!
    let power_cmd = PowerCommand { power: Power::On };

    // This blocks and returns immediately with the response
    let response = transport.send_command(&power_cmd)?;
    println!("Command response: {:?}", response);

    println!("Power on command sent successfully!");

    Ok(())
}

#[cfg(feature = "async-client")]
async fn async_example() -> Result<(), Error> {
    use common::r#async;

    println!("=== Async Transport Example ===");

    // Create an async TCP transport with VISCA protocol handling
    let mut transport = r#async::tcp_transport("192.168.1.100:5678").await?;

    // Use the transport interface - similar API to blocking!
    let power_cmd = PowerCommand { power: Power::On };

    // This returns a future that we await
    let response = transport.send_command(&power_cmd).await?;
    println!("Command response: {:?}", response);

    println!("Power on command sent successfully!");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Unified Transport Demo");
    println!("This example shows how to use transports in both async and blocking contexts.\n");

    // Run blocking example if feature is enabled
    #[cfg(feature = "blocking-client")]
    {
        if let Err(e) = blocking_example() {
            eprintln!("Blocking example error: {}", e);
        }
        println!();
    }

    // Run async example if feature is enabled
    #[cfg(feature = "async-client")]
    {
        if let Err(e) = async_example().await {
            eprintln!("Async example error: {}", e);
        }
    }

    #[cfg(not(any(feature = "blocking-client", feature = "async-client")))]
    {
        println!(
            "Please enable either 'blocking-client' or 'async-client' feature to run this example."
        );
    }

    Ok(())
}

// The new transport API has separate blocking and async interfaces
// For transport-agnostic code, you would use either:
// 1. Blocking: transport::blocking::ViscaTransport<T> where T: Transport
// 2. Async: transport::ViscaTransport<T> where T: RawTransport
//
// This separation provides cleaner APIs for each use case without
// forcing async overhead on blocking scenarios.
