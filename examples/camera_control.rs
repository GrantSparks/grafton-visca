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
            println!("✓ Saved initial zoom: {}", zoom);
        }
        _ => {
            println!("⚠ Could not save initial position/zoom, will return to home at end");
        }
    }
    println!();

    // === PAN/TILT OPERATIONS ===
    println!("═══ Pan/Tilt Operations ═══");

    // Home position
    println!("Moving to home position...");
    camera.pan_tilt_home()?;
    camera.wait_for_pan_tilt_completion(Duration::from_secs(30))?;
    println!("✓ At home position");

    // Absolute positioning with degrees
    println!("Moving to absolute position (45°, 15°)...");
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)?;
    camera.wait_for_pan_tilt_completion(Duration::from_secs(30))?;
    println!("✓ Moved to position");

    // Relative movement
    println!("Moving relative (+10°, +5°)...");
    camera.pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)?;
    camera.wait_for_pan_tilt_completion(Duration::from_secs(30))?;
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
    camera.wait_for_zoom_completion(Duration::from_secs(10))?;
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
    camera.wait_for_zoom_completion(Duration::from_secs(10))?;
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
    camera.wait_for_focus_completion(Duration::from_secs(5))?;
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

    // Flip control - check current state first
    println!("Testing image flip...");

    // Get current flip status
    let original_flip = camera.get_image_flip()?;
    println!(
        "  Current flip state - Vertical: {}, Horizontal: {}",
        original_flip.vertical, original_flip.horizontal
    );

    // Test both states, then restore to original
    if original_flip.vertical {
        // Flip is currently ON - test by toggling it off then back on
        camera.disable_flip()?;
        println!("✓ Image flip disabled (image will be upside down temporarily)");
        sleep(Duration::from_secs(1));
        camera.enable_flip()?;
        println!("✓ Image flip re-enabled (restored to original correct orientation)");
    } else {
        // Flip is currently OFF - test by toggling it on then back off
        camera.enable_flip()?;
        println!("✓ Image flip enabled (image will be upside down temporarily)");
        sleep(Duration::from_secs(1));
        camera.disable_flip()?;
        println!("✓ Image flip disabled (restored to original correct orientation)");
    }
    println!();

    // === PRESET MANAGEMENT ===
    println!("═══ Preset Management ═══");
    println!("Demonstrating practical preset usage for multi-zone monitoring...");
    println!();

    // Define preset configurations for different monitoring zones
    struct PresetConfig {
        number: u8,
        name: &'static str,
        pan: Degrees,
        tilt: Degrees,
        zoom: Normalized,
        description: &'static str,
    }

    let preset_configs = [
        PresetConfig {
            number: 1,
            name: "Overview",
            pan: Degrees(0.0),
            tilt: Degrees(0.0),
            zoom: Normalized(0.0),
            description: "Wide angle overview of the entire area",
        },
        PresetConfig {
            number: 2,
            name: "Right View",
            pan: Degrees(30.0),
            tilt: Degrees(-5.0),
            zoom: Normalized(0.3),
            description: "View to the right side",
        },
        PresetConfig {
            number: 3,
            name: "Left View",
            pan: Degrees(-30.0),
            tilt: Degrees(0.0),
            zoom: Normalized(0.4),
            description: "View to the left side",
        },
    ];

    // Save all preset positions
    println!("Setting up monitoring presets:");
    println!("{}", "─".repeat(50));

    for config in &preset_configs {
        println!("Configuring Preset {} - '{}'", config.number, config.name);
        println!("  {}", config.description);
        println!(
            "  Target: Pan={:.1}°, Tilt={:.1}°, Zoom={:.0}%",
            config.pan.0,
            config.tilt.0,
            config.zoom.0 * 100.0
        );

        // Move to position - pan/tilt first, then zoom
        camera.pan_tilt_absolute(config.pan, config.tilt, SpeedLevel::Medium)?;
        camera.wait_for_pan_tilt_completion(Duration::from_secs(30))?;

        camera.zoom_absolute(config.zoom)?;

        // Wait for zoom to complete
        println!("  Waiting for movements to complete...");
        camera.wait_for_pan_tilt_completion(Duration::from_secs(30))?;
        camera.wait_for_zoom_completion(Duration::from_secs(10))?;

        // Verify we're at the correct position before saving
        if let Ok((actual_pan, actual_tilt)) = camera.get_pan_tilt_degrees() {
            if let Ok(actual_zoom) = camera.get_zoom_position() {
                let zoom_percent = (actual_zoom as f32 / 16384.0) * 100.0;
                println!(
                    "  Current position: Pan={:.1}°, Tilt={:.1}°, Zoom={:.0}%",
                    actual_pan.0, actual_tilt.0, zoom_percent
                );
            }
        }

        // Save as preset - this captures the current complete state
        camera.preset_set(PresetNumber::new(config.number)?)?;
        println!("  ✓ Preset {} saved", config.number);

        sleep(Duration::from_millis(500));
        println!();
    }

    // Demonstrate preset tour/patrol
    println!("Demonstrating Preset Tour (Security Patrol):");
    println!("{}", "─".repeat(50));
    println!("Starting automated security patrol sequence...");
    println!();

    // Perform two cycles of the tour
    for cycle in 1..=2 {
        println!("Patrol Cycle {}/2:", cycle);

        for config in &preset_configs {
            println!(
                "  → Moving to Preset {}: {} ({})",
                config.number, config.name, config.description
            );

            // Get position before recall
            let before_pos = camera.get_pan_tilt_degrees();

            // Recall the preset
            camera.preset_recall(PresetNumber::new(config.number)?)?;

            // Wait for movement to complete
            camera.wait_for_all_movements(Duration::from_secs(5))?;

            // Check current position to verify preset recall worked
            if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees() {
                if let Ok((before_pan, before_tilt)) = before_pos {
                    if (before_pan.0 - pan.0).abs() > 0.5 || (before_tilt.0 - tilt.0).abs() > 0.5 {
                        println!(
                            "    ✓ Moved from ({:.1}°, {:.1}°) to ({:.1}°, {:.1}°)",
                            before_pan.0, before_tilt.0, pan.0, tilt.0
                        );
                    } else {
                        println!(
                            "    Already at position: Pan={:.1}°, Tilt={:.1}°",
                            pan.0, tilt.0
                        );
                    }
                } else {
                    println!(
                        "    Current position: Pan={:.1}°, Tilt={:.1}°",
                        pan.0, tilt.0
                    );
                }
            }

            // Dwell time at each preset (simulating monitoring time)
            let dwell_time = if config.number == 2 || config.number == 4 {
                // Longer dwell on critical areas (entrance and emergency exit)
                Duration::from_secs(2)
            } else {
                Duration::from_secs(1)
            };

            sleep(dwell_time);
        }

        if cycle < 2 {
            println!("  Cycle {} complete, starting next cycle...", cycle);
            println!();
        }
    }

    println!("✓ Preset tour complete");
    println!();

    // Demonstrate quick preset switching for incident response
    println!("Demonstrating Quick Response Scenario:");
    println!("{}", "─".repeat(50));
    println!("Simulating incident at entrance - quick response...");

    // Quick jump to entrance
    camera.preset_recall(PresetNumber::new(2)?)?;
    camera.wait_for_all_movements(Duration::from_secs(5))?;
    println!("✓ Immediately moved to Entrance preset");

    // Zoom in further for detail
    println!("  Zooming in for more detail...");
    camera.zoom_absolute(Normalized(0.6))?;
    camera.wait_for_zoom_completion(Duration::from_secs(5))?;

    // Pan slightly to track subject
    println!("  Adjusting view to track subject...");
    camera.pan_tilt_relative(Degrees(5.0), Degrees(0.0), SpeedLevel::Medium)?;
    camera.wait_for_pan_tilt_completion(Duration::from_secs(5))?;

    // Save this as a temporary preset for later review
    println!("  Saving current view as temporary preset 10...");
    camera.preset_set(PresetNumber::new(10)?)?;
    println!("✓ Incident view saved for later review");
    println!();

    // Return to overview
    println!("Returning to overview position...");
    camera.preset_recall(PresetNumber::new(1)?)?;
    camera.wait_for_all_movements(Duration::from_secs(5))?;
    println!("✓ Back to overview monitoring");
    println!();

    // Clear temporary preset
    println!("Clearing temporary preset 10...");
    camera.preset_reset(PresetNumber::new(10)?)?;
    println!("✓ Temporary preset cleared");
    println!();

    // === RESTORE INITIAL STATE ===
    println!("═══ Finishing Demo ═══");
    println!("Restoring camera to initial state...");

    // Restore to initial position and zoom if we saved them
    match (&initial_position, &initial_zoom) {
        (Ok((pan, tilt)), Ok(zoom)) => {
            println!(
                "  Initial state was: Pan={:.1}°, Tilt={:.1}°, Zoom={}",
                pan.0, tilt.0, zoom
            );

            // Move back to initial pan/tilt
            println!("  Returning to original position...");
            camera.pan_tilt_absolute(*pan, *tilt, SpeedLevel::Fast)?;

            // Wait for pan/tilt to complete
            camera.wait_for_pan_tilt_completion(Duration::from_secs(30))?;

            // Then restore zoom
            let normalized_zoom = (*zoom as f32) / 16384.0;
            camera.zoom_absolute(Normalized(normalized_zoom))?;

            // Wait for zoom to complete
            camera.wait_for_zoom_completion(Duration::from_secs(5))?;

            // Verify restoration
            if let Ok((final_pan, final_tilt)) = camera.get_pan_tilt_degrees() {
                if let Ok(final_zoom) = camera.get_zoom_position() {
                    println!(
                        "  Final position: Pan={:.1}°, Tilt={:.1}°, Zoom={}",
                        final_pan.0, final_tilt.0, final_zoom
                    );
                }
            }

            println!("✓ Camera restored to initial state");
        }
        _ => {
            // Fallback to home if we couldn't save initial state
            println!("  Returning to home position (couldn't save initial state)");
            camera.pan_tilt_home()?;
            camera.wait_for_pan_tilt_completion(Duration::from_secs(30))?;
            camera.zoom_absolute(Normalized(0.0))?;
            camera.wait_for_zoom_completion(Duration::from_secs(10))?;
            println!("✓ Camera at home position");
        }
    }

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
