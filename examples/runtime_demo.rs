//! Demo of runtime capabilities using high-level Camera API.
//!
//! This example demonstrates runtime features like priority scheduling
//! and metrics using the high-level accessor-based Camera API.
//!
//! For low-level runtime usage, see examples-advanced/runtime_demo_lowlevel.rs

#[cfg(not(all(feature = "mode-async", feature = "runtime-tokio")))]
fn main() {
    eprintln!("This example requires the 'async' and 'runtime-tokio' features to be enabled.");
    eprintln!("Run with: cargo run --example runtime_demo --features async,runtime-tokio");
}

#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    runtime::TokioRuntime,
    Error,
};

#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Camera configuration
    let camera_address = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.100".to_string());

    println!("Connecting to camera at {camera_address}...");

    // Create camera using high-level API
    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(camera_address, runtime).await?;

    println!("Connected! Demonstrating runtime features with high-level API...");

    // Power on the camera using accessor
    println!("\n1. Powering on camera...");
    match camera.power().on().await {
        Ok(_) => println!("   ✓ Camera powered on"),
        Err(e) => println!("   ✗ Power on failed: {e}"),
    }

    // Wait a moment for the camera to initialize
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Send zoom in command using accessor
    println!("\n2. Starting zoom in...");
    match camera.zoom().tele().await {
        Ok(_) => println!("   ✓ Zoom in started"),
        Err(e) => println!("   ✗ Zoom in failed: {e}"),
    }

    // Wait for zoom to move
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    // Stop zoom using accessor
    println!("\n3. Stopping zoom...");
    match camera.zoom().stop().await {
        Ok(_) => println!("   ✓ Zoom stopped"),
        Err(e) => println!("   ✗ Zoom stop failed: {e}"),
    }

    // Get power state using accessor
    println!("\n4. Getting power state...");
    match camera.power().state().await {
        Ok(is_on) => {
            let status = if is_on { "ON" } else { "OFF" };
            println!("   ✓ Power status: {status}");
        }
        Err(e) => println!("   ✗ Power inquiry failed: {e}"),
    }

    // Get runtime metrics (if available through Camera API)
    println!("\n5. Runtime metrics demonstration...");
    println!("   Note: Metrics access through high-level API");
    println!("   For detailed runtime metrics, see examples-advanced/");

    // Demonstrate concurrent operations with high-level API
    println!("\n6. Demonstrating concurrent operations...");
    println!("   Sending concurrent zoom and inquiry commands...");

    // Launch concurrent operations - need to bind accessors first
    let zoom_accessor = camera.zoom();
    let power_accessor = camera.power();
    let zoom_accessor2 = camera.zoom();

    let zoom_future = zoom_accessor.wide();
    let state_future = power_accessor.state();
    let position_future = zoom_accessor2.position();

    // Await all operations concurrently
    let (zoom_result, state_result, position_result) =
        tokio::join!(zoom_future, state_future, position_future);

    match zoom_result {
        Ok(_) => println!("   ✓ Zoom wide command completed"),
        Err(e) => println!("   ✗ Zoom wide failed: {e}"),
    }

    match state_result {
        Ok(is_on) => {
            let status = if is_on { "ON" } else { "OFF" };
            println!("   ✓ Power state: {status}");
        }
        Err(e) => println!("   ✗ State inquiry failed: {e}"),
    }

    match position_result {
        Ok(pos) => println!("   ✓ Zoom position: {:?}", pos),
        Err(e) => println!("   ✗ Position inquiry failed: {e}"),
    }

    // Stop zoom after concurrent operations
    camera.zoom().stop().await?;

    // Additional high-level operations demonstration
    println!("\n7. Additional accessor examples...");

    // System information
    println!("   Getting system version...");
    match camera.system().version().await {
        Ok(version) => println!("   ✓ System version: {:?}", version),
        Err(e) => println!("   ✗ Version inquiry failed: {e}"),
    }

    // Pan/Tilt demonstration
    println!("   Moving to home position...");
    match camera.pan_tilt().home().await {
        Ok(_) => println!("   ✓ Pan/Tilt home command sent"),
        Err(e) => println!("   ✗ Pan/Tilt home failed: {e}"),
    }

    println!("\n✓ Demo complete - camera connection will close automatically");

    Ok(())
}
