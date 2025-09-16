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
            focus::FocusControl, image_processing::ImageProcessingControl,
            pan_tilt::PanTiltControl, zoom::ZoomControl,
        },
        profiles::PtzOpticsG2,
    },
    runtime_trait::TokioRuntime,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    units::{Degrees, Normalized},
    Error,
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

    // Create the Tokio runtime and connect with type-safe pairing
    let runtime = TokioRuntime::from_current()?;
    let camera = grafton_visca::camera::convenience::Camera::open_tcp_async::<PtzOpticsG2, _>(
        camera_addr,
        runtime,
    )
    .await?;

    println!("✅ Connected successfully!");
    println!();

    // Get the camera reference from the session
    let cam = camera.camera().expect("Camera session is open");

    println!("═══ Saving Initial Camera State ═══");

    let _initial_pan = Degrees(0.0);
    let _initial_tilt = Degrees(0.0);

    println!("✓ Initial state saved (will return to home at end)");
    println!();

    println!("═══ Basic Movement Operations ═══");

    println!("Moving to home position...");
    cam.pan_tilt().home().await?;
    cam.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    println!("✓ At home position");

    println!("Testing zoom...");
    println!("  Zooming in briefly...");
    cam.zoom().tele().await?;
    sleep(Duration::from_millis(100)).await;
    cam.zoom().stop().await?;
    cam.await_zoom_idle(Duration::from_secs(5)).await?;

    println!("  Zooming out briefly...");
    cam.zoom().wide().await?;
    sleep(Duration::from_millis(100)).await;
    cam.zoom().stop().await?;
    cam.await_zoom_idle(Duration::from_secs(5)).await?;
    println!("✓ Zoom complete");

    println!("Testing pan/tilt...");
    println!("  Panning right briefly...");
    cam.pan_tilt()
        .right(PanSpeed::new(10)?, TiltSpeed::new(0)?)
        .await?;
    sleep(Duration::from_millis(100)).await;
    cam.pan_tilt().stop().await?;
    cam.await_pan_tilt_idle(Duration::from_secs(5)).await?;

    println!("  Tilting up briefly...");
    cam.pan_tilt()
        .up(PanSpeed::new(0)?, TiltSpeed::new(10)?)
        .await?;
    sleep(Duration::from_millis(100)).await;
    cam.pan_tilt().stop().await?;
    cam.await_pan_tilt_idle(Duration::from_secs(5)).await?;
    println!("✓ Pan/tilt complete");
    println!();

    println!("═══ Advanced Positioning ═══");
    println!("Moving to absolute position (45°, 15°) with custom detection...");
    cam.pan_tilt()
        .absolute(Degrees::new(45.0), Degrees::new(15.0), SpeedLevel::Fast)
        .await?;

    sleep(Duration::from_secs(3)).await;
    println!("✓ Moved to position");
    println!("Moving relative (+10°, +5°)...");
    cam.pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)
        .await?;
    cam.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    println!("✓ Relative movement complete");

    println!("Setting zoom to 50%...");
    cam.zoom_absolute(Normalized(0.5)).await?;
    cam.await_zoom_idle(Duration::from_secs(10)).await?;
    println!("✓ Zoom at 50%");
    println!();

    // === DEMONSTRATE NEW MOVEMENT DETECTION ===
    println!("═══ Advanced Movement Detection ═══");

    println!("Moving to absolute position with await...");
    cam.pan_tilt()
        .absolute(Degrees(-30.0), Degrees(10.0), SpeedLevel::Medium)
        .await?;
    cam.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    println!("✓ Movement completed");

    println!("Initiating multiple movements...");
    cam.pan_tilt()
        .absolute(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Fast)
        .await?;
    cam.zoom_absolute(Normalized(0.3)).await?;

    println!("Waiting for all movements to complete...");
    cam.await_idle(Duration::from_secs(30)).await?;
    println!("✓ All movements completed");

    println!("\nDemonstrating async concurrent operations...");
    use tokio::join;

    // Start multiple movements simultaneously
    let pan_tilt = cam.pan_tilt_absolute(Degrees(20.0), Degrees(-5.0), SpeedLevel::Fast);
    let zoom = cam.zoom_absolute(Normalized(0.6));

    // Execute them concurrently
    let (pt_result, z_result) = join!(pan_tilt, zoom);
    pt_result?;
    z_result?;

    // Wait for all to complete
    cam.await_idle(Duration::from_secs(30)).await?;
    println!("✓ Concurrent operations completed");
    println!();

    // === FOCUS CONTROL ===
    println!("═══ Focus Control ═══");

    println!("Setting auto focus...");
    cam.focus().auto().await?;
    println!("✓ Auto focus enabled");

    println!("Testing manual focus...");
    cam.focus().manual().await?;
    cam.focus().near(SpeedLevel::Medium).await?;
    sleep(Duration::from_millis(100)).await;
    cam.focus().stop().await?;
    cam.await_focus_idle(Duration::from_secs(5)).await?;

    cam.focus().far(SpeedLevel::Medium).await?;
    sleep(Duration::from_millis(100)).await;
    cam.focus().stop().await?;
    cam.await_focus_idle(Duration::from_secs(5)).await?;
    println!("✓ Manual focus complete");

    println!("Triggering one-push auto focus...");
    cam.focus_one_push().await?;
    cam.await_focus_idle(Duration::from_secs(10)).await?;
    println!("✓ One-push focus complete");
    cam.focus().auto().await?;
    println!();

    // === EXPOSURE & WHITE BALANCE ===
    println!("═══ Exposure & White Balance ═══");

    // Exposure modes
    println!("Testing exposure modes...");
    cam.exposure().auto().await?;
    println!("  ✓ Auto exposure");
    cam.exposure().manual().await?;
    println!("  ✓ Manual exposure");
    cam.exposure().shutter_priority().await?;
    println!("  ✓ Shutter priority");
    cam.exposure().auto().await?;

    // White balance modes
    println!("Testing white balance modes...");
    cam.white_balance().auto().await?;
    println!("  ✓ Auto white balance");
    cam.white_balance().indoor().await?;
    println!("  ✓ Indoor");
    cam.white_balance().outdoor().await?;
    println!("  ✓ Outdoor");
    cam.white_balance().one_push_trigger().await?;
    println!("  ✓ One-push");
    cam.white_balance().auto().await?;
    println!();

    // === IMAGE ADJUSTMENTS ===
    println!("═══ Image Adjustments ═══");

    // Flip control
    println!("Testing image flip...");

    // Enable vertical flip
    cam.enable_flip().await?;
    println!("  ✓ Vertical flip enabled");
    sleep(Duration::from_millis(500)).await;

    // Enable horizontal flip
    cam.enable_horizontal_flip().await?;
    println!("  ✓ Horizontal flip enabled");
    sleep(Duration::from_millis(500)).await;

    // Disable both flips
    cam.disable_flip().await?;
    cam.disable_horizontal_flip().await?;
    println!("  ✓ Flips disabled");
    println!();

    // === PRESET MANAGEMENT ===
    println!("═══ Preset Management ═══");

    // Save presets
    println!("Saving preset positions...");

    // Preset 1: Wide overview
    cam.pan_tilt()
        .absolute(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Medium)
        .await?;
    cam.zoom_absolute(Normalized(0.0)).await?;
    cam.await_idle(Duration::from_secs(5)).await?;
    cam.presets().set(1).await?;
    println!("  ✓ Preset 1 (Wide Overview) saved");

    // Preset 2: Right view
    cam.pan_tilt()
        .absolute(Degrees::new(45.0), Degrees::new(-10.0), SpeedLevel::Medium)
        .await?;
    cam.zoom_absolute(Normalized(0.3)).await?;
    cam.await_idle(Duration::from_secs(5)).await?;
    cam.presets().set(2).await?;
    println!("  ✓ Preset 2 (Right View) saved");

    // Preset 3: Left view
    cam.pan_tilt()
        .absolute(Degrees::new(-45.0), Degrees::new(-10.0), SpeedLevel::Medium)
        .await?;
    cam.zoom_absolute(Normalized(0.3)).await?;
    cam.await_idle(Duration::from_secs(5)).await?;
    cam.presets().set(3).await?;
    println!("  ✓ Preset 3 (Left View) saved");

    // Test preset recall
    println!("Testing preset recall...");
    for i in 1..=3 {
        println!("  Recalling Preset {i}...");
        cam.presets().recall(i).await?;
        cam.await_idle(Duration::from_secs(5)).await?;
        println!("    Position saved");
    }
    println!("✓ Preset recall complete");

    // Clear a preset
    println!("Clearing Preset 3...");
    cam.presets().reset(3).await?;
    println!("✓ Preset 3 cleared");
    println!();

    // === CONCURRENT OPERATIONS (ASYNC ADVANTAGE) ===
    println!("═══ Concurrent Operations (Async Advantage!) ═══");
    println!("Performing concurrent operations...");

    // Execute multiple operations concurrently
    let _: (Result<(), Error>, Result<(), Error>) =
        tokio::join!(cam.pan_tilt_home(), cam.zoom_stop());

    println!("✓ Concurrent operations complete");
    println!("  Note: Multiple commands executed in parallel!");
    println!();

    // === RESTORE INITIAL STATE ===
    println!("═══ Finishing Demo ═══");
    println!("Restoring camera to initial state...");

    // Return to home position
    cam.pan_tilt().home().await?;
    cam.await_pan_tilt_idle(Duration::from_secs(30)).await?;
    cam.zoom_absolute(Normalized(0.0)).await?;
    cam.await_zoom_idle(Duration::from_secs(10)).await?;
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
