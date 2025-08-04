//! Quickstart example - minimal VISCA camera control.
//!
//! This example demonstrates the simplest way to connect to and control a PTZ camera
//! using the blocking (synchronous) API. It covers:
//! - Connecting to a camera
//! - Basic movement commands (pan, tilt, zoom)
//! - Saving and recalling presets
//!
//! Run with:
//! ```sh
//! cargo run --example quickstart [camera_ip[:port]]
//! ```
//!
//! If no address is provided, defaults to 192.168.0.110 (PTZOptics test camera).
//! If no port is provided, the builder automatically selects the correct default
//! based on the camera profile and transport type.

use grafton_visca::{
    prelude::blocking::*,
    types::{PanSpeed, TiltSpeed},
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

    println!("🎥 Connecting to camera at {camera_addr}");

    // Create camera using the builder pattern
    // The builder automatically adds the correct default port if not specified:
    // - TCP: 5678 for PTZOptics, 52381 for Sony
    // - UDP: 1259 for PTZOptics, 52381 for Sony
    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()?;

    println!("✅ Connected successfully!");
    println!();

    // Save initial camera state
    let initial_state = camera.save_state()?;
    println!(
        "💾 Saved initial state: Pan={:.1}°, Tilt={:.1}°, Zoom={}",
        initial_state.pan, initial_state.tilt, initial_state.zoom
    );

    // Power on the camera
    println!("📍 Powering on camera...");
    camera.power_on()?;
    sleep(Duration::from_secs(2)); // Wait for camera to initialize - can't use wait helpers during power-on

    // Move to home position
    println!("🏠 Moving to home position...");
    camera.pan_tilt_home()?;
    camera.wait_for_pan_tilt_completion(Duration::from_secs(10))?; // Wait for movement to complete

    // Demonstrate zoom control
    println!("🔍 Testing zoom...");
    println!("   Zooming in...");
    camera.zoom_in()?;
    sleep(Duration::from_secs(2));

    println!("   Stopping zoom...");
    camera.zoom_stop()?;
    sleep(Duration::from_millis(500));

    println!("   Zooming out...");
    camera.zoom_out()?;
    sleep(Duration::from_secs(2));

    println!("   Stopping zoom...");
    camera.zoom_stop()?;

    // Demonstrate pan/tilt control
    println!("🔄 Testing pan/tilt...");
    println!("   Panning right...");
    camera.pan_tilt_move(
        PanTiltDirection::Right,
        PanSpeed::new(10)?,
        TiltSpeed::new(0)?,
    )?;
    sleep(Duration::from_secs(1));

    println!("   Stopping movement...");
    camera.pan_tilt_stop()?;
    sleep(Duration::from_millis(500));

    println!("   Tilting up...");
    camera.pan_tilt_move(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)?;
    sleep(Duration::from_secs(1));

    println!("   Stopping movement...");
    camera.pan_tilt_stop()?;

    // Return to home
    println!("🏠 Returning to home position...");
    camera.pan_tilt_home()?;
    camera.wait_for_pan_tilt_completion(Duration::from_secs(10))?;

    // Demonstrate presets
    println!("💾 Testing presets...");
    println!("   Saving current position as preset 1...");
    camera.preset_set(PresetNumber::new(1)?)?;
    sleep(Duration::from_millis(500));

    // Move away from saved position
    println!("   Moving to a different position...");
    camera.pan_tilt_move(
        PanTiltDirection::Left,
        PanSpeed::new(10)?,
        TiltSpeed::new(0)?,
    )?;
    sleep(Duration::from_secs(2));
    camera.pan_tilt_stop()?;

    // Recall the preset
    println!("   Recalling preset 1...");
    camera.preset_recall(PresetNumber::new(1)?)?;
    camera.wait_for_all_movements(Duration::from_secs(5))?;

    // Restore initial camera state
    println!("🔄 Restoring initial camera state...");
    camera.restore_state(&initial_state)?;
    println!("✅ Camera restored to initial state");

    println!();
    println!("✨ Quickstart complete!");
    println!();
    println!("Next steps:");
    println!("  - Try the quickstart_async example for async/await support");
    println!("  - See camera_control for comprehensive camera operations");
    println!("  - Check transports example for TCP/UDP options");

    Ok(())
}
