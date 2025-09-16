//! Advanced: Low-level runtime API demonstration.
//!
//! This example shows direct usage of RuntimeHandle and VISCA commands
//! for advanced users and library maintainers.
//!
//! For typical usage, see the high-level examples in examples/

#[cfg(not(all(feature = "async", feature = "rt-tokio")))]
fn main() {
    eprintln!("This example requires the 'async' and 'rt-tokio' features to be enabled.");
    eprintln!("Run with: cargo run --example runtime_demo_lowlevel --features async,rt-tokio");
}

#[cfg(all(feature = "async", feature = "rt-tokio"))]
use std::sync::Arc;

#[cfg(all(feature = "async", feature = "rt-tokio"))]
use grafton_visca::{
    camera::profiles::GenericVisca,
    camera_id::CameraId,
    command::{power::Power, zoom::Zoom, InquiryResponse, ViscaResponse},
    runtime::{Priority, RuntimeHandle},
    TokioExecutor,
};

#[cfg(all(feature = "async", feature = "rt-tokio"))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Camera configuration
    let camera_address = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.100".to_string());

    println!("Connecting to camera at {camera_address}...");

    // Create a runtime with raw TCP transport (PTZOptics style)
    let executor = Arc::new(TokioExecutor::from_current()?);
    let runtime = RuntimeHandle::<GenericVisca, _>::new_tcp_raw(&camera_address, executor).await?;

    println!("Connected! Demonstrating low-level runtime API...");

    // Power on the camera
    println!("\n1. Sending power on command...");
    let power_on = Power::On;
    let response = runtime
        .send_command(&power_on, CameraId::default(), Some(Priority::High))
        .await?;
    match response {
        ViscaResponse::Completion { .. } => println!("   ✓ Camera powered on"),
        ViscaResponse::Error(e) => println!("   ✗ Power on failed: {e}"),
        _ => println!("   ? Unexpected response: {response:?}"),
    }

    // Wait a moment for the camera to initialize
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Send zoom in command
    println!("\n2. Sending zoom in command...");
    let zoom_in = Zoom::TeleStd;
    let response = runtime
        .send_command(&zoom_in, CameraId::default(), None)
        .await?;
    match response {
        ViscaResponse::Completion { .. } => println!("   ✓ Zoom in started"),
        ViscaResponse::Error(e) => println!("   ✗ Zoom in failed: {e}"),
        _ => println!("   ? Unexpected response: {response:?}"),
    }

    // Wait for zoom to move
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Stop zoom
    println!("\n3. Sending zoom stop command...");
    let zoom_stop = Zoom::Stop;
    let response = runtime
        .send_command(&zoom_stop, CameraId::default(), None)
        .await?;
    match response {
        ViscaResponse::Completion { .. } => println!("   ✓ Zoom stopped"),
        ViscaResponse::Error(e) => println!("   ✗ Zoom stop failed: {e}"),
        _ => println!("   ? Unexpected response: {response:?}"),
    }

    // Send power inquiry
    println!("\n4. Sending power inquiry...");
    let power_inquiry = grafton_visca::command::inquiry::PowerInquiry;
    let response = runtime
        .send_inquiry(&power_inquiry, CameraId::default())
        .await?;
    match response {
        ViscaResponse::Inquiry(InquiryResponse::Power { on }) => {
            let status = if on { "ON" } else { "OFF" };
            println!("   ✓ Power status: {status}");
        }
        ViscaResponse::Error(e) => println!("   ✗ Power inquiry failed: {e}"),
        _ => println!("   ? Unexpected response: {response:?}"),
    }

    // Get runtime metrics
    println!("\n5. Getting runtime metrics...");
    let metrics = runtime.metrics().await?;
    println!("   Commands sent: {}", metrics.commands_sent);
    println!("   Commands completed: {}", metrics.commands_completed);
    println!("   Commands failed: {}", metrics.commands_failed);
    println!("   Commands retried: {}", metrics.commands_retried);

    // Demonstrate priority scheduling
    println!("\n6. Demonstrating priority scheduling...");
    println!("   Sending low priority zoom command...");
    let zoom_wide = Zoom::WideStd;
    let low_priority_future =
        runtime.send_command(&zoom_wide, CameraId::default(), Some(Priority::Low));

    println!("   Sending high priority zoom stop command...");
    let zoom_stop = Zoom::Stop;
    let high_priority_future =
        runtime.send_command(&zoom_stop, CameraId::default(), Some(Priority::High));

    // High priority should complete first even though it was sent second
    let (high_result, low_result) = tokio::join!(high_priority_future, low_priority_future);

    match high_result {
        Ok(ViscaResponse::Completion { .. }) => println!("   ✓ High priority command completed"),
        Ok(resp) => println!("   ? High priority response: {resp:?}"),
        Err(e) => println!("   ✗ High priority failed: {e}"),
    }

    match low_result {
        Ok(ViscaResponse::Completion { .. }) => println!("   ✓ Low priority command completed"),
        Ok(resp) => println!("   ? Low priority response: {resp:?}"),
        Err(e) => println!("   ✗ Low priority failed: {e}"),
    }

    // Shutdown the runtime
    println!("\n7. Shutting down runtime...");
    runtime.shutdown().await;
    println!("   ✓ Runtime shutdown complete");

    Ok(())
}
