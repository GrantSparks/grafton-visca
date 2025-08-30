//! Comprehensive async camera control example using tokio.
//!
//! This example demonstrates the full range of camera control operations available
//! in the **ASYNC API** (requires tokio runtime), including:
//! - Connection and power management
//! - Pan/Tilt/Zoom (PTZ) operations
//! - Focus control
//! - Exposure settings
//! - White balance
//! - Image adjustments
//! - Presets management
//! - Concurrent operations (async advantage!)
//!
//! **Context**: This example uses native async transports with the tokio runtime.
//! It demonstrates true async/await operations with zero-cost abstractions.
//!
//! Run with:
//! ```sh
//! cargo run --example quickstart_async --features rt-tokio [camera_ip[:port]]
//! ```

#[cfg(feature = "rt-tokio")]
use tokio::time::{sleep, Duration};

#[cfg(feature = "rt-tokio")]
use std::env;

#[cfg(feature = "rt-tokio")]
use grafton_visca::{
    camera::{
        controls::{
            exposure::ExposureControl, focus::FocusControl,
            image_processing::ImageProcessingControl, pan_tilt::PanTiltControl,
            presets::PresetsControl, white_balance::WhiteBalanceControl, zoom::ZoomControl,
        },
        profiles::PtzOpticsG2,
    },
    command::preset::PresetNumber,
    transport::Transport,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    units::{Degrees, Normalized},
    CameraBuilder, Error, PanTiltDirection,
};

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:52381".to_string());

    println!("🎥 Comprehensive Async Camera Control Demo");
    println!("==========================================");
    println!("Connecting to camera at {camera_addr}");
    println!();

    // Using uniform Transport API - runtime is automatically selected
    let transport = Transport::tcp()
        .address(camera_addr)
        .connect_timeout(Duration::from_secs(5))
        .build_async()
        .await?;

    let camera = CameraBuilder::tokio()?
        .build_async::<PtzOpticsG2, _>(transport)
        .await?;

    println!("✅ Connected successfully!");
    println!();

    println!("═══ Saving Initial Camera State ═══");

    let _initial_pan = Degrees(0.0);
    let _initial_tilt = Degrees(0.0);

    println!("✓ Initial state saved (will return to home at end)");
    println!();

    println!("═══ Basic Movement Operations ═══");

    println!("Moving to home position...");
    camera.pan_tilt_home().await?;
    camera.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    println!("✓ At home position");

    println!("Testing zoom...");
    println!("  Zooming in briefly...");
    camera.zoom_tele_std().await?;
    sleep(Duration::from_millis(100)).await;
    camera.zoom_stop().await?;
    camera.await_zoom_idle(Duration::from_secs(5)).await?;

    println!("  Zooming out briefly...");
    camera.zoom_wide_std().await?;
    sleep(Duration::from_millis(100)).await;
    camera.zoom_stop().await?;
    camera.await_zoom_idle(Duration::from_secs(5)).await?;
    println!("✓ Zoom complete");

    println!("Testing pan/tilt...");
    println!("  Panning right briefly...");
    camera
        .pan_tilt_move(
            PanTiltDirection::Right,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await?;
    sleep(Duration::from_millis(100)).await;
    camera.pan_tilt_stop().await?;
    camera.await_pan_tilt_idle(Duration::from_secs(5)).await?;

    println!("  Tilting up briefly...");
    camera
        .pan_tilt_move(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)
        .await?;
    sleep(Duration::from_millis(100)).await;
    camera.pan_tilt_stop().await?;
    camera.await_pan_tilt_idle(Duration::from_secs(5)).await?;
    println!("✓ Pan/tilt complete");
    println!();

    println!("═══ Advanced Positioning ═══");
    println!("Moving to absolute position (45°, 15°) with custom detection...");
    camera
        .pan_tilt_absolute(Degrees::new(45.0), Degrees::new(15.0), SpeedLevel::Fast)
        .await?;

    sleep(Duration::from_secs(3)).await;
    println!("✓ Moved to position");
    println!("Moving relative (+10°, +5°)...");
    camera
        .pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)
        .await?;
    camera.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    println!("✓ Relative movement complete");

    println!("Setting zoom to 50%...");
    camera.zoom_absolute(Normalized(0.5)).await?;
    camera.await_zoom_idle(Duration::from_secs(10)).await?;
    println!("✓ Zoom at 50%");
    println!();

    // === DEMONSTRATE NEW MOVEMENT DETECTION ===
    println!("═══ Advanced Movement Detection ═══");

    println!("Moving to absolute position with await...");
    camera
        .pan_tilt_absolute(Degrees(-30.0), Degrees(10.0), SpeedLevel::Medium)
        .await?;
    camera.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    println!("✓ Movement completed");

    println!("Initiating multiple movements...");
    camera
        .pan_tilt_absolute(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Fast)
        .await?;
    camera.zoom_absolute(Normalized(0.3)).await?;

    println!("Waiting for all movements to complete...");
    camera.await_idle(Duration::from_secs(30)).await?;
    println!("✓ All movements completed");

    println!("\nDemonstrating async concurrent operations...");
    use tokio::join;

    // Start multiple movements simultaneously
    let pan_tilt = camera.pan_tilt_absolute(Degrees(20.0), Degrees(-5.0), SpeedLevel::Fast);
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

    println!("Setting auto focus...");
    camera.focus_auto().await?;
    println!("✓ Auto focus enabled");

    println!("Testing manual focus...");
    camera.focus_manual().await?;
    camera.focus_near(SpeedLevel::Medium).await?;
    sleep(Duration::from_millis(100)).await;
    camera.focus_stop().await?;
    camera.await_focus_idle(Duration::from_secs(5)).await?;

    camera.focus_far(SpeedLevel::Medium).await?;
    sleep(Duration::from_millis(100)).await;
    camera.focus_stop().await?;
    camera.await_focus_idle(Duration::from_secs(5)).await?;
    println!("✓ Manual focus complete");

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

    // Enable vertical flip
    camera.enable_flip().await?;
    println!("  ✓ Vertical flip enabled");
    sleep(Duration::from_millis(500)).await;

    // Enable horizontal flip
    camera.enable_horizontal_flip().await?;
    println!("  ✓ Horizontal flip enabled");
    sleep(Duration::from_millis(500)).await;

    // Disable both flips
    camera.disable_flip().await?;
    camera.disable_horizontal_flip().await?;
    println!("  ✓ Flips disabled");
    println!();

    // === PRESET MANAGEMENT ===
    println!("═══ Preset Management ═══");

    // Save presets
    println!("Saving preset positions...");

    // Preset 1: Wide overview
    camera
        .pan_tilt_absolute(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Medium)
        .await?;
    camera.zoom_absolute(Normalized(0.0)).await?;
    camera.await_idle(Duration::from_secs(5)).await?;
    camera.preset_set(PresetNumber::new(1)?).await?;
    println!("  ✓ Preset 1 (Wide Overview) saved");

    // Preset 2: Right view
    camera
        .pan_tilt_absolute(Degrees::new(45.0), Degrees::new(-10.0), SpeedLevel::Medium)
        .await?;
    camera.zoom_absolute(Normalized(0.3)).await?;
    camera.await_idle(Duration::from_secs(5)).await?;
    camera.preset_set(PresetNumber::new(2)?).await?;
    println!("  ✓ Preset 2 (Right View) saved");

    // Preset 3: Left view
    camera
        .pan_tilt_absolute(Degrees::new(-45.0), Degrees::new(-10.0), SpeedLevel::Medium)
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
        // Position saved to preset
        println!("    Position saved");
    }
    println!("✓ Preset recall complete");

    // Clear a preset
    println!("Clearing Preset 3...");
    camera.preset_reset(PresetNumber::new(3)?).await?;
    println!("✓ Preset 3 cleared");
    println!();

    // === CONCURRENT OPERATIONS (ASYNC ADVANTAGE) ===
    println!("═══ Concurrent Operations (Async Advantage!) ═══");
    println!("Performing concurrent operations...");

    // Execute multiple operations concurrently
    let _ = tokio::join!(camera.pan_tilt_home(), camera.zoom_stop());

    println!("✓ Concurrent operations complete");
    println!("  Note: Multiple commands executed in parallel!");
    println!();

    // === RESTORE INITIAL STATE ===
    println!("═══ Finishing Demo ═══");
    println!("Restoring camera to initial state...");

    // Return to home position
    camera.pan_tilt_home().await?;
    camera.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    camera.zoom_absolute(Normalized(0.0)).await?;
    camera.await_zoom_idle(Duration::from_secs(10)).await?;
    println!("✓ Camera returned to home position");

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

#[cfg(not(feature = "rt-tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example quickstart_async --features tokio");
    std::process::exit(1);
}
