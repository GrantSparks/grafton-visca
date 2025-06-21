//! Demo of the new simplified tokio transport API.
//!
//! This example demonstrates how to use the new clean tokio transports
//! that work directly with AsyncTransport without adapters.

use grafton_visca::{
    profiles::PTZOpticsG2,
    transport::{create, AsyncTransport},
    Camera, Error,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Example: Using the simplified tokio transport API
    println!("=== Simplified Tokio Transport API ===");

    // Connect using the create helper functions
    let visca = create::tcp("192.168.1.100:1259").await?;
    println!("Connected via TCP");

    // Create camera using the transport
    let camera = Camera::<PTZOpticsG2, _>::new(visca);

    // Stop any ongoing movement
    println!("Stopping camera movement...");
    camera.stop().await?;

    // Get current position
    println!("Getting current pan/tilt position...");
    let (pan, tilt) = camera.get_position().await?;
    println!("Current position: pan={:?}, tilt={:?}", pan, tilt);

    // Move camera to home position
    println!("Moving to home position...");
    camera.home().await?;

    // Demo using custom transport implementation
    println!("\n=== Custom Transport Demo ===");
    let custom_transport = CustomTransport::new("Demo transport".to_string());
    let custom_camera = Camera::<PTZOpticsG2, _>::new(custom_transport);
    
    println!("Testing custom transport...");
    // This will use the custom transport's send/receive methods
    custom_camera.stop().await?;
    
    println!("Demo completed successfully!");

    Ok(())
}

// Example of how you could implement a custom transport using AsyncTransport
#[derive(Debug)]
struct CustomTransport {
    description: String,
}

impl CustomTransport {
    pub fn new(description: String) -> Self {
        Self { description }
    }
}

impl AsyncTransport for CustomTransport {
    type SendFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Error>> + Send + 'a>>;
    type ReceiveFuture<'a> =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFuture<'a> {
        Box::pin(async move {
            println!(
                "{}: Custom transport sending {} bytes: {:02X?}",
                self.description,
                data.len(),
                data
            );
            // Simulate sending
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Ok(())
        })
    }

    fn receive(&self) -> Self::ReceiveFuture<'_> {
        Box::pin(async move {
            // Simulate receiving an ACK response
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(vec![0x90, 0x41, 0xFF]) // Simple ACK for socket 1
        })
    }
}
