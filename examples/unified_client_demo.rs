//! Demo of the unified Camera API in both blocking and async contexts.
//!
//! This example shows how the Camera API works seamlessly with
//! different transport adapters for blocking and async usage.

#[cfg(any(not(feature = "async"), feature = "tokio"))]
use grafton_visca::{
    camera::{
        methods::{FocusMethodsExt, PanTiltMethodsExt, PowerMethodsExt, ZoomMethodsExt},
        Camera,
    },
    command::pan_tilt::PanTiltDirection,
    profiles::PTZOpticsG2,
    types::{PanSpeed, TiltSpeed},
    units::Degrees,
    Error,
};
#[cfg(any(not(feature = "async"), feature = "tokio"))]
use std::time::Duration;

#[cfg(not(feature = "async"))]
use grafton_visca::transport::blocking::create;
#[cfg(all(feature = "async", feature = "tokio"))]
use grafton_visca::transport::create;

// ==================== BLOCKING EXAMPLES ====================

#[cfg(not(feature = "async"))]
fn blocking_udp_example() -> Result<(), Error> {
    println!("=== Blocking UDP Example ===");

    // Create a camera with blocking UDP transport
    let transport = create::udp("192.168.1.100:5678")?;
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    // All operations are synchronous
    println!("Powering on camera...");
    camera.power_on()?;

    println!("Moving to home position...");
    camera.pan_tilt_home()?;

    println!("Zooming in...");
    camera.zoom_in()?;
    std::thread::sleep(Duration::from_secs(1));
    camera.zoom_stop()?;

    Ok(())
}

#[cfg(not(feature = "async"))]
fn blocking_tcp_example() -> Result<(), Error> {
    println!("\n=== Blocking TCP Example ===");

    // Create a camera with blocking TCP transport
    let transport = create::tcp("192.168.1.100:5678")?;
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    // All operations are synchronous
    println!("Powering on camera...");
    camera.power_on()?;

    println!("Setting position...");
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0))?;

    println!("Adjusting focus...");
    camera.focus_auto()?;

    Ok(())
}

#[cfg(not(feature = "async"))]
fn blocking_movement_example() -> Result<(), Error> {
    println!("\n=== Blocking Movement Example ===");

    let transport = create::udp("192.168.1.100:5678")?;
    let mut camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("Moving camera up...");
    camera.pan_tilt_move(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)?;
    std::thread::sleep(Duration::from_millis(500));
    camera.pan_tilt_stop()?;

    println!("Moving camera down...");
    camera.pan_tilt_move(
        PanTiltDirection::Down,
        PanSpeed::new(0)?,
        TiltSpeed::new(10)?,
    )?;
    std::thread::sleep(Duration::from_millis(500));
    camera.pan_tilt_stop()?;

    Ok(())
}

// ==================== ASYNC EXAMPLES ====================

#[cfg(all(feature = "async", feature = "tokio"))]
async fn async_udp_example() -> Result<(), Error> {
    println!("=== Async UDP Example ===");

    // Create a camera with async UDP transport
    let transport = create::udp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // All operations are async
    println!("Powering on camera...");
    camera.power_on().await?;

    println!("Moving to home position...");
    camera.pan_tilt_home().await?;

    println!("Zooming in...");
    camera.zoom_in().await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;

    Ok(())
}

#[cfg(all(feature = "async", feature = "tokio"))]
async fn async_tcp_example() -> Result<(), Error> {
    println!("\n=== Async TCP Example ===");

    // Create a camera with async TCP transport
    let transport = create::tcp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // All operations are async
    println!("Powering on camera...");
    camera.power_on().await?;

    println!("Setting position...");
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0)).await?;

    println!("Adjusting focus...");
    camera.focus_auto().await?;

    Ok(())
}

#[cfg(all(feature = "async", feature = "tokio"))]
async fn async_movement_example() -> Result<(), Error> {
    println!("\n=== Async Movement Example ===");

    let transport = create::udp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("Moving camera up...");
    camera
        .pan_tilt_move(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)
        .await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    camera.pan_tilt_stop().await?;

    println!("Moving camera down...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Down,
            PanSpeed::new(0)?,
            TiltSpeed::new(10)?,
        )
        .await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    camera.pan_tilt_stop().await?;

    Ok(())
}

// ==================== MAIN FUNCTIONS ====================

#[cfg(all(feature = "async", not(feature = "tokio")))]
fn main() {
    eprintln!("This example requires either no features (blocking) or tokio feature (async).");
    eprintln!("Run with: cargo run --example unified_client_demo --features tokio");
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== Unified Camera API Demo (Blocking Mode) ===\n");
    println!("This demo shows how the Camera API works in blocking mode.\n");

    // Run blocking examples
    blocking_udp_example()?;
    blocking_tcp_example()?;
    blocking_movement_example()?;

    println!("\n=== Demo Complete ===");
    println!("\nKey takeaways:");
    println!("• Camera API provides synchronous operations in blocking mode");
    println!("• Works with blocking UDP and TCP transports");
    println!("• Same API surface as async mode");
    println!("• Methods take &mut self for blocking operations");

    Ok(())
}

#[cfg(all(feature = "async", feature = "tokio"))]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== Unified Camera API Demo (Async Mode) ===\n");
    println!("This demo shows how the Camera API works in async mode.\n");

    // Run async examples
    async_udp_example().await?;
    async_tcp_example().await?;
    async_movement_example().await?;

    println!("\n=== Demo Complete ===");
    println!("\nKey takeaways:");
    println!("• Camera API provides async operations with .await");
    println!("• Works with async UDP and TCP transports");
    println!("• Same API surface as blocking mode");
    println!("• Methods take &self for async operations (interior mutability)");

    Ok(())
}
