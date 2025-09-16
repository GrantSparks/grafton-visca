//! Async camera control quickstart example.
//!
//! This example demonstrates basic async camera control operations using the high-level
//! Camera API with your choice of async runtime (tokio, async-std, or smol).
//!
//! The example shows:
//! - Connection and power management
//! - Pan/Tilt/Zoom (PTZ) operations
//! - Focus control
//! - Using the accessor-based API
//!
//! Run with your preferred runtime:
//! ```sh
//! # With tokio (most common)
//! cargo run --example quickstart_async --features rt-tokio [camera_ip[:port]]
//!
//! # With async-std
//! cargo run --example quickstart_async --features rt-async-std [camera_ip[:port]]
//!
//! # With smol
//! cargo run --example quickstart_async --features rt-smol [camera_ip[:port]]
//! ```

#[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
use std::env;

#[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    Camera, Error,
};

// Main function for when no runtime is selected
#[cfg(not(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
fn main() {
    eprintln!("This example requires an async runtime feature.");
    eprintln!("Run with one of:");
    eprintln!("  cargo run --example quickstart_async --features rt-tokio");
    eprintln!("  cargo run --example quickstart_async --features rt-async-std");
    eprintln!("  cargo run --example quickstart_async --features rt-smol");
}

// =================== TOKIO RUNTIME ===================
#[cfg(all(
    feature = "rt-tokio",
    not(any(feature = "rt-async-std", feature = "rt-smol"))
))]
#[tokio::main]
async fn main() -> Result<(), Error> {
    use tokio::time::{sleep, Duration};

    use grafton_visca::runtime_trait::TokioRuntime;

    tracing_subscriber::fmt::init();

    // Parse camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:52381".to_string());

    println!("=== Async Quickstart with Tokio ===");
    println!("Connecting to camera at {camera_addr}...\n");

    // Connect to camera using the high-level API
    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&camera_addr, runtime).await?;
    println!("✓ Connected successfully");

    // Power on the camera
    println!("\n--- Power Management ---");
    println!("Powering on...");
    camera.power().on().await?;

    // Wait a moment for power-on to complete
    sleep(Duration::from_secs(2)).await;

    // Check power state
    let is_on = camera.power().state().await?;
    println!("Power state: {}", if is_on { "ON" } else { "OFF" });

    // PTZ operations using accessors
    println!("\n--- PTZ Operations ---");

    // Zoom operations
    println!("Testing zoom...");
    camera.zoom().stop().await?;
    sleep(Duration::from_millis(500)).await;

    camera.zoom().tele().await?;
    println!("  Zooming in (tele)...");
    sleep(Duration::from_secs(2)).await;

    camera.zoom().wide().await?;
    println!("  Zooming out (wide)...");
    sleep(Duration::from_secs(2)).await;

    camera.zoom().stop().await?;
    println!("  Zoom stopped");

    // Pan/Tilt operations
    println!("\nTesting pan/tilt...");
    camera.pan_tilt().home().await?;
    println!("  Moving to home position...");
    sleep(Duration::from_secs(3)).await;

    // Move to specific position
    println!("  Moving to center position...");
    camera
        .pan_tilt()
        .absolute(
            grafton_visca::units::Degrees(0.0),
            grafton_visca::units::Degrees(0.0),
            grafton_visca::types::SpeedLevel::Medium,
        )
        .await?;
    sleep(Duration::from_secs(2)).await;

    // Focus operations
    println!("\n--- Focus Control ---");
    camera.focus().auto().await?;
    println!("Focus mode set to auto");

    // Final home position
    println!("\n--- Returning Home ---");
    camera.pan_tilt().home().await?;
    println!("Moved to home position");

    println!("\n✓ Quickstart completed successfully!");
    println!("  Runtime: tokio");
    println!("  Camera: {camera_addr}");

    Ok(())
}

