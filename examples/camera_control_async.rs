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
    camera::profiles::PTZOpticsG2,
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

    // === SAVE INITIAL STATE ===
    println!("═══ Saving Initial Camera State ═══");

    // Give camera a moment to stabilize after connection
    sleep(Duration::from_secs(1)).await;

    // Try to get zoom a few times if it's 0
    let mut initial_zoom = camera.get_zoom_position().await;
    if let Ok(0) = initial_zoom {
        sleep(Duration::from_millis(500)).await;
        initial_zoom = camera.get_zoom_position().await;
    }

    let initial_position = camera.get_pan_tilt_degrees().await;

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
    camera.pan_tilt_home().await?;
    camera
        .wait_for_pan_tilt_completion(Duration::from_secs(30))
        .await?;
    println!("✓ At home position");

    // Absolute positioning with degrees
    println!("Moving to absolute position (45°, 15°)...");
    camera
        .pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)
        .await?;
    camera
        .wait_for_pan_tilt_completion(Duration::from_secs(30))
        .await?;
    println!("✓ Moved to position");

    // Relative movement
    println!("Moving relative (+10°, +5°)...");
    camera
        .pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)
        .await?;
    camera
        .wait_for_pan_tilt_completion(Duration::from_secs(30))
        .await?;
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
    camera
        .wait_for_zoom_completion(Duration::from_secs(10))
        .await?;
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
    camera
        .wait_for_zoom_completion(Duration::from_secs(10))
        .await?;
    println!("✓ Zoom at 50%");
    println!();

    // === FOCUS OPERATIONS ===
    println!("═══ Focus Operations ═══");

    // Auto focus
    println!("Setting auto focus...");
    camera.focus_auto().await?;
    // Wait a moment for auto focus to engage
    sleep(Duration::from_millis(500)).await;
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
    camera
        .wait_for_focus_completion(Duration::from_secs(10))
        .await?;
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

    // Flip control - check current state first
    println!("Testing image flip...");

    // Get current flip status
    let original_flip = camera.get_image_flip().await?;
    println!(
        "  Current flip state - Vertical: {}, Horizontal: {}",
        original_flip.vertical, original_flip.horizontal
    );

    // Test both states, then restore to original
    if original_flip.vertical {
        // Flip is currently ON - test by toggling it off then back on
        camera.disable_flip().await?;
        println!("✓ Image flip disabled (image will be upside down temporarily)");
        sleep(Duration::from_secs(1)).await;
        camera.enable_flip().await?;
        println!("✓ Image flip re-enabled (restored to original correct orientation)");
    } else {
        // Flip is currently OFF - test by toggling it on then back off
        camera.enable_flip().await?;
        println!("✓ Image flip enabled (image will be upside down temporarily)");
        sleep(Duration::from_secs(1)).await;
        camera.disable_flip().await?;
        println!("✓ Image flip disabled (restored to original correct orientation)");
    }
    println!();

    // === PRESET MANAGEMENT ===
    println!("═══ Preset Management ═══");

    // Save three different preset positions
    println!("Saving preset positions...");

    // Preset 1: Wide overview
    println!("  Setting up Preset 1 (Wide Overview)...");
    camera
        .pan_tilt_absolute(Degrees(0.0), Degrees(0.0), SpeedLevel::Medium)
        .await?;
    camera.zoom_absolute(Normalized(0.0)).await?;
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;
    camera.preset_set(PresetNumber::new(1)?).await?;
    println!("  ✓ Preset 1 saved");

    // Preset 2: Zoomed right view
    println!("  Setting up Preset 2 (Right View)...");
    camera
        .pan_tilt_absolute(Degrees(45.0), Degrees(-10.0), SpeedLevel::Medium)
        .await?;
    camera.zoom_absolute(Normalized(0.3)).await?;
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;
    camera.preset_set(PresetNumber::new(2)?).await?;
    println!("  ✓ Preset 2 saved");

    // Preset 3: Zoomed left view
    println!("  Setting up Preset 3 (Left View)...");
    camera
        .pan_tilt_absolute(Degrees(-45.0), Degrees(-10.0), SpeedLevel::Medium)
        .await?;
    camera.zoom_absolute(Normalized(0.3)).await?;
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;
    camera.preset_set(PresetNumber::new(3)?).await?;
    println!("  ✓ Preset 3 saved");
    println!();

    // Demonstrate preset recall
    println!("Testing preset recall...");

    println!("  Recalling Preset 1...");
    camera.preset_recall(PresetNumber::new(1)?).await?;
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;
    if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees().await {
        println!(
            "    Current position: Pan={:.1}°, Tilt={:.1}°",
            pan.0, tilt.0
        );
    }

    println!("  Recalling Preset 2...");
    camera.preset_recall(PresetNumber::new(2)?).await?;
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;
    if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees().await {
        println!(
            "    Current position: Pan={:.1}°, Tilt={:.1}°",
            pan.0, tilt.0
        );
    }

    println!("  Recalling Preset 3...");
    camera.preset_recall(PresetNumber::new(3)?).await?;
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;
    if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees().await {
        println!(
            "    Current position: Pan={:.1}°, Tilt={:.1}°",
            pan.0, tilt.0
        );
    }

    println!("✓ Preset recall complete");
    println!();

    // Clear a preset
    println!("Clearing Preset 3...");
    camera.preset_reset(PresetNumber::new(3)?).await?;
    println!("✓ Preset 3 cleared");
    println!();

    // === CONCURRENT OPERATIONS (ASYNC ADVANTAGE) ===
    println!("═══ Concurrent Operations (Async Advantage) ═══");
    println!("Demonstrating concurrent inquiry operations...");

    // Perform multiple queries concurrently
    let (pan_tilt, zoom, focus_mode, exposure_mode) = tokio::join!(
        camera.get_pan_tilt_degrees(),
        camera.get_zoom_position(),
        camera.get_focus_mode(),
        camera.get_exposure_mode()
    );

    println!("  Concurrent query results:");
    if let Ok((pan, tilt)) = pan_tilt {
        println!("    Position: Pan={:.1}°, Tilt={:.1}°", pan.0, tilt.0);
    }
    if let Ok(zoom) = zoom {
        println!("    Zoom: {}", zoom);
    }
    if let Ok(mode) = focus_mode {
        println!("    Focus: {:?}", mode);
    }
    if let Ok(mode) = exposure_mode {
        println!("    Exposure: {:?}", mode);
    }

    println!("✓ Concurrent operations complete");
    println!("  Note: Async allows multiple operations to run in parallel!");
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

            // Move back to initial pan/tilt first
            println!("  Returning to original position...");
            camera
                .pan_tilt_absolute(*pan, *tilt, SpeedLevel::Fast)
                .await?;

            // Wait for pan/tilt to complete
            camera
                .wait_for_pan_tilt_completion(Duration::from_secs(30))
                .await?;

            // Then restore zoom
            let normalized_zoom = (*zoom as f32) / 16384.0;
            println!(
                "  Restoring zoom: raw={}, normalized={:.3}",
                zoom, normalized_zoom
            );
            camera.zoom_absolute(Normalized(normalized_zoom)).await?;

            // Wait for zoom to complete
            camera
                .wait_for_zoom_completion(Duration::from_secs(10))
                .await?;

            // Verify restoration with concurrent checks
            let (position_result, zoom_result) =
                tokio::join!(camera.get_pan_tilt_degrees(), camera.get_zoom_position());

            if let (Ok((final_pan, final_tilt)), Ok(final_zoom)) = (position_result, zoom_result) {
                println!(
                    "  Final position: Pan={:.1}°, Tilt={:.1}°, Zoom={}",
                    final_pan.0, final_tilt.0, final_zoom
                );
            }

            println!("✓ Camera restored to initial state");
        }
        _ => {
            // Fallback to home if we couldn't save initial state
            println!("  Returning to home position (couldn't save initial state)");

            camera.pan_tilt_home().await?;
            camera
                .wait_for_pan_tilt_completion(Duration::from_secs(30))
                .await?;

            camera.zoom_absolute(Normalized(0.0)).await?;
            camera
                .wait_for_zoom_completion(Duration::from_secs(10))
                .await?;

            println!("✓ Camera at home position");
        }
    }

    println!();
    println!("✨ Camera control demo complete!");
    println!();
    println!("Demonstrated features:");
    println!("  ✓ Pan/Tilt control (absolute, relative, directional)");
    println!("  ✓ Zoom control (standard, variable speed, absolute)");
    println!("  ✓ Focus control (auto, manual, one-push)");
    println!("  ✓ Exposure modes");
    println!("  ✓ White balance modes");
    println!("  ✓ Image adjustments (flip)");
    println!("  ✓ Preset management (save, recall, clear)");
    println!("  ✓ Concurrent operations (async advantage!)");
    println!();
    println!("See camera_control for the blocking version of this demo.");

    Ok(())
}
