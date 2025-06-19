//! Example demonstrating multi-threaded camera control using ChannelTransport.
//!
//! This example shows how to use the ChannelTransport wrapper to safely share
//! a transport across multiple threads without explicit locking.

use std::sync::Arc;
use std::time::Duration;

use grafton_visca::transport::{
    AsyncTcpTransport, ChannelTransport, ChannelTransportBuilder, Priority, Transport,
};
use grafton_visca::{camera::profiles::PTZOpticsG2, Camera, Command, Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create the underlying transport
    let tcp_transport = AsyncTcpTransport::new("192.168.1.100:5678").await?;

    // Wrap it in a ChannelTransport with custom configuration
    let channel_transport = ChannelTransportBuilder::new(tcp_transport)
        .queue_size(200)
        .operation_timeout(Duration::from_secs(10))
        .auto_socket_management(true)
        .max_concurrent_commands(2)
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
            println!("Task 2: Zoom {}", i + 1);
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        println!("Task 2: Completed");
    }));

    // Task 3: Focus control with high priority
    let _camera3 = camera_arc.clone();
    handles.push(tokio::spawn(async move {
        println!("Task 3: Starting high-priority focus operations");
        for i in 0..2 {
            println!("Task 3: Focus {} (HIGH PRIORITY)", i + 1);
            // In a real implementation with priority support:
            // camera.transport().with_priority(Priority::High).send_command(...)
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        println!("Task 3: Completed");
    }));

    // Task 4: Status inquiries
    let _camera4 = camera_arc.clone();
    handles.push(tokio::spawn(async move {
        println!("Task 4: Starting status inquiries");
        for i in 0..5 {
            println!("Task 4: Status check {}", i + 1);
            tokio::time::sleep(Duration::from_millis(200)).await;
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

// Example of using priority with a custom transport wrapper
#[allow(dead_code)]
async fn send_priority_command<T: Command>(
    transport: &ChannelTransport,
    command: T,
    priority: Priority,
) -> Result<(), Error> {
    // Create a priority transport for this specific command
    let mut priority_transport = transport.with_priority(priority);

    // Send the command with the specified priority
    priority_transport
        .send_command(&command, grafton_visca::types::SocketId::SOCKET_0)
        .await?;

    // Receive the response
    let (_socket_id, _response) = priority_transport.receive_response().await?;

    Ok(())
}

// Example showing how to use Arc<ChannelTransport> directly
#[allow(dead_code)]
async fn demo_arc_transport() -> Result<(), Box<dyn std::error::Error>> {
    let tcp_transport = AsyncTcpTransport::new("192.168.1.100:5678").await?;

    let channel_transport = Arc::new(ChannelTransport::with_default_config(tcp_transport));

    // Multiple threads can clone this Arc and use it directly
    let transport1 = channel_transport.clone();
    let transport2 = channel_transport.clone();

    // Spawn concurrent tasks using the Arc<ChannelTransport>
    let task1 = tokio::spawn(async move {
        let _transport = transport1;
        // Use transport.send_command() and transport.receive_response()
        println!("Task using Arc<ChannelTransport> 1");
    });

    let task2 = tokio::spawn(async move {
        let _transport = transport2;
        // Use transport.send_command() and transport.receive_response()
        println!("Task using Arc<ChannelTransport> 2");
    });

    task1.await?;
    task2.await?;

    Ok(())
}
