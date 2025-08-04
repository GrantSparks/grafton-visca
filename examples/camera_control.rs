//! Comprehensive camera control example - all camera operations.
//!
//! This example demonstrates the full range of camera control operations available
//! in the blocking API, including:
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
//! cargo run --example camera_control [camera_ip[:port]]
//! ```

use grafton_visca::{
    prelude::blocking::*,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    units::*,
    CameraBuilder, Error, PanTiltDirection,
};
use std::time::Duration;
use std::{env, thread::sleep};

fn main() -> Result<(), Error> {
    // Initialize logging (set RUST_LOG=debug for verbose output)
    env_logger::init();

    // Get camera address from command line or use default
    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Comprehensive Camera Control Demo");
    println!("====================================");
    println!("Connecting to camera at {camera_addr}");
    println!();

    // Create camera using the builder pattern
    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()?;

    println!("✅ Connected successfully!");
    println!();

    // === POWER MANAGEMENT ===
    println!("═══ Power Management ═══");
    println!("Powering on camera...");
    camera.power_on()?;
    sleep(Duration::from_secs(2));
    println!("✓ Camera powered on");
    println!();

    // === PAN/TILT OPERATIONS ===
    println!("═══ Pan/Tilt Operations ═══");

    // Home position
    println!("Moving to home position...");
    camera.pan_tilt_home()?;
    sleep(Duration::from_secs(3));
    println!("✓ At home position");

    // Absolute positioning with degrees
    println!("Moving to absolute position (45°, 15°)...");
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)?;
    sleep(Duration::from_secs(3));
    println!("✓ Moved to position");

    // Relative movement
    println!("Moving relative (+10°, +5°)...");
    camera.pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)?;
    sleep(Duration::from_secs(2));
    println!("✓ Relative movement complete");

    // Directional movements
    println!("Testing directional movements...");
    println!("  Panning left...");
    camera.pan_tilt_move(
        PanTiltDirection::Left,
        PanSpeed::new(10)?,
        TiltSpeed::new(0)?,
    )?;
    sleep(Duration::from_millis(500));
    camera.pan_tilt_stop()?;

    println!("  Panning right...");
    camera.pan_tilt_move(
        PanTiltDirection::Right,
        PanSpeed::new(10)?,
        TiltSpeed::new(0)?,
    )?;
    sleep(Duration::from_millis(500));
    camera.pan_tilt_stop()?;

    println!("  Tilting up...");
    camera.pan_tilt_move(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)?;
    sleep(Duration::from_millis(500));
    camera.pan_tilt_stop()?;

    println!("  Tilting down...");
    camera.pan_tilt_move(
        PanTiltDirection::Down,
        PanSpeed::new(0)?,
        TiltSpeed::new(10)?,
    )?;
    sleep(Duration::from_millis(500));
    camera.pan_tilt_stop()?;

    println!("✓ Directional movements complete");
    println!();

    // === ZOOM OPERATIONS ===
    println!("═══ Zoom Operations ═══");

    // Set zoom to minimum
    println!("Setting zoom to minimum (1x)...");
    camera.zoom_absolute(Normalized(0.0))?;
    sleep(Duration::from_secs(2));
    println!("✓ Zoom at minimum");

    // Standard zoom controls
    println!("Testing standard zoom...");
    println!("  Zooming in (standard speed)...");
    camera.zoom_in_standard()?;
    sleep(Duration::from_secs(1));
    camera.zoom_stop()?;
    println!("  ✓ Zoom in complete");

    println!("  Zooming out (standard speed)...");
    camera.zoom_out_standard()?;
    sleep(Duration::from_secs(1));
    camera.zoom_stop()?;
    println!("  ✓ Zoom out complete");

    // Variable speed zoom
    println!("Testing variable speed zoom...");
    println!("  Zooming in (fast)...");
    camera.zoom_in()?;
    sleep(Duration::from_millis(500));
    camera.zoom_stop()?;

    println!("  Zooming out (fast)...");
    camera.zoom_out()?;
    sleep(Duration::from_millis(500));
    camera.zoom_stop()?;
    println!("✓ Variable speed zoom complete");

    // Absolute zoom positioning
    println!("Setting zoom to 50% (normalized)...");
    camera.zoom_absolute(Normalized(0.5))?;
    sleep(Duration::from_secs(2));
    println!("✓ Zoom at 50%");
    println!();

    // === FOCUS OPERATIONS ===
    println!("═══ Focus Operations ═══");

    // Auto focus
    println!("Setting auto focus...");
    camera.focus_auto()?;
    sleep(Duration::from_secs(1));
    println!("✓ Auto focus enabled");

    // Manual focus
    println!("Testing manual focus...");
    camera.focus_manual()?;
    println!("  Focusing near...");
    camera.focus_near(SpeedLevel::Medium)?;
    sleep(Duration::from_millis(500));
    camera.focus_stop()?;

    println!("  Focusing far...");
    camera.focus_far(SpeedLevel::Medium)?;
    sleep(Duration::from_millis(500));
    camera.focus_stop()?;
    println!("✓ Manual focus complete");

    // One-push auto focus
    println!("Triggering one-push auto focus...");
    camera.focus_one_push()?;
    sleep(Duration::from_secs(2));
    println!("✓ One-push focus complete");

    // Return to auto focus
    camera.focus_auto()?;
    println!();

    // === EXPOSURE CONTROL ===
    println!("═══ Exposure Control ═══");

    // Auto exposure
    println!("Setting auto exposure...");
    camera.exposure_auto()?;
    println!("✓ Auto exposure enabled");

    // Manual exposure
    println!("Setting manual exposure mode...");
    camera.exposure_manual()?;
    println!("✓ Manual exposure enabled");

    // Shutter priority
    println!("Setting shutter priority mode...");
    camera.exposure_shutter_priority()?;
    println!("✓ Shutter priority enabled");

    // Back to auto
    camera.exposure_auto()?;
    println!();

    // === WHITE BALANCE ===
    println!("═══ White Balance ═══");

    println!("Setting auto white balance...");
    camera.white_balance_auto()?;
    println!("✓ Auto white balance enabled");

    println!("Setting indoor white balance...");
    camera.white_balance_indoor()?;
    println!("✓ Indoor white balance set");

    println!("Setting outdoor white balance...");
    camera.white_balance_outdoor()?;
    println!("✓ Outdoor white balance set");

    println!("Setting one-push white balance...");
    camera.white_balance_one_push()?;
    println!("✓ One-push white balance set");

    // Return to auto
    camera.white_balance_auto()?;
    println!();

    // === IMAGE ADJUSTMENTS ===
    println!("═══ Image Adjustments ═══");

    // Flip control
    println!("Testing image flip...");
    camera.enable_flip()?;
    println!("✓ Image flip enabled");
    sleep(Duration::from_secs(1));

    camera.disable_flip()?;
    println!("✓ Image flip disabled");
    println!();

    // === PRESET MANAGEMENT ===
    println!("═══ Preset Management ═══");

    // Save presets at different positions
    println!("Saving presets...");

    // Save home as preset 1
    camera.pan_tilt_home()?;
    sleep(Duration::from_secs(2));
    camera.preset_set(PresetNumber::new(1)?)?;
    println!("  ✓ Saved home position as preset 1");

    // Move and save as preset 2
    camera.pan_tilt_absolute(Degrees(90.0), Degrees(0.0), SpeedLevel::Fast)?;
    sleep(Duration::from_secs(3));
    camera.preset_set(PresetNumber::new(2)?)?;
    println!("  ✓ Saved position (90°, 0°) as preset 2");

    // Move and save as preset 3
    camera.pan_tilt_absolute(Degrees(-90.0), Degrees(30.0), SpeedLevel::Fast)?;
    sleep(Duration::from_secs(3));
    camera.preset_set(PresetNumber::new(3)?)?;
    println!("  ✓ Saved position (-90°, 30°) as preset 3");

    // Recall presets
    println!("Recalling presets...");

    println!("  Recalling preset 1 (home)...");
    camera.preset_recall(PresetNumber::new(1)?)?;
    sleep(Duration::from_secs(3));
    println!("  ✓ At preset 1");

    println!("  Recalling preset 2...");
    camera.preset_recall(PresetNumber::new(2)?)?;
    sleep(Duration::from_secs(3));
    println!("  ✓ At preset 2");

    println!("  Recalling preset 3...");
    camera.preset_recall(PresetNumber::new(3)?)?;
    sleep(Duration::from_secs(3));
    println!("  ✓ At preset 3");

    // Clear a preset
    println!("Clearing preset 3...");
    camera.preset_reset(PresetNumber::new(3)?)?;
    println!("✓ Preset 3 cleared");
    println!();

    // === RETURN TO HOME ===
    println!("═══ Finishing Demo ═══");
    println!("Returning to home position...");
    camera.pan_tilt_home()?;
    camera.zoom_absolute(Normalized(0.0))?;
    sleep(Duration::from_secs(3));

    println!();
    println!("✨ Camera control demo complete!");
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
    println!();
    println!("See camera_control_async for the async version of this demo.");

    Ok(())
}
