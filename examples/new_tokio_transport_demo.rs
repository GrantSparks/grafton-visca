//! Demo of the new simplified tokio transport API.
//!
//! This example demonstrates how to use the new clean tokio transports
//! that work directly with AsyncTransport without adapters.

use bytes::Bytes;
use grafton_visca::{
    camera::methods::PanTiltOps,
    prelude::r#async::PTZOpticsG2Cam,
    transport::{core::Transport, tokio::Tcp},
    types::SpeedLevel,
    units::Degrees,
    Error,
};
use std::future::{ready, Ready};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Example: Using the simplified tokio transport API
    println!("=== Simplified Tokio Transport API ===");

    // Connect using the Tcp transport
    let visca = Tcp::connect("192.168.1.100:1259").await?;
    println!("Connected via TCP");

    // Create camera using the transport
    let camera = PTZOpticsG2Cam::new(visca);

    // Stop any ongoing movement
    println!("Stopping camera movement...");
    camera.pan_tilt_stop().await?;

    // Move to center position
    println!("Moving to center position...");
    camera
        .pan_tilt_absolute(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Medium)
        .await?;

    // Move camera to home position
    println!("Moving to home position...");
    camera.pan_tilt_home().await?;

    // Demo using custom transport implementation
    println!("\n=== Custom Transport Demo ===");
    let custom_transport = CustomTransport::new("Demo transport".to_string());
    let custom_camera = PTZOpticsG2Cam::new(custom_transport);

    println!("Testing custom transport...");
    // This will use the custom transport's send/receive methods
    custom_camera.pan_tilt_stop().await?;

    println!("Demo completed successfully!");

    Ok(())
}

// Example of how you could implement a custom transport using AsyncTransport
#[derive(Debug, Clone)]
struct CustomTransport {
    description: String,
}

impl CustomTransport {
    pub fn new(description: String) -> Self {
        Self { description }
    }
}

impl Transport for CustomTransport {
    type Error = Error;
    type SendFut<'a> = Ready<Result<(), Error>>;
    type RecvFut<'a> = Ready<Result<Bytes, Error>>;

    fn send<'a>(&'a self, data: &'a [u8]) -> Self::SendFut<'a> {
        println!(
            "{}: Custom transport sending {} bytes: {:02X?}",
            self.description,
            data.len(),
            data
        );
        // Simulate sending
        ready(Ok(()))
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        // Simulate receiving an ACK response
        ready(Ok(Bytes::from_static(&[0x90, 0x41, 0xFF]))) // Simple ACK for socket 1
    }
}

#[async_trait::async_trait]
impl grafton_visca::transport::UnifiedTransport for CustomTransport {
    async fn send(&self, bytes: &[u8]) -> Result<(), Error> {
        println!(
            "{}: Custom transport sending {} bytes: {:02X?}",
            self.description,
            bytes.len(),
            bytes
        );
        Ok(())
    }

    async fn recv(&self) -> Result<Bytes, Error> {
        // Simulate receiving an ACK response
        Ok(Bytes::from_static(&[0x90, 0x41, 0xFF])) // Simple ACK for socket 1
    }

    fn send_blocking(&self, bytes: &[u8]) -> Result<(), Error> {
        println!(
            "{}: Custom transport sending {} bytes: {:02X?}",
            self.description,
            bytes.len(),
            bytes
        );
        Ok(())
    }

    fn recv_blocking_timeout(&self, _timeout: std::time::Duration) -> Result<Bytes, Error> {
        // Simulate receiving an ACK response
        Ok(Bytes::from_static(&[0x90, 0x41, 0xFF])) // Simple ACK for socket 1
    }
}
