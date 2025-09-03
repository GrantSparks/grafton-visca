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
    camera::{profiles::PtzOpticsG2, Camera},
    mode::{Blocking, BlockingFutureExt},
    transport::builder::TransportBuilder,
    types::SpeedLevel,
    units::{Degrees, Normalized},
    Error, InquiryControl, PanTiltControl, PanTiltInquiryControl, PresetNumber, PresetsControl,
    ZoomControl,
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

    // Create blocking transport using TransportBuilder (no async runtime needed)
    let transport = TransportBuilder::tcp()
        .address(format!("{camera_addr}:5678"))
        .connect_timeout(Duration::from_secs(5))
        .build() // .build() for pure blocking transport
        .map_err(|e| {
            eprintln!("Failed to connect to camera at {camera_addr}: {e}");
            e
        })?;

    // Build camera using the builder pattern for clarity and extensibility
    let mut camera = Camera::<Blocking, PtzOpticsG2, _>::new_blocking(transport)?;

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
        if let Ok(pos) = camera.get_pan_tilt_position().block() {
            println!(
                "  At position: Pan={:.1}°, Tilt={:.1}°",
                pos.pan as f32 / 614.4,
                pos.tilt as f32 / 614.4
            );
        }

        camera
            .preset_set(PresetNumber::new(preset.number)?)
            .block()?;
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

    if let Ok(pos) = camera.get_pan_tilt_position().block() {
        println!(
            "Current position: Pan={:.1}°, Tilt={:.1}°\n",
            pos.pan as f32 / 614.4,
            pos.tilt as f32 / 614.4
        );
    }

    for preset in &presets {
        println!("Recalling Preset {} - '{}'", preset.number, preset.name);

        let before = camera.get_pan_tilt_position().block();
        let before_zoom = camera.get_zoom_position().block();

        camera
            .preset_recall(PresetNumber::new(preset.number)?)
            .block()?;

        camera.await_idle(Duration::from_secs(10))?;

        let after = camera.get_pan_tilt_position().block();
        let after_zoom = camera.get_zoom_position().block();
        if let (Ok(before_pos), Ok(after_pos)) = (before, after) {
            let before_pan_deg = before_pos.pan as f32 / 614.4;
            let before_tilt_deg = before_pos.tilt as f32 / 614.4;
            let after_pan_deg = after_pos.pan as f32 / 614.4;
            let after_tilt_deg = after_pos.tilt as f32 / 614.4;

            println!(
                "  Pan: {:.1}° → {:.1}° (expected {:.1}°)",
                before_pan_deg, after_pan_deg, preset.pan.0
            );
            println!(
                "  Tilt: {:.1}° → {:.1}° (expected {:.1}°)",
                before_tilt_deg, after_tilt_deg, preset.tilt.0
            );

            let pan_diff = (after_pan_deg - preset.pan.0).abs();
            let tilt_diff = (after_tilt_deg - preset.tilt.0).abs();

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

    println!("✨ Preset demo complete!");

    Ok(())
}

#[cfg(feature = "async")]
fn main() {
    println!("This example requires blocking mode. Run without the async feature:");
    println!("  cargo run --example preset_demo --no-default-features");
}
