//! Focused preset management demonstration.
//!
//! This example demonstrates preset functionality including:
//! - Saving multiple preset positions with pan/tilt/zoom
//! - Recalling presets to verify they work correctly
//! - Showing position changes during recall
//!
//! Run with:
//! ```sh
//! cargo run --example preset_demo [camera_ip[:port]]
//! ```

use grafton_visca::{prelude::blocking::*, CameraBuilder, Error};

use std::{env, thread::sleep, time::Duration};

fn main() -> Result<(), Error> {
    env_logger::init();

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Preset Management Demo");
    println!("========================");
    println!("Connecting to camera at {camera_addr}\n");

    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()?;

    println!("✅ Connected successfully!\n");

    // Save initial state
    let initial_state = camera.save_state()?;

    // Start from home position
    println!("Moving to home position...");
    camera.pan_tilt_home()?;
    camera.zoom_absolute(Normalized(0.0))?;
    camera.await_idle(Duration::from_secs(10))?;
    println!("✓ At home position\n");

    // Define test presets
    struct PresetTest {
        number: u8,
        name: &'static str,
        pan: Degrees,
        tilt: Degrees,
        zoom: Normalized,
    }

    let presets = [
        PresetTest {
            number: 1,
            name: "Wide Overview",
            pan: Degrees(0.0),
            tilt: Degrees(0.0),
            zoom: Normalized(0.0),
        },
        PresetTest {
            number: 2,
            name: "Left Corner",
            pan: Degrees(-45.0),
            tilt: Degrees(10.0),
            zoom: Normalized(0.3),
        },
        PresetTest {
            number: 3,
            name: "Right Corner",
            pan: Degrees(45.0),
            tilt: Degrees(-5.0),
            zoom: Normalized(0.4),
        },
    ];

    // Save presets
    println!("═══ Saving Presets ═══");
    for preset in &presets {
        println!("Setting up Preset {} - '{}'", preset.number, preset.name);
        println!(
            "  Moving to: Pan={:.1}°, Tilt={:.1}°, Zoom={:.0}%",
            preset.pan.0,
            preset.tilt.0,
            preset.zoom.0 * 100.0
        );

        // Move to position
        camera.pan_tilt_absolute(preset.pan, preset.tilt, SpeedLevel::Medium)?;
        camera.zoom_absolute(preset.zoom)?;

        // Wait for movement
        camera.await_idle(Duration::from_secs(10))?;

        // Verify position
        if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees() {
            println!("  At position: Pan={:.1}°, Tilt={:.1}°", pan.0, tilt.0);
        }

        // Save preset
        camera.preset_set(PresetNumber::new(preset.number)?)?;
        println!("  ✓ Preset {} saved\n", preset.number);

        // Brief pause before next preset
        sleep(Duration::from_millis(200));
    }

    // Test preset recall
    println!("═══ Testing Preset Recall ═══");

    // Go to a different position first
    println!("Moving to test position (60°, -15°)...");
    camera.pan_tilt_absolute(Degrees(60.0), Degrees(-15.0), SpeedLevel::Fast)?;
    camera.zoom_absolute(Normalized(0.7))?;
    camera.await_idle(Duration::from_secs(10))?;

    if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees() {
        println!("Current position: Pan={:.1}°, Tilt={:.1}°\n", pan.0, tilt.0);
    }

    // Now recall each preset
    for preset in &presets {
        println!("Recalling Preset {} - '{}'", preset.number, preset.name);

        // Get position before
        let before = camera.get_pan_tilt_degrees();
        let before_zoom = camera.get_zoom_position();

        // Recall preset
        camera.preset_recall(PresetNumber::new(preset.number)?)?;

        // Wait for movement
        camera.await_idle(Duration::from_secs(10))?;

        // Get position after
        let after = camera.get_pan_tilt_degrees();
        let after_zoom = camera.get_zoom_position();

        // Show movement
        if let (Ok((before_pan, before_tilt)), Ok((after_pan, after_tilt))) = (before, after) {
            println!(
                "  Pan: {:.1}° → {:.1}° (expected {:.1}°)",
                before_pan.0, after_pan.0, preset.pan.0
            );
            println!(
                "  Tilt: {:.1}° → {:.1}° (expected {:.1}°)",
                before_tilt.0, after_tilt.0, preset.tilt.0
            );

            // Check if we reached the expected position
            let pan_diff = (after_pan.0 - preset.pan.0).abs();
            let tilt_diff = (after_tilt.0 - preset.tilt.0).abs();

            if pan_diff < 1.0 && tilt_diff < 1.0 {
                println!("  ✓ Preset recalled successfully!");
            } else {
                println!(
                    "  ⚠ Position differs from expected (Pan diff: {pan_diff:.1}°, Tilt diff: {tilt_diff:.1}°)"
                );
            }
        }

        if let (Ok(before_z), Ok(after_z)) = (before_zoom, after_zoom) {
            let expected_zoom = (preset.zoom.0 * 16384.0) as u16;
            println!("  Zoom: {before_z} → {after_z} (expected ~{expected_zoom})");
        }

        println!();
    }

    // Restore initial state
    println!("Restoring initial camera state...");
    camera.restore_state(&initial_state)?;

    println!("✨ Preset demo complete!");

    Ok(())
}