// =================== ASYNC-STD RUNTIME ===================
#[cfg(all(feature = "rt-async-std", not(feature = "rt-smol")))]
#[async_std::main]
async fn main() -> Result<(), Error> {
    use async_std::task::sleep;

    use std::time::Duration;

    use grafton_visca::runtime_trait::AsyncStdRuntime;

    tracing_subscriber::fmt::init();

    // Parse camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:52381".to_string());

    println!("=== Async Quickstart with async-std ===");
    println!("Connecting to camera at {camera_addr}...\n");

    // Connect to camera using the high-level API
    let runtime = AsyncStdRuntime::new();
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&camera_addr, runtime).await?;
    println!("✓ Connected successfully");

    // Power on the camera
    println!("\n--- Power Management ---");
    println!("Powering on...");
    camera.power().on().await?;

    // Wait a moment for power-on to complete
    sleep(Duration::from_secs(2)).await;

    // Check power state
    let is_on = camera.power().state().await?;
    println!("Power state: {}", if is_on { "ON" } else { "OFF" });

    // PTZ operations using accessors
    println!("\n--- PTZ Operations ---");

    // Zoom operations
    println!("Testing zoom...");
    camera.zoom().stop().await?;
    sleep(Duration::from_millis(500)).await;

    camera.zoom().tele().await?;
    println!("  Zooming in (tele)...");
    sleep(Duration::from_secs(2)).await;

    camera.zoom().wide().await?;
    println!("  Zooming out (wide)...");
    sleep(Duration::from_secs(2)).await;

    camera.zoom().stop().await?;
    println!("  Zoom stopped");

    // Pan/Tilt operations
    println!("\nTesting pan/tilt...");
    camera.pan_tilt().home().await?;
    println!("  Moving to home position...");
    sleep(Duration::from_secs(3)).await;

    // Move to specific position
    println!("  Moving to center position...");
    camera
        .pan_tilt()
        .absolute(
            grafton_visca::units::Degrees(0.0),
            grafton_visca::units::Degrees(0.0),
            grafton_visca::types::SpeedLevel::Medium,
        )
        .await?;
    sleep(Duration::from_secs(2)).await;

    // Focus operations
    println!("\n--- Focus Control ---");
    camera.focus().auto().await?;
    println!("Focus mode set to auto");

    // Final home position
    println!("\n--- Returning Home ---");
    camera.pan_tilt().home().await?;
    println!("Moved to home position");

    println!("\n✓ Quickstart completed successfully!");
    println!("  Runtime: async-std");
    println!("  Camera: {camera_addr}");

    Ok(())
}

// =================== SMOL RUNTIME ===================
#[cfg(feature = "rt-smol")]
fn main() -> Result<(), Error> {
    use grafton_visca::runtime_trait::SmolRuntime;

    tracing_subscriber::fmt::init();

    smol::block_on(async {
        // Parse camera address from command line or use default
        let camera_addr = env::args()
            .nth(1)
            .unwrap_or_else(|| "192.168.0.110:52381".to_string());

        println!("=== Async Quickstart with smol ===");
        println!("Connecting to camera at {camera_addr}...\n");

        // Connect to camera using the high-level API
        let runtime = SmolRuntime::new();
        let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&camera_addr, runtime).await?;
        println!("✓ Connected successfully");

        // Power on the camera
        println!("\n--- Power Management ---");
        println!("Powering on...");
        camera.power().on().await?;

        // Wait a moment for power-on to complete
        smol::Timer::after(std::time::Duration::from_secs(2)).await;

        // Check power state
        let is_on = camera.power().state().await?;
        println!("Power state: {}", if is_on { "ON" } else { "OFF" });

        // PTZ operations using accessors
        println!("\n--- PTZ Operations ---");

        // Zoom operations
        println!("Testing zoom...");
        camera.zoom().stop().await?;
        smol::Timer::after(std::time::Duration::from_millis(500)).await;

        camera.zoom().tele().await?;
        println!("  Zooming in (tele)...");
        smol::Timer::after(std::time::Duration::from_secs(2)).await;

        camera.zoom().wide().await?;
        println!("  Zooming out (wide)...");
        smol::Timer::after(std::time::Duration::from_secs(2)).await;

        camera.zoom().stop().await?;
        println!("  Zoom stopped");

        // Pan/Tilt operations
        println!("\nTesting pan/tilt...");
        camera.pan_tilt().home().await?;
        println!("  Moving to home position...");
        smol::Timer::after(std::time::Duration::from_secs(3)).await;

        // Move to specific position
        println!("  Moving to center position...");
        camera
            .pan_tilt()
            .absolute(
                grafton_visca::units::Degrees(0.0),
                grafton_visca::units::Degrees(0.0),
                grafton_visca::types::SpeedLevel::Medium,
            )
            .await?;
        smol::Timer::after(std::time::Duration::from_secs(2)).await;

        // Focus operations
        println!("\n--- Focus Control ---");
        camera.focus().auto().await?;
        println!("Focus mode set to auto");

        // Final home position
        println!("\n--- Returning Home ---");
        camera.pan_tilt().home().await?;
        println!("Moved to home position");

        println!("\n✓ Quickstart completed successfully!");
        println!("  Runtime: smol");
        println!("  Camera: {camera_addr}");

        Ok(())
    })
}
