//! Example of using grafton-visca with the smol runtime.
//!
//! This example demonstrates how to use the library with smol for async operations.
//!
//! # Usage
//! ```bash
//! cargo run --example quickstart_async_smol --features rt-smol
//! ```

#[cfg(not(feature = "rt-smol"))]
fn main() {
    eprintln!("This example requires the 'rt-smol' feature.");
    eprintln!("Run with: cargo run --example quickstart_async_smol --features rt-smol");
}

#[cfg(feature = "rt-smol")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    smol::block_on(async_main())
}

#[cfg(feature = "rt-smol")]
async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::runtime_adapters::smol::TcpTransport;
    use grafton_visca::transport::AsyncTransport;

    // Connect to camera
    println!("Connecting to camera at 192.168.0.110:5678...");
    let mut transport = TcpTransport::connect("192.168.0.110:5678").await?;

    // Send a simple power on command
    let power_on = vec![0x81, 0x01, 0x04, 0x00, 0x02, 0xFF];
    println!("Sending power on command: {:02X?}", power_on);
    transport.send(&power_on).await?;

    // Receive response
    let response = transport.recv().await?;
    println!("Received response: {:02X?}", response.as_ref());

    // Send a power inquiry command
    let power_inquiry = vec![0x81, 0x09, 0x04, 0x00, 0xFF];
    println!("Sending power inquiry: {:02X?}", power_inquiry);
    transport.send(&power_inquiry).await?;

    // Receive response
    let response = transport.recv().await?;
    println!("Received response: {:02X?}", response.as_ref());

    println!("smol example completed successfully!");
    Ok(())
}
