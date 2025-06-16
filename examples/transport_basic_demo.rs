//! Example program

//! Basic transport demonstration example.
//!
//! This example shows how to:
//! - Use the unified Client with different transports
//! - Switch between UDP and TCP transports
//! - Send commands using different transport types
//! - Handle transport-specific errors
//!
//! NOTE: This example still uses the old Client API because:
//! 1. It demonstrates transport-specific features that are abstracted away in Camera<P>
//! 2. It uses inquiry commands which Camera<P> doesn't support yet
//!    For examples using Camera<P> with different transports, see basic_camera_demo.rs

use grafton_visca::command::power::Power;
use grafton_visca::command::{InquiryCommand, PowerCommand, Response};
use grafton_visca::{Client, Error};

fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("=== VISCA Transport Demo ===\n");

    // Get camera address from command line or use default
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.1.100:5678".to_string());

    // Demonstrate different transports
    demo_udp_transport(&camera_addr)?;
    demo_tcp_transport(&camera_addr)?;

    #[cfg(feature = "async-client")]
    {
        // Run async examples
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(demo_async_transports(&camera_addr))?;
    }

    Ok(())
}

fn demo_udp_transport(camera_addr: &str) -> Result<(), Error> {
    println!("1. UDP Transport:");
    println!("   Connecting via UDP to {}...", camera_addr);

    #[cfg(feature = "blocking-client")]
    {
        match Client::connect_udp(camera_addr) {
            Ok(client) => {
                println!("   ✓ UDP connection established");

                // Send some test commands
                test_basic_commands(&client)?;
            }
            Err(e) => {
                println!("   ✗ UDP connection failed: {}", e);
            }
        }
    }

    #[cfg(not(feature = "blocking-client"))]
    {
        println!("   ⚠️  Blocking client feature not enabled");
    }

    println!();
    Ok(())
}

fn demo_tcp_transport(camera_addr: &str) -> Result<(), Error> {
    println!("2. TCP Transport:");
    println!("   Connecting via TCP to {}...", camera_addr);

    #[cfg(feature = "blocking-client")]
    {
        match Client::connect_tcp(camera_addr) {
            Ok(client) => {
                println!("   ✓ TCP connection established");

                // Send some test commands
                test_basic_commands(&client)?;
            }
            Err(e) => {
                println!("   ✗ TCP connection failed: {}", e);
            }
        }
    }

    #[cfg(not(feature = "blocking-client"))]
    {
        println!("   ⚠️  Blocking client feature not enabled");
    }

    println!();
    Ok(())
}

#[cfg(feature = "blocking-client")]
fn test_basic_commands(client: &Client) -> Result<(), Error> {
    // Test inquiry
    println!("\n   Testing basic commands:");

    match client.send(&InquiryCommand::Power) {
        Ok(Response::InquiryResponse(resp)) => {
            println!("   ✓ Power inquiry: {:?}", resp);
        }
        Ok(_) => println!("   ✓ Power inquiry sent but unexpected response"),
        Err(e) => println!("   ✗ Power inquiry failed: {}", e),
    }

    // Test action command
    match client.send(&PowerCommand { power: Power::On }) {
        Ok(_) => println!("   ✓ Power on command sent"),
        Err(e) => println!("   ✗ Power on command failed: {}", e),
    }

    // Test another inquiry
    match client.send(&InquiryCommand::ZoomPosition) {
        Ok(Response::InquiryResponse(resp)) => {
            println!("   ✓ Zoom inquiry: {:?}", resp);
        }
        Ok(_) => println!("   ✓ Zoom inquiry sent but unexpected response"),
        Err(e) => println!("   ✗ Zoom inquiry failed: {}", e),
    }

    Ok(())
}

#[cfg(feature = "async-client")]
async fn demo_async_transports(camera_addr: &str) -> Result<(), Error> {
    println!("3. Async Transports:");

    // Test async UDP
    println!("\n   Testing async UDP transport:");
    match Client::connect_udp_async(camera_addr).await {
        Ok(client) => {
            println!("   ✓ Async UDP connection established");
            test_async_commands(&client).await?;
        }
        Err(e) => {
            println!("   ✗ Async UDP connection failed: {}", e);
        }
    }

    // Test async TCP
    println!("\n   Testing async TCP transport:");
    match Client::connect_tcp_async(camera_addr).await {
        Ok(client) => {
            println!("   ✓ Async TCP connection established");
            test_async_commands(&client).await?;
        }
        Err(e) => {
            println!("   ✗ Async TCP connection failed: {}", e);
        }
    }

    println!();
    Ok(())
}

#[cfg(feature = "async-client")]
async fn test_async_commands(client: &Client) -> Result<(), Error> {
    // Test async inquiry
    match client.send_async(&InquiryCommand::Power).await {
        Ok(Response::InquiryResponse(resp)) => {
            println!("   ✓ Async power inquiry: {:?}", resp);
        }
        Ok(_) => println!("   ✓ Async power inquiry sent but unexpected response"),
        Err(e) => println!("   ✗ Async power inquiry failed: {}", e),
    }

    // Test async action command
    match client.send_async(&PowerCommand { power: Power::On }).await {
        Ok(_) => println!("   ✓ Async power on command sent"),
        Err(e) => println!("   ✗ Async power on command failed: {}", e),
    }

    Ok(())
}
