//! Comprehensive camera control example using the blocking API.
//!
//! This example demonstrates the full range of camera control operations available
//! in the blocking API, including:
//! - Connection and power management
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
//! cargo run --example quickstart [camera_ip[:port]]
//! ```

use grafton_visca::{
    camera::profiles::PTZOpticsG2,
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

    // === SAVE INITIAL STATE ===
    println!("═══ Saving Initial Camera State ═══");
    let initial_position = camera.get_pan_tilt_degrees();
    let initial_zoom = camera.get_zoom_position();

    match (&initial_position, &initial_zoom) {
        (Ok((pan, tilt)), Ok(zoom)) => {
            println!(
                "✓ Saved initial position: Pan={:.1}°, Tilt={:.1}°",
                pan.0, tilt.0
            );
            println!("✓ Saved initial zoom: {zoom}");
        }
        _ => {
            println!("⚠ Could not save initial position/zoom, will return to home at end");
        }
    }
    println!();

    // === BASIC MOVEMENT ===
    println!("═══ Basic Movement Operations ═══");

    // Home position
    println!("Moving to home position...");
    camera.pan_tilt_home()?;
    camera.await_pan_tilt_idle(Duration::from_secs(30))?;
    println!("✓ At home position");

    // Demonstrate zoom control
    println!("Testing zoom...");
    println!("  Zooming in...");
    camera.zoom_in()?;
    sleep(Duration::from_secs(1));
    camera.zoom_stop()?;

    println!("  Zooming out...");
    camera.zoom_out()?;
    sleep(Duration::from_secs(1));
    camera.zoom_stop()?;
    println!("✓ Zoom complete");

    // Demonstrate pan/tilt control
    println!("Testing pan/tilt...");
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
    println!("✓ Pan/tilt complete");
    println!();

    // === ADVANCED POSITIONING ===
    println!("═══ Advanced Positioning ═══");

    // Absolute positioning with degrees
    println!("Moving to absolute position (45°, 15°)...");
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)?;
    camera.await_pan_tilt_idle(Duration::from_secs(30))?;
    println!("✓ Moved to position");

    // Relative movement
    println!("Moving relative (+10°, +5°)...");
    camera.pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)?;
    camera.await_pan_tilt_idle(Duration::from_secs(30))?;
    println!("✓ Relative movement complete");

    // Absolute zoom positioning
    println!("Setting zoom to 50%...");
    camera.zoom_absolute(Normalized(0.5))?;
    camera.await_zoom_idle(Duration::from_secs(10))?;
    println!("✓ Zoom at 50%");
    println!();

    // === FOCUS CONTROL ===
    println!("═══ Focus Control ═══");

    // Auto focus
    println!("Setting auto focus...");
    camera.focus_auto()?;
    sleep(Duration::from_secs(1));
    println!("✓ Auto focus enabled");

    // Manual focus demonstration
    println!("Testing manual focus...");
    camera.focus_manual()?;
    camera.focus_near(SpeedLevel::Medium)?;
    sleep(Duration::from_millis(300));
    camera.focus_stop()?;
    camera.focus_far(SpeedLevel::Medium)?;
    sleep(Duration::from_millis(300));
    camera.focus_stop()?;
    println!("✓ Manual focus complete");

    // One-push auto focus
    println!("Triggering one-push auto focus...");
    camera.focus_one_push()?;
    camera.await_focus_idle(Duration::from_secs(5))?;
    println!("✓ One-push focus complete");
    camera.focus_auto()?;
    println!();

    // === EXPOSURE & WHITE BALANCE ===
    println!("═══ Exposure & White Balance ═══");

    // Exposure modes
    println!("Testing exposure modes...");
    camera.exposure_auto()?;
    println!("  ✓ Auto exposure");
    camera.exposure_manual()?;
    println!("  ✓ Manual exposure");
    camera.exposure_shutter_priority()?;
    println!("  ✓ Shutter priority");
    camera.exposure_auto()?;

    // White balance modes
    println!("Testing white balance modes...");
    camera.white_balance_auto()?;
    println!("  ✓ Auto white balance");
    camera.white_balance_indoor()?;
    println!("  ✓ Indoor");
    camera.white_balance_outdoor()?;
    println!("  ✓ Outdoor");
    camera.white_balance_one_push()?;
    println!("  ✓ One-push");
    camera.white_balance_auto()?;
    println!();

    // === IMAGE ADJUSTMENTS ===
    println!("═══ Image Adjustments ═══");

    // Flip control
    println!("Testing image flip...");
    let original_flip = camera.get_image_flip()?;
    println!(
        "  Current: V={}, H={}",
        original_flip.vertical, original_flip.horizontal
    );

    if original_flip.vertical {
        camera.disable_flip()?;
        println!("  ✓ Flip disabled");
        sleep(Duration::from_millis(500));
        camera.enable_flip()?;
        println!("  ✓ Flip re-enabled");
    } else {
        camera.enable_flip()?;
        println!("  ✓ Flip enabled");
        sleep(Duration::from_millis(500));
        camera.disable_flip()?;
        println!("  ✓ Flip disabled");
    }
    println!();

    // === PRESET MANAGEMENT ===
    println!("═══ Preset Management ═══");

    // Save presets
    println!("Saving preset positions...");

    // Preset 1: Wide overview
    camera.pan_tilt_absolute(Degrees(0.0), Degrees(0.0), SpeedLevel::Medium)?;
    camera.zoom_absolute(Normalized(0.0))?;
    camera.await_idle(Duration::from_secs(5))?;
    camera.preset_set(PresetNumber::new(1)?)?;
    println!("  ✓ Preset 1 (Wide Overview) saved");

    // Preset 2: Right view
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(-10.0), SpeedLevel::Medium)?;
    camera.zoom_absolute(Normalized(0.3))?;
    camera.await_idle(Duration::from_secs(5))?;
    camera.preset_set(PresetNumber::new(2)?)?;
    println!("  ✓ Preset 2 (Right View) saved");

    // Preset 3: Left view
    camera.pan_tilt_absolute(Degrees(-45.0), Degrees(-10.0), SpeedLevel::Medium)?;
    camera.zoom_absolute(Normalized(0.3))?;
    camera.await_idle(Duration::from_secs(5))?;
    camera.preset_set(PresetNumber::new(3)?)?;
    println!("  ✓ Preset 3 (Left View) saved");

    // Test preset recall
    println!("Testing preset recall...");
    for i in 1..=3 {
        println!("  Recalling Preset {i}...");
        camera.preset_recall(PresetNumber::new(i)?)?;
        camera.await_idle(Duration::from_secs(5))?;
        if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees() {
            println!("    Position: Pan={:.1}°, Tilt={:.1}°", pan.0, tilt.0);
        }
    }
    println!("✓ Preset recall complete");

    // Clear a preset
    println!("Clearing Preset 3...");
    camera.preset_reset(PresetNumber::new(3)?)?;
    println!("✓ Preset 3 cleared");
    println!();

    // === RESTORE INITIAL STATE ===
    println!("═══ Finishing Demo ═══");
    println!("Restoring camera to initial state...");

    match (&initial_position, &initial_zoom) {
        (Ok((pan, tilt)), Ok(zoom)) => {
            camera.pan_tilt_absolute(*pan, *tilt, SpeedLevel::Fast)?;
            camera.await_pan_tilt_idle(Duration::from_secs(30))?;

            let normalized_zoom = (*zoom as f32) / 16384.0;
            camera.zoom_absolute(Normalized(normalized_zoom))?;
            camera.await_zoom_idle(Duration::from_secs(5))?;

            println!("✓ Camera restored to initial state");
        }
        _ => {
            camera.pan_tilt_home()?;
            camera.await_pan_tilt_idle(Duration::from_secs(30))?;
            camera.zoom_absolute(Normalized(0.0))?;
            camera.await_zoom_idle(Duration::from_secs(10))?;
            println!("✓ Camera at home position");
        }
    }

    println!();
    println!("✨ Demo complete!");
    println!();
    println!("Demonstrated features:");
    println!("  ✓ Basic movement (pan/tilt/zoom)");
    println!("  ✓ Advanced positioning (absolute, relative)");
    println!("  ✓ Focus control (auto, manual, one-push)");
    println!("  ✓ Exposure & white balance modes");
    println!("  ✓ Image adjustments (flip)");
    println!("  ✓ Preset management (save, recall, clear)");
    println!();
    println!("Next steps:");
    println!("  - Try quickstart_async for the async version");
    println!("  - Check other examples for specific features");

    Ok(())
}
