//! Focused preset management demonstration.
//!
//! This example demonstrates preset functionality including:
//! - Saving multiple preset positions with pan/tilt/zoom
//! - Recalling presets to verify they work correctly
//! - Using axis-specific movement detection for efficient waiting
//! - Proper timeout configuration for different operation types
//!
//! **Context**: This is a blocking example that requires NO async features or runtime.
//!
//! Run with:
//! ```sh
//! cargo run --example preset_demo [camera_ip[:port]]
//! ```

#[cfg(not(feature = "mode-async"))]
use std::{env, thread::sleep, time::Duration};

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{
    camera::{profiles::PtzOpticsG2, AwaitConfig, Axes, Connect},
    types::SpeedLevel,
    units::{Degrees, Normalized},
    Error,
};

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Preset Management Demo");
    println!("========================");
    println!("Connecting to camera at {camera_addr}\n");

    // Connect to camera using the convenience API
    let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&camera_addr).map_err(|e| {
        eprintln!("Failed to connect to camera at {camera_addr}: {e}");
        e
    })?;

    println!("✅ Connected successfully!\n");

    println!("Moving to home position...");
    camera.pan_tilt().home()?;
    camera.zoom().set_position(Normalized(0.0))?;
    // Only wait for pan/tilt and zoom - we set both, no focus change
    camera.await_axes_idle(Axes::PAN_TILT | Axes::ZOOM, Duration::from_secs(30))?;
    println!("✓ At home position\n");

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

    println!("═══ Saving Presets ═══");
    for preset in &presets {
        println!("Setting up Preset {} - '{}'", preset.number, preset.name);
        println!(
            "  Moving to: Pan={:.1}°, Tilt={:.1}°, Zoom={:.0}%",
            preset.pan.0,
            preset.tilt.0,
            preset.zoom.0 * 100.0
        );

        camera
            .pan_tilt()
            .absolute(preset.pan, preset.tilt, SpeedLevel::Medium)?;
        camera.zoom().set_position(preset.zoom)?;

        // Wait for pan/tilt and zoom to complete - 20s is generous for medium speed
        camera.await_axes_idle(Axes::PAN_TILT | Axes::ZOOM, Duration::from_secs(20))?;

        camera.presets().set(preset.number)?;
        println!("  ✓ Preset {} saved\n", preset.number);

        sleep(Duration::from_millis(200));
    }

    println!("═══ Testing Preset Recall ═══");
    println!("Moving to test position (60°, -15°)...");
    camera
        .pan_tilt()
        .absolute(Degrees(60.0), Degrees(-15.0), SpeedLevel::Fast)?;
    camera.zoom().set_position(Normalized(0.7))?;
    // Fast speed but still needs time for the full range of motion
    camera.await_axes_idle(Axes::PAN_TILT | Axes::ZOOM, Duration::from_secs(15))?;
    println!("At test position\n");

    for preset in &presets {
        println!("Recalling Preset {} - '{}'", preset.number, preset.name);

        camera.presets().recall(preset.number)?;

        // Use the preset-specific config: 60s timeout, all axes monitored
        // Presets can move pan/tilt, zoom, and focus simultaneously
        camera.await_with_config(&AwaitConfig::for_preset_recall())?;

        println!(
            "  ✓ Recalled to Pan={:.1}°, Tilt={:.1}°, Zoom={:.0}%\n",
            preset.pan.0,
            preset.tilt.0,
            preset.zoom.0 * 100.0
        );
    }

    println!("✨ Preset demo complete!");

    Ok(())
}

#[cfg(feature = "mode-async")]
fn main() {
    println!("This example requires blocking mode. Run without the async feature:");
    println!("  cargo run --example preset_demo --no-default-features");
}
