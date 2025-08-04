//! Comprehensive async camera control example - all camera operations.
//!
//! This example demonstrates the full range of camera control operations available
//! in the async API using tokio, including:
//! - Power management
//! - Pan/Tilt/Zoom (PTZ) operations
//! - Focus control
//! - Exposure settings
//! - White balance
//! - Image adjustments
//! - Presets management
//! - Speed control
//!
//! Run with:
//! ```sh
//! cargo run --example camera_control_async --features tokio [camera_ip[:port]]
//! ```

use grafton_visca::{
    prelude::r#async::*,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    units::*,
    CameraBuilder, Error, PanTiltDirection,
};
use std::env;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging (set RUST_LOG=debug for verbose output)
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Comprehensive Async Camera Control Demo");
    println!("==========================================");
    println!("Connecting to camera at {camera_addr}");
    println!();

    // Create camera using the builder pattern with async support
    let camera = CameraBuilder::tokio_tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()
        .await?;

    println!("✅ Connected successfully!");
    println!();

    // === POWER MANAGEMENT ===
    println!("═══ Power Management ═══");
    println!("Powering on camera...");
    camera.power_on().await?;
    sleep(Duration::from_secs(2)).await;
    println!("✓ Camera powered on");
    println!();

    // === PAN/TILT OPERATIONS ===
    println!("═══ Pan/Tilt Operations ═══");

    // Home position
    println!("Moving to home position...");
    camera.pan_tilt_home().await?;
    sleep(Duration::from_secs(3)).await;
    println!("✓ At home position");

    // Absolute positioning with degrees
    println!("Moving to absolute position (45°, 15°)...");
    camera
        .pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)
        .await?;
    sleep(Duration::from_secs(3)).await;
    println!("✓ Moved to position");

    // Relative movement
    println!("Moving relative (+10°, +5°)...");
    camera
        .pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)
        .await?;
    sleep(Duration::from_secs(2)).await;
    println!("✓ Relative movement complete");

    // Directional movements
    println!("Testing directional movements...");
    println!("  Panning left...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Left,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await?;
    sleep(Duration::from_millis(500)).await;
    camera.pan_tilt_stop().await?;

    println!("  Panning right...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Right,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await?;
    sleep(Duration::from_millis(500)).await;
    camera.pan_tilt_stop().await?;

    println!("  Tilting up...");
    camera
        .pan_tilt_move(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)
        .await?;
    sleep(Duration::from_millis(500)).await;
    camera.pan_tilt_stop().await?;

    println!("  Tilting down...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Down,
            PanSpeed::new(0)?,
            TiltSpeed::new(10)?,
        )
        .await?;
    sleep(Duration::from_millis(500)).await;
    camera.pan_tilt_stop().await?;

    println!("✓ Directional movements complete");
    println!();

    // === ZOOM OPERATIONS ===
    println!("═══ Zoom Operations ═══");

    // Set zoom to minimum
    println!("Setting zoom to minimum (1x)...");
    camera.zoom_absolute(Normalized(0.0)).await?;
    sleep(Duration::from_secs(2)).await;
    println!("✓ Zoom at minimum");

    // Standard zoom controls
    println!("Testing standard zoom...");
    println!("  Zooming in (standard speed)...");
    camera.zoom_in_standard().await?;
    sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;
    println!("  ✓ Zoom in complete");

    println!("  Zooming out (standard speed)...");
    camera.zoom_out_standard().await?;
    sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;
    println!("  ✓ Zoom out complete");

    // Variable speed zoom
    println!("Testing variable speed zoom...");
    println!("  Zooming in (fast)...");
    camera.zoom_in().await?;
    sleep(Duration::from_millis(500)).await;
    camera.zoom_stop().await?;

    println!("  Zooming out (fast)...");
    camera.zoom_out().await?;
    sleep(Duration::from_millis(500)).await;
    camera.zoom_stop().await?;
    println!("✓ Variable speed zoom complete");

    // Absolute zoom positioning
    println!("Setting zoom to 50% (normalized)...");
    camera.zoom_absolute(Normalized(0.5)).await?;
    sleep(Duration::from_secs(2)).await;
    println!("✓ Zoom at 50%");
    println!();

    // === FOCUS OPERATIONS ===
    println!("═══ Focus Operations ═══");

    // Auto focus
    println!("Setting auto focus...");
    camera.focus_auto().await?;
    sleep(Duration::from_secs(1)).await;
    println!("✓ Auto focus enabled");

    // Manual focus
    println!("Testing manual focus...");
    camera.focus_manual().await?;
    println!("  Focusing near...");
    camera.focus_near(SpeedLevel::Medium).await?;
    sleep(Duration::from_millis(500)).await;
    camera.focus_stop().await?;

    println!("  Focusing far...");
    camera.focus_far(SpeedLevel::Medium).await?;
    sleep(Duration::from_millis(500)).await;
    camera.focus_stop().await?;
    println!("✓ Manual focus complete");

    // One-push auto focus
    println!("Triggering one-push auto focus...");
    camera.focus_one_push().await?;
    sleep(Duration::from_secs(2)).await;
    println!("✓ One-push focus complete");

    // Return to auto focus
    camera.focus_auto().await?;
    println!();

    // === EXPOSURE CONTROL ===
    println!("═══ Exposure Control ═══");

    // Auto exposure
    println!("Setting auto exposure...");
    camera.exposure_auto().await?;
    println!("✓ Auto exposure enabled");

    // Manual exposure
    println!("Setting manual exposure mode...");
    camera.exposure_manual().await?;
    println!("✓ Manual exposure enabled");

    // Shutter priority
    println!("Setting shutter priority mode...");
    camera.exposure_shutter_priority().await?;
    println!("✓ Shutter priority enabled");

    // Back to auto
    camera.exposure_auto().await?;
    println!();

    // === WHITE BALANCE ===
    println!("═══ White Balance ═══");

    println!("Setting auto white balance...");
    camera.white_balance_auto().await?;
    println!("✓ Auto white balance enabled");

    println!("Setting indoor white balance...");
    camera.white_balance_indoor().await?;
    println!("✓ Indoor white balance set");

    println!("Setting outdoor white balance...");
    camera.white_balance_outdoor().await?;
    println!("✓ Outdoor white balance set");

    println!("Setting one-push white balance...");
    camera.white_balance_one_push().await?;
    println!("✓ One-push white balance set");

    // Return to auto
    camera.white_balance_auto().await?;
    println!();

    // === IMAGE ADJUSTMENTS ===
    println!("═══ Image Adjustments ═══");

    // Flip control
    println!("Testing image flip...");
    camera.enable_flip().await?;
    println!("✓ Image flip enabled");
    sleep(Duration::from_secs(1)).await;

    camera.disable_flip().await?;
    println!("✓ Image flip disabled");
    println!();

    // === PRESET MANAGEMENT ===
    println!("═══ Preset Management ═══");

    // Save presets at different positions
    println!("Saving presets...");

    // Save home as preset 1
    camera.pan_tilt_home().await?;
    sleep(Duration::from_secs(2)).await;
    camera.preset_set(PresetNumber::new(1)?).await?;
    println!("  ✓ Saved home position as preset 1");

    // Move and save as preset 2
    camera
        .pan_tilt_absolute(Degrees(90.0), Degrees(0.0), SpeedLevel::Fast)
        .await?;
    sleep(Duration::from_secs(3)).await;
    camera.preset_set(PresetNumber::new(2)?).await?;
    println!("  ✓ Saved position (90°, 0°) as preset 2");

    // Move and save as preset 3
    camera
        .pan_tilt_absolute(Degrees(-90.0), Degrees(30.0), SpeedLevel::Fast)
        .await?;
    sleep(Duration::from_secs(3)).await;
    camera.preset_set(PresetNumber::new(3)?).await?;
    println!("  ✓ Saved position (-90°, 30°) as preset 3");

    // Recall presets
    println!("Recalling presets...");

    println!("  Recalling preset 1 (home)...");
    camera.preset_recall(PresetNumber::new(1)?).await?;
    sleep(Duration::from_secs(3)).await;
    println!("  ✓ At preset 1");

    println!("  Recalling preset 2...");
    camera.preset_recall(PresetNumber::new(2)?).await?;
    sleep(Duration::from_secs(3)).await;
    println!("  ✓ At preset 2");

    println!("  Recalling preset 3...");
    camera.preset_recall(PresetNumber::new(3)?).await?;
    sleep(Duration::from_secs(3)).await;
    println!("  ✓ At preset 3");

    // Clear a preset
    println!("Clearing preset 3...");
    camera.preset_reset(PresetNumber::new(3)?).await?;
    println!("✓ Preset 3 cleared");
    println!();

    // === CONCURRENT OPERATIONS DEMO ===
    println!("═══ Concurrent Operations (Async Advantage) ═══");
    println!("Demonstrating concurrent pan and zoom...");

    // Move home first
    camera.pan_tilt_home().await?;
    camera.zoom_absolute(Normalized(0.0)).await?;
    sleep(Duration::from_secs(2)).await;

    // Start concurrent operations
    let pan_handle = {
        let camera = camera.clone();
        tokio::spawn(async move {
            camera
                .pan_tilt_absolute(Degrees(90.0), Degrees(30.0), SpeedLevel::Slow)
                .await
                .unwrap();
        })
    };

    let zoom_handle = {
        let camera = camera.clone();
        tokio::spawn(async move {
            camera.zoom_absolute(Normalized(0.7)).await.unwrap();
        })
    };

    // Wait for both operations to complete
    pan_handle.await.unwrap();
    zoom_handle.await.unwrap();
    println!("✓ Concurrent operations complete");
    println!();

    // === RETURN TO HOME ===
    println!("═══ Finishing Demo ═══");
    println!("Returning to home position...");

    // Use concurrent operations to return home faster
    let (home_result, zoom_result) = tokio::join!(
        camera.pan_tilt_home(),
        camera.zoom_absolute(Normalized(0.0))
    );

    // Check both operations succeeded
    home_result?;
    zoom_result?;

    sleep(Duration::from_secs(3)).await;

    println!();
    println!("✨ Async camera control demo complete!");
    println!();
    println!("Explored features:");
    println!("  ✓ Power management");
    println!("  ✓ Pan/Tilt control (absolute, relative, directional)");
    println!("  ✓ Zoom control (standard, variable speed, absolute)");
    println!("  ✓ Focus control (auto, manual, one-push)");
    println!("  ✓ Exposure modes");
    println!("  ✓ White balance modes");
    println!("  ✓ Image adjustments (flip)");
    println!("  ✓ Preset management (save, recall, clear)");
    println!("  ✓ Concurrent operations (async advantage!)");
    println!();
    println!("The async API enables concurrent operations that aren't possible");
    println!("with the blocking API, allowing for more efficient camera control.");

    Ok(())
}
