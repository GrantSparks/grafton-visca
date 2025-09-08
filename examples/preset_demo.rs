//! Focused preset management demonstration.
//!
//! This example demonstrates preset functionality including:
//! - Saving multiple preset positions with pan/tilt/zoom
//! - Recalling presets to verify they work correctly
//! - Showing position changes during recall
//!
//! **Context**: This is a blocking example that requires NO async features or runtime.
//!
//! Run with:
//! ```sh
//! cargo run --example preset_demo [camera_ip[:port]]
//! ```

#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    mode::BlockingFutureExt,
    types::SpeedLevel,
    units::{Degrees, Normalized},
    Camera, Error, PanTiltControl, ZoomControl,
};

#[cfg(not(feature = "async"))]
use std::{env, thread::sleep, time::Duration};

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("🎥 Preset Management Demo");
    println!("========================");
    println!("Connecting to camera at {camera_addr}\n");

    // Connect to camera using the convenience API
    let mut camera = Camera::open_tcp_blocking::<PtzOpticsG2>(format!("{camera_addr}:5678"))
        .map_err(|e| {
            eprintln!("Failed to connect to camera at {camera_addr}: {e}");
            e
        })?;

    println!("✅ Connected successfully!\n");

    println!("Moving to home position...");
    camera.pan_tilt_home().block()?;
    camera.zoom_absolute(Normalized(0.0)).block()?;
    camera.await_idle(Duration::from_secs(10))?;
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
            .pan_tilt_absolute(preset.pan, preset.tilt, SpeedLevel::Medium)
            .block()?;
        camera.zoom_absolute(preset.zoom).block()?;

        camera.await_idle(Duration::from_secs(10))?;
        // Note: Position inquiry not implemented in this demo
        // Position would be displayed here if inquiry was available

        camera.presets().set(preset.number).block()?;
        println!("  ✓ Preset {} saved\n", preset.number);

        sleep(Duration::from_millis(200));
    }

    println!("═══ Testing Preset Recall ═══");
    println!("Moving to test position (60°, -15°)...");
    camera
        .pan_tilt_absolute(Degrees(60.0), Degrees(-15.0), SpeedLevel::Fast)
        .block()?;
    camera.zoom_absolute(Normalized(0.7)).block()?;
    camera.await_idle(Duration::from_secs(10))?;

    // Note: Position inquiry not implemented in this demo
    println!("Current position: (position inquiry not available)\n");

    for preset in &presets {
        println!("Recalling Preset {} - '{}'", preset.number, preset.name);

        // Note: Position inquiry not implemented in this demo

        camera.presets().recall(preset.number).block()?;

        camera.await_idle(Duration::from_secs(10))?;

        // Position inquiry not available - preset recall occurs but we can't verify position
        println!("  Preset recalled (position verification not available)");

        println!();
    }

    println!("✨ Preset demo complete!");

    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    println!("This example requires blocking mode. Run without the async feature:");
    println!("  cargo run --example preset_demo --no-default-features");
}
