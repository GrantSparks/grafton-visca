//! Demonstrates camera readiness checking and command concurrency management.
//!
//! This example shows how to use the `is_ready()` and `pending_commands()` methods
//! to check if the camera can accept new commands without blocking.

use std::time::Duration;

use grafton_visca::{
    camera::{Camera, GenericVisca},
    command::inquiry::InquiryCommand,
    transport::AsyncUdpTransport,
    Result,
};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Create camera with async UDP transport
    let transport = AsyncUdpTransport::new("192.168.1.100:52381").await?;
    let camera = Camera::<GenericVisca>::new(transport);

    println!("Camera Readiness Demo");
    println!("====================");

    // Check initial readiness
    println!("\nInitial state:");
    println!("  Is ready: {}", camera.is_ready());
    println!("  Pending commands: {}", camera.pending_commands());

    // Send two commands concurrently (filling both sockets)
    println!("\nSending two concurrent commands...");

    let cam1 = camera.clone();
    let cam2 = camera.clone();

    let handle1 = tokio::spawn(async move {
        println!("  Task 1: Querying pan/tilt position");
        let result = cam1.send_raw_async(&InquiryCommand::PanTiltPosition).await;
        println!("  Task 1: Complete - {:?}", result.is_ok());
        result
    });

    let handle2 = tokio::spawn(async move {
        println!("  Task 2: Querying zoom position");
        let result = cam2.send_raw_async(&InquiryCommand::ZoomPosition).await;
        println!("  Task 2: Complete - {:?}", result.is_ok());
        result
    });

    // Give commands time to start
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Check readiness while commands are in flight
    println!("\nWhile commands are executing:");
    println!("  Is ready: {}", camera.is_ready());
    println!("  Pending commands: {}", camera.pending_commands());

    // Try to send a third command (should wait for a socket)
    let cam3 = camera.clone();
    let handle3 = tokio::spawn(async move {
        println!("\nTask 3: Attempting to send command while sockets are full...");

        // Check readiness before sending
        if !cam3.is_ready() {
            println!("  Task 3: Camera not ready (all sockets in use), command will wait");
        }

        let start = tokio::time::Instant::now();
        let result = cam3.send_raw_async(&InquiryCommand::PanTiltPosition).await;
        let elapsed = start.elapsed();

        println!(
            "  Task 3: Complete after {:?} - {:?}",
            elapsed,
            result.is_ok()
        );
        result
    });

    // Wait for all commands to complete
    let _ = tokio::join!(handle1, handle2, handle3);

    // Check final state
    println!("\nFinal state:");
    println!("  Is ready: {}", camera.is_ready());
    println!("  Pending commands: {}", camera.pending_commands());

    println!("\nDemo complete!");
    Ok(())
}
