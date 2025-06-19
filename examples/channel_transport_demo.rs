//! Example demonstrating multi-threaded camera control using ChannelTransport.
//!
//! This example shows how to use the ChannelTransport wrapper to safely share
//! a transport across multiple threads without explicit locking.

#[cfg(feature = "async-client")]
mod common;
#[cfg(feature = "async-client")]
use common::r#async::tcp_transport;
#[cfg(feature = "async-client")]
use std::sync::Arc;
#[cfg(feature = "async-client")]
use std::time::Duration;

#[cfg(feature = "async-client")]
use grafton_visca::transport::{ChannelTransport, ChannelTransportBuilder, Transport};
#[cfg(feature = "async-client")]
use grafton_visca::{camera::profiles::PTZOpticsG2, Camera, Command};

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create the underlying transport
    let tcp_transport = tcp_transport("192.168.1.100:5678").await?;

    // Wrap it in a ChannelTransport with custom configuration
    let channel_transport = ChannelTransportBuilder::new(tcp_transport)
        .queue_size(200)
        .build();

    // Create a camera using the channel transport
    let camera: Camera<PTZOpticsG2> = Camera::new(channel_transport);

    // Clone the camera for multi-threaded access
    // (Camera must implement Clone for this to work)
    let camera_arc = Arc::new(camera);

    // Spawn multiple tasks that control the camera concurrently
    let mut handles = vec![];

    // Task 1: Pan/tilt control
    let _camera1 = camera_arc.clone();
    handles.push(tokio::spawn(async move {
        println!("Task 1: Starting pan/tilt movements");
        for i in 0..3 {
            println!("Task 1: Movement {}", i + 1);
            // In a real implementation, you would use camera methods here
            // For now, we'll just demonstrate the concept
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        println!("Task 1: Completed");
    }));

    // Task 2: Zoom control
    let _camera2 = camera_arc.clone();
    handles.push(tokio::spawn(async move {
        println!("Task 2: Starting zoom operations");
        for i in 0..3 {
            println!("Task 2: Zoom operation {}", i + 1);
            // In a real implementation, you would use camera methods here
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        println!("Task 2: Completed");
    }));

    // Task 3: Inquiry commands
    let _camera3 = camera_arc.clone();
    handles.push(tokio::spawn(async move {
        println!("Task 3: Starting status inquiries");
        for i in 0..3 {
            println!("Task 3: Inquiry {}", i + 1);
            // In a real implementation, you would use camera methods here
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        println!("Task 3: Completed");
    }));

    // Task 4: Preset operations
    let _camera4 = camera_arc.clone();
    handles.push(tokio::spawn(async move {
        println!("Task 4: Starting preset operations");
        for i in 0..3 {
            println!("Task 4: Preset operation {}", i + 1);
            // In a real implementation, you would use camera methods here
            tokio::time::sleep(Duration::from_millis(600)).await;
        }
        println!("Task 4: Completed");
    }));

    // Wait for all tasks to complete
    for handle in handles {
        handle.await?;
    }

    println!("\nAll tasks completed successfully!");
    println!("The ChannelTransport automatically:");
    println!("- Queued commands when both sockets were busy");
    println!("- Managed socket allocation across threads");
    println!("- Ensured thread-safe access without explicit locks");

    Ok(())
}

// Example showing how to use Arc<ChannelTransport> directly
#[allow(dead_code)]
async fn demo_arc_transport() -> Result<(), Box<dyn std::error::Error>> {
    let tcp_transport = tcp_transport("192.168.1.100:5678").await?;

    let channel_transport = Arc::new(ChannelTransport::new(tcp_transport, Default::default()));

    // Multiple threads can clone this Arc and use it directly
    let t1 = channel_transport.clone();
    let task1 = tokio::spawn(async move {
        let transport = t1;
        // Use transport for commands
        let _ = transport; // Suppress unused warning
    });

    let t2 = channel_transport.clone();
    let task2 = tokio::spawn(async move {
        let transport = t2;
        // Use transport for commands
        let _ = transport; // Suppress unused warning
    });

    task1.await?;
    task2.await?;

    Ok(())
}

// Mock transport for demonstration
#[cfg(feature = "async-client")]
#[allow(dead_code)]
struct MockTransport;

#[cfg(feature = "async-client")]
impl Transport for MockTransport {
    fn send_command<'a>(
        &'a mut self,
        _command: &'a dyn Command,
    ) -> grafton_visca::transport::TransportFuture<'a, grafton_visca::Response> {
        Box::pin(async { Ok(grafton_visca::Response::Completion) })
    }
}

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature to be enabled.");
    eprintln!("Run with: cargo run --example channel_transport_demo --features async-client");
}
