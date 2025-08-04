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
    println!("Demonstrating practical preset usage for multi-zone monitoring...");

    // Note: Motion Sync features removed for simplicity
    println!("ℹ Using sequential movements for preset operations");
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

    // Save all preset positions with concurrent operations
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

        // Issue movement commands sequentially
        camera
            .pan_tilt_absolute(config.pan, config.tilt, SpeedLevel::Medium)
            .await?;
        camera.zoom_absolute(config.zoom).await?;

        // Wait for all movements to complete
        println!("  Moving to position...");
        camera
            .wait_for_pan_tilt_completion(Duration::from_secs(30))
            .await?;
        camera
            .wait_for_zoom_completion(Duration::from_secs(10))
            .await?;

        // Verify position with concurrent queries
        let (position_result, zoom_result) =
            tokio::join!(camera.get_pan_tilt_degrees(), camera.get_zoom_position());

        if let (Ok((actual_pan, actual_tilt)), Ok(actual_zoom)) = (position_result, zoom_result) {
            let zoom_percent = (actual_zoom as f32 / 16384.0) * 100.0;
            println!(
                "  Current position: Pan={:.1}°, Tilt={:.1}°, Zoom={:.0}%",
                actual_pan.0, actual_tilt.0, zoom_percent
            );
        }

        // Save as preset - this captures the current complete state
        camera.preset_set(PresetNumber::new(config.number)?).await?;
        println!("  ✓ Preset {} saved", config.number);

        sleep(Duration::from_millis(500)).await;
        println!();
    }

    // Demonstrate preset tour/patrol with async monitoring
    println!("Demonstrating Async Preset Tour (Security Patrol):");
    println!("{}", "─".repeat(50));
    println!("Starting automated security patrol with concurrent monitoring...");
    println!();

    // Simulate concurrent monitoring tasks during patrol
    for cycle in 1..=2 {
        println!("Patrol Cycle {}/2:", cycle);

        for config in &preset_configs {
            println!(
                "  → Moving to Preset {}: {} ({})",
                config.number, config.name, config.description
            );

            // Get position before recall
            let before_pos = camera.get_pan_tilt_degrees().await;

            // Start movement to preset
            let recall_future = camera.preset_recall(PresetNumber::new(config.number)?);

            // While moving, we can perform other async tasks
            let monitoring_task = async {
                sleep(Duration::from_millis(500)).await;
                println!("    [Background] Monitoring system active during movement...");
            };

            // Execute both concurrently
            let (recall_result, _) = tokio::join!(recall_future, monitoring_task);
            recall_result?;

            // Wait for movement to complete
            camera
                .wait_for_all_movements(Duration::from_secs(5))
                .await?;

            // Check current position to verify preset recall worked
            if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees().await {
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

            // Dwell time at each preset
            let dwell_time = if config.number == 2 || config.number == 4 {
                Duration::from_secs(2)
            } else {
                Duration::from_secs(1)
            };

            sleep(dwell_time).await;
        }

        if cycle < 2 {
            println!("  Cycle {} complete, starting next cycle...", cycle);
            println!();
        }
    }

    println!("✓ Async preset tour complete");
    println!();

    // Demonstrate quick preset switching with parallel operations
    println!("Demonstrating Async Quick Response Scenario:");
    println!("{}", "─".repeat(50));
    println!("Simulating incident at entrance - multi-camera coordination...");

    // Simulate coordinated response with multiple async operations
    let camera_clone = camera.clone();

    // Main camera jumps to entrance
    let main_response = async {
        camera.preset_recall(PresetNumber::new(2)?).await?;
        println!("✓ Main camera moved to Entrance preset");
        Ok::<(), Error>(())
    };

    // Simulate notification system (would be actual alert in production)
    let alert_system = async {
        sleep(Duration::from_millis(100)).await;
        println!("✓ Alert system notified");
    };

    // Simulate recording trigger (would start recording in production)
    let recording_system = async {
        sleep(Duration::from_millis(200)).await;
        println!("✓ Recording started for incident");
    };

    // Execute all responses concurrently
    let (main_result, _, _) = tokio::join!(main_response, alert_system, recording_system);
    main_result?;

    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;

    // Enhanced view with concurrent zoom and fine adjustment
    println!("  Enhancing view with concurrent operations...");

    let zoom_enhance = camera_clone.zoom_absolute(Normalized(0.6));
    let position_adjust = async {
        sleep(Duration::from_millis(500)).await;
        camera
            .pan_tilt_relative(Degrees(5.0), Degrees(0.0), SpeedLevel::Medium)
            .await
    };

    let (zoom_result, adjust_result) = tokio::join!(zoom_enhance, position_adjust);
    zoom_result?;
    adjust_result?;

    println!("  ✓ View enhanced with zoom and position adjustment");
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;

    // Save incident view
    println!("  Saving incident view as temporary preset 10...");
    camera.preset_set(PresetNumber::new(10)?).await?;
    println!("✓ Incident view saved for investigation");
    println!();

    // Demonstrate preset speed tour (async advantage)
    println!("Demonstrating High-Speed Preset Scan:");
    println!("{}", "─".repeat(50));
    println!("Rapid scan of all zones using async operations...");

    // Quick scan through all presets
    for i in 1..=3 {
        let preset_name = preset_configs[i - 1].name;
        print!("  Scanning {}: ", preset_name);

        // Move to preset and immediately start next movement
        camera.preset_recall(PresetNumber::new(i as u8)?).await?;

        // Very brief dwell (async allows rapid switching)
        sleep(Duration::from_millis(800)).await;
        println!("✓");
    }

    println!("✓ High-speed scan complete");
    println!();

    // Return to overview and cleanup
    println!("Returning to overview and cleaning up...");

    // Concurrent cleanup operations
    let return_overview = camera.preset_recall(PresetNumber::new(1)?);
    let clear_temp = async {
        sleep(Duration::from_millis(500)).await;
        camera.preset_reset(PresetNumber::new(10)?).await
    };

    let (overview_result, clear_result) = tokio::join!(return_overview, clear_temp);
    overview_result?;
    clear_result?;

    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;
    println!("✓ Returned to overview and cleared temporary preset");
    println!();

    // === CONCURRENT OPERATIONS DEMO ===
    println!("═══ Concurrent Operations (Async Advantage) ═══");
    println!("Demonstrating concurrent pan and zoom...");

    // Move home first
    camera.pan_tilt_home().await?;
    camera.zoom_absolute(Normalized(0.0)).await?;
    camera
        .wait_for_all_movements(Duration::from_secs(5))
        .await?;

    // Start concurrent operations with timeout
    let pan_handle = {
        let camera = camera.clone();
        tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(10), async {
                camera
                    .pan_tilt_absolute(Degrees(90.0), Degrees(30.0), SpeedLevel::Slow)
                    .await
                    .unwrap();
            })
            .await
            .ok();
        })
    };

    let zoom_handle = {
        let camera = camera.clone();
        tokio::spawn(async move {
            tokio::time::timeout(Duration::from_secs(10), async {
                camera.zoom_absolute(Normalized(0.7)).await.unwrap();
            })
            .await
            .ok();
        })
    };

    // Wait for both operations to complete
    let _ = pan_handle.await;
    let _ = zoom_handle.await;

    // Wait for all movements to actually complete
    let _ = tokio::time::timeout(
        Duration::from_secs(15),
        camera.wait_for_all_movements(Duration::from_secs(10)),
    )
    .await;
    println!("✓ Concurrent operations complete");
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
