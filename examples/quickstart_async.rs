//! Quickstart async example - minimal VISCA camera control with async/await.
//!
//! This example demonstrates the simplest way to connect to and control a PTZ camera
//! using the async API with tokio. It covers:
//! - Connecting to a camera asynchronously
//! - Basic movement commands (pan, tilt, zoom)
//! - Saving and recalling presets
//!
//! Run with:
//! ```sh
//! cargo run --example quickstart_async --features tokio [camera_ip[:port]]
//! ```
//!
//! If no address is provided, defaults to 192.168.0.110 (PTZOptics test camera).
//! If no port is provided, the builder automatically selects the correct default
//! based on the camera profile and transport type.

#[cfg(feature = "tokio")]
use grafton_visca::{
    prelude::r#async::*,
    types::{PanSpeed, TiltSpeed},
    CameraBuilder, Error, PanTiltDirection,
};
#[cfg(feature = "tokio")]
use std::env;
#[cfg(feature = "tokio")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging (set RUST_LOG=debug for verbose output)
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Connecting to camera at {camera_addr} (async mode)");

    // Create camera using the async builder pattern
    // The builder automatically adds the correct default port if not specified:
    // - TCP: 5678 for PTZOptics, 52381 for Sony
    // - UDP: 1259 for PTZOptics, 52381 for Sony
    let camera = CameraBuilder::tokio_tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()
        .await?;

    println!("✅ Connected successfully!");
    println!();

    // Save initial camera state
    let initial_state = camera.save_state_async().await?;
    println!(
        "💾 Saved initial state: Pan={:.1}°, Tilt={:.1}°, Zoom={}",
        initial_state.pan, initial_state.tilt, initial_state.zoom
    );

    // Power on the camera
    println!("📍 Powering on camera...");
    camera.power_on().await?;
    sleep(Duration::from_secs(2)).await; // Wait for camera to initialize - can't use wait helpers during power-on

    // Move to home position
    println!("🏠 Moving to home position...");
    camera.pan_tilt_home().await?;
    camera
        .wait_for_pan_tilt_completion(Duration::from_secs(10))
        .await?; // Wait for movement to complete

    // Demonstrate zoom control
    println!("🔍 Testing zoom...");
    println!("   Zooming in...");
    camera.zoom_in().await?;
    sleep(Duration::from_secs(2)).await;

    println!("   Stopping zoom...");
    camera.zoom_stop().await?;
    sleep(Duration::from_millis(500)).await;

    println!("   Zooming out...");
    camera.zoom_out().await?;
    sleep(Duration::from_secs(2)).await;

    println!("   Stopping zoom...");
    camera.zoom_stop().await?;

    // Demonstrate pan/tilt control
    println!("🔄 Testing pan/tilt...");
    println!("   Panning right...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Right,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await?;
    sleep(Duration::from_secs(1)).await;

    println!("   Stopping movement...");
    camera.pan_tilt_stop().await?;
    sleep(Duration::from_millis(500)).await;

    println!("   Tilting up...");
    camera
        .pan_tilt_move(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)
        .await?;
    sleep(Duration::from_secs(1)).await;

    println!("   Stopping movement...");
    camera.pan_tilt_stop().await?;

    // Return to home
    println!("🏠 Returning to home position...");
    camera.pan_tilt_home().await?;
    camera
        .wait_for_pan_tilt_completion(Duration::from_secs(10))
        .await?;

    // Demonstrate presets
    println!("💾 Testing presets...");
    println!("   Saving current position as preset 1...");
    camera.preset_set(PresetNumber::new(1)?).await?;
    sleep(Duration::from_millis(500)).await;

    // Move away from saved position
    println!("   Moving to a different position...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Left,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await?;
    sleep(Duration::from_secs(2)).await;
    camera.pan_tilt_stop().await?;

    // Recall the preset
    println!("   Recalling preset 1...");
    camera.preset_recall(PresetNumber::new(1)?).await?;
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;

    // Restore initial camera state
    println!("🔄 Restoring initial camera state...");
    camera.restore_state_async(&initial_state).await?;
    println!("✅ Camera restored to initial state");

    println!();
    println!("✨ Async quickstart complete!");
    println!();
    println!("Next steps:");
    println!("  - Try the quickstart example for blocking/sync support");
    println!("  - See camera_control_async for comprehensive async operations");
    println!("  - Check concurrent_control for parallel camera operations");

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example quickstart_async --features tokio");
    std::process::exit(1);
}
