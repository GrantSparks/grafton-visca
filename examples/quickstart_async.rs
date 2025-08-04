//! Comprehensive async camera control example using tokio.
//!
//! This example demonstrates the full range of camera control operations available
//! in the async API, including:
//! - Connection and power management
//! - Pan/Tilt/Zoom (PTZ) operations
//! - Focus control
//! - Exposure settings
//! - White balance
//! - Image adjustments
//! - Presets management
//! - Concurrent operations (async advantage!)
//!
//! Run with:
//! ```sh
//! cargo run --example quickstart_async --features tokio [camera_ip[:port]]
//! ```

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, MovementDetectionConfig},
    prelude::r#async::*,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    units::*,
    CameraBuilder, Error, PanTiltDirection,
};
#[cfg(feature = "tokio")]
use std::env;
#[cfg(feature = "tokio")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "tokio")]
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

    // Create camera using the async builder pattern
    let camera = CameraBuilder::tokio_tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()
        .await?;

    println!("✅ Connected successfully!");
    println!();

    // === SAVE INITIAL STATE ===
    println!("═══ Saving Initial Camera State ═══");

    let initial_position = camera.get_pan_tilt_degrees().await;
    let initial_zoom = camera.get_zoom_position().await;

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

    // Home position with new concise API
    println!("Moving to home position...");
    camera.pan_tilt_home().await?;
    // Using the new concise method name (was wait_for_pan_tilt_completion)
    camera.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    println!("✓ At home position");

    // Demonstrate zoom control
    println!("Testing zoom...");
    println!("  Zooming in briefly...");
    camera.zoom_in().await?;
    // Brief delay to let zoom start moving
    sleep(Duration::from_millis(100)).await;
    camera.zoom_stop().await?;
    camera.await_zoom_idle(Duration::from_secs(5)).await?;

    println!("  Zooming out briefly...");
    camera.zoom_out().await?;
    // Brief delay to let zoom start moving
    sleep(Duration::from_millis(100)).await;
    camera.zoom_stop().await?;
    camera.await_zoom_idle(Duration::from_secs(5)).await?;
    println!("✓ Zoom complete");

    // Demonstrate pan/tilt control
    println!("Testing pan/tilt...");
    println!("  Panning right briefly...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Right,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await?;
    // Brief delay to let movement start
    sleep(Duration::from_millis(100)).await;
    camera.pan_tilt_stop().await?;
    camera.await_pan_tilt_idle(Duration::from_secs(5)).await?;

    println!("  Tilting up briefly...");
    camera
        .pan_tilt_move(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)
        .await?;
    // Brief delay to let movement start
    sleep(Duration::from_millis(100)).await;
    camera.pan_tilt_stop().await?;
    camera.await_pan_tilt_idle(Duration::from_secs(5)).await?;
    println!("✓ Pan/tilt complete");
    println!();

    // === ADVANCED POSITIONING ===
    println!("═══ Advanced Positioning ═══");

    // Absolute positioning with custom movement detection
    println!("Moving to absolute position (45°, 15°) with custom detection...");
    camera
        .pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)
        .await?;

    // Demonstrate custom movement detection configuration
    let custom_config = MovementDetectionConfig {
        timeout: Duration::from_secs(30),
        tolerance_pan_stable: 1, // More precise detection (default is 2)
        tolerance_tilt_stable: 1,
        stability_threshold: 5, // Require more stable readings (default is 3)
        debug: true,            // Enable debug logging for this movement
        ..Default::default()
    };
    camera
        .await_pan_tilt_idle_with_config(&custom_config)
        .await?;
    println!("✓ Moved to position with high precision");

    // Relative movement with simplified API
    println!("Moving relative (+10°, +5°)...");
    camera
        .pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)
        .await?;
    camera.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    println!("✓ Relative movement complete");

    // Absolute zoom positioning with new API
    println!("Setting zoom to 50%...");
    camera.zoom_absolute(Normalized(0.5)).await?;
    camera.await_zoom_idle(Duration::from_secs(10)).await?;
    println!("✓ Zoom at 50%");
    println!();

    // === DEMONSTRATE NEW MOVEMENT DETECTION ===
    println!("═══ Advanced Movement Detection ═══");

    // Move to position and wait - new concise API
    println!("Using move_to helper (combines movement + wait)...");
    camera
        .move_to(Degrees(-30.0), Degrees(10.0), Duration::from_secs(30))
        .await?;
    println!("✓ move_to completed");

    // Wait for all movements to complete (pan/tilt/zoom/focus)
    println!("Initiating multiple movements...");
    camera
        .pan_tilt_absolute(Degrees(0.0), Degrees(0.0), SpeedLevel::Fast)
        .await?;
    camera.zoom_absolute(Normalized(0.3)).await?;

    println!("Waiting for all movements to complete...");
    camera.await_idle(Duration::from_secs(30)).await?; // Waits for pan/tilt/zoom/focus simultaneously
    println!("✓ All movements completed");

    // Demonstrate async advantage: concurrent operations
    println!("\nDemonstrating async concurrent operations...");
    use tokio::join;

    // Start multiple movements simultaneously
    let pan_tilt = camera.pan_tilt_absolute(Degrees(20.0), Degrees(-5.0), SpeedLevel::Medium);
    let zoom = camera.zoom_absolute(Normalized(0.6));

    // Execute them concurrently
    let (pt_result, z_result) = join!(pan_tilt, zoom);
    pt_result?;
    z_result?;

    // Wait for all to complete
    camera.await_idle(Duration::from_secs(30)).await?;
    println!("✓ Concurrent operations completed");
    println!();

    // === FOCUS CONTROL ===
    println!("═══ Focus Control ═══");

    // Auto focus
    println!("Setting auto focus...");
    camera.focus_auto().await?;
    // Auto focus is a mode change, not a movement - no wait needed
    println!("✓ Auto focus enabled");

    // Manual focus demonstration
    println!("Testing manual focus...");
    camera.focus_manual().await?;
    camera.focus_near(SpeedLevel::Medium).await?;
    // Brief delay to let focus start moving
    sleep(Duration::from_millis(100)).await;
    camera.focus_stop().await?;
    camera.await_focus_idle(Duration::from_secs(5)).await?;

    camera.focus_far(SpeedLevel::Medium).await?;
    // Brief delay to let focus start moving
    sleep(Duration::from_millis(100)).await;
    camera.focus_stop().await?;
    camera.await_focus_idle(Duration::from_secs(5)).await?;
    println!("✓ Manual focus complete");

    // One-push auto focus with new API
    println!("Triggering one-push auto focus...");
    camera.focus_one_push().await?;
    camera.await_focus_idle(Duration::from_secs(10)).await?;
    println!("✓ One-push focus complete");
    camera.focus_auto().await?;
    println!();

    // === EXPOSURE & WHITE BALANCE ===
    println!("═══ Exposure & White Balance ═══");

    // Exposure modes
    println!("Testing exposure modes...");
    camera.exposure_auto().await?;
    println!("  ✓ Auto exposure");
    camera.exposure_manual().await?;
    println!("  ✓ Manual exposure");
    camera.exposure_shutter_priority().await?;
    println!("  ✓ Shutter priority");
    camera.exposure_auto().await?;

    // White balance modes
    println!("Testing white balance modes...");
    camera.white_balance_auto().await?;
    println!("  ✓ Auto white balance");
    camera.white_balance_indoor().await?;
    println!("  ✓ Indoor");
    camera.white_balance_outdoor().await?;
    println!("  ✓ Outdoor");
    camera.white_balance_one_push().await?;
    println!("  ✓ One-push");
    camera.white_balance_auto().await?;
    println!();

    // === IMAGE ADJUSTMENTS ===
    println!("═══ Image Adjustments ═══");

    // Flip control
    println!("Testing image flip...");
    let original_flip = camera.get_image_flip().await?;
    println!(
        "  Current: V={}, H={}",
        original_flip.vertical, original_flip.horizontal
    );

    if original_flip.vertical {
        camera.disable_flip().await?;
        println!("  ✓ Flip disabled");
        // Flip is an instant operation, but verify the state change
        let mut retries = 0;
        while camera.get_image_flip().await?.vertical && retries < 10 {
            sleep(Duration::from_millis(50)).await;
            retries += 1;
        }
        camera.enable_flip().await?;
        println!("  ✓ Flip re-enabled");
    } else {
        camera.enable_flip().await?;
        println!("  ✓ Flip enabled");
        // Flip is an instant operation, but verify the state change
        let mut retries = 0;
        while !camera.get_image_flip().await?.vertical && retries < 10 {
            sleep(Duration::from_millis(50)).await;
            retries += 1;
        }
        camera.disable_flip().await?;
        println!("  ✓ Flip disabled");
    }
    println!();

    // === PRESET MANAGEMENT ===
    println!("═══ Preset Management ═══");

    // Save presets
    println!("Saving preset positions...");

    // Preset 1: Wide overview
    camera
        .pan_tilt_absolute(Degrees(0.0), Degrees(0.0), SpeedLevel::Medium)
        .await?;
    camera.zoom_absolute(Normalized(0.0)).await?;
    camera.await_idle(Duration::from_secs(5)).await?;
    camera.preset_set(PresetNumber::new(1)?).await?;
    println!("  ✓ Preset 1 (Wide Overview) saved");

    // Preset 2: Right view
    camera
        .pan_tilt_absolute(Degrees(45.0), Degrees(-10.0), SpeedLevel::Medium)
        .await?;
    camera.zoom_absolute(Normalized(0.3)).await?;
    camera.await_idle(Duration::from_secs(5)).await?;
    camera.preset_set(PresetNumber::new(2)?).await?;
    println!("  ✓ Preset 2 (Right View) saved");

    // Preset 3: Left view
    camera
        .pan_tilt_absolute(Degrees(-45.0), Degrees(-10.0), SpeedLevel::Medium)
        .await?;
    camera.zoom_absolute(Normalized(0.3)).await?;
    camera.await_idle(Duration::from_secs(5)).await?;
    camera.preset_set(PresetNumber::new(3)?).await?;
    println!("  ✓ Preset 3 (Left View) saved");

    // Test preset recall
    println!("Testing preset recall...");
    for i in 1..=3 {
        println!("  Recalling Preset {i}...");
        camera.preset_recall(PresetNumber::new(i)?).await?;
        camera.await_idle(Duration::from_secs(5)).await?;
        if let Ok((pan, tilt)) = camera.get_pan_tilt_degrees().await {
            println!("    Position: Pan={:.1}°, Tilt={:.1}°", pan.0, tilt.0);
        }
    }
    println!("✓ Preset recall complete");

    // Clear a preset
    println!("Clearing Preset 3...");
    camera.preset_reset(PresetNumber::new(3)?).await?;
    println!("✓ Preset 3 cleared");
    println!();

    // === CONCURRENT OPERATIONS (ASYNC ADVANTAGE) ===
    println!("═══ Concurrent Operations (Async Advantage!) ═══");
    println!("Performing multiple queries concurrently...");

    // Perform multiple queries concurrently
    let (pan_tilt, zoom, focus_mode, exposure_mode, white_balance) = tokio::join!(
        camera.get_pan_tilt_degrees(),
        camera.get_zoom_position(),
        camera.get_focus_mode(),
        camera.get_exposure_mode(),
        camera.get_white_balance_mode()
    );

    println!("  Concurrent query results:");
    if let Ok((pan, tilt)) = pan_tilt {
        println!("    Position: Pan={:.1}°, Tilt={:.1}°", pan.0, tilt.0);
    }
    if let Ok(zoom) = zoom {
        println!("    Zoom: {zoom}");
    }
    if let Ok(mode) = focus_mode {
        println!("    Focus: {mode:?}");
    }
    if let Ok(mode) = exposure_mode {
        println!("    Exposure: {mode:?}");
    }
    if let Ok(mode) = white_balance {
        println!("    White Balance: {mode:?}");
    }

    println!("✓ Concurrent operations complete");
    println!("  Note: All 5 queries executed in parallel!");
    println!();

    // === RESTORE INITIAL STATE ===
    println!("═══ Finishing Demo ═══");
    println!("Restoring camera to initial state...");

    match (&initial_position, &initial_zoom) {
        (Ok((pan, tilt)), Ok(zoom)) => {
            // Move back to initial position and zoom concurrently
            let pan_tilt_future = camera.pan_tilt_absolute(*pan, *tilt, SpeedLevel::Fast);
            let zoom_future = camera.zoom_absolute(Normalized((*zoom as f32) / 16384.0));

            // Execute both movements concurrently
            tokio::try_join!(pan_tilt_future, zoom_future)?;

            // Wait for both to complete
            let _ = tokio::join!(
                camera.await_pan_tilt_idle(Duration::from_secs(30)),
                camera.await_zoom_idle(Duration::from_secs(10))
            );

            println!("✓ Camera restored to initial state");
        }
        _ => {
            // Fallback to home position
            camera.pan_tilt_home().await?;
            camera.await_pan_tilt_idle(Duration::from_secs(30)).await?;
            camera.zoom_absolute(Normalized(0.0)).await?;
            camera.await_zoom_idle(Duration::from_secs(10)).await?;
            println!("✓ Camera at home position");
        }
    }

    println!();
    println!("✨ Async demo complete!");
    println!();
    println!("Demonstrated features:");
    println!("  ✓ Basic movement (pan/tilt/zoom)");
    println!("  ✓ Advanced positioning (absolute, relative)");
    println!("  ✓ Focus control (auto, manual, one-push)");
    println!("  ✓ Exposure & white balance modes");
    println!("  ✓ Image adjustments (flip)");
    println!("  ✓ Preset management (save, recall, clear)");
    println!("  ✓ Concurrent operations (async advantage!)");
    println!();
    println!("Next steps:");
    println!("  - Try quickstart for the blocking version");
    println!("  - Check concurrent_control for more async patterns");

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example quickstart_async --features tokio");
    std::process::exit(1);
}
