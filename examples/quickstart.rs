//! Comprehensive camera control example using the blocking API.
//!
//! This example demonstrates the full range of camera control operations available
//! in the **BLOCKING API** (no async runtime required), including:
//! - Connection and power management
//! - Pan/Tilt/Zoom (PTZ) operations
//! - Focus control
//! - Exposure settings
//! - White balance
//! - Image adjustments
//! - Presets management
//! - Speed control
//!
//! **Context**: This is a pure blocking example that requires NO async features or runtime.
//! It demonstrates the library's ability to work with zero async dependencies.
//!
//! Run with:
//! ```sh
//! cargo run --example quickstart [camera_ip[:port]]
//! ```

#[cfg(not(feature = "mode-async"))]
use std::{env, thread::sleep, time::Duration};

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    mode::BlockingFutureExt,
    types::{PanSpeed, PanTiltDirection, SpeedLevel, TiltSpeed},
    units::{Degrees, Normalized},
    Error,
};

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let camera_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:52381".to_string());

    println!("🎥 Comprehensive Camera Control Demo");
    println!("====================================");
    println!("Connecting to camera at {camera_addr}");
    println!();

    // Use the new convenience API for blocking mode
    let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&camera_addr)?;

    println!("✅ Connected successfully!");
    println!();

    println!("═══ Saving Initial Camera State ═══");
    println!("⚠ Position inquiry not yet available, will return to home at end");
    println!();

    println!("═══ Basic Movement Operations ═══");

    println!("Moving to home position...");
    camera.pan_tilt().home().block()?;
    camera.await_pan_tilt_idle(Duration::from_secs(5))?;
    println!("✓ At home position");

    println!("Testing zoom...");
    println!("  Zooming in briefly...");
    camera.zoom().tele().block()?;
    sleep(Duration::from_millis(100));
    camera.zoom().stop().block()?;
    camera.await_zoom_idle(Duration::from_secs(2))?;

    println!("  Zooming out briefly...");
    camera.zoom().wide().block()?;
    sleep(Duration::from_millis(100));
    camera.zoom().stop().block()?;
    camera.await_zoom_idle(Duration::from_secs(2))?;
    println!("✓ Zoom complete");

    println!("Testing pan/tilt...");
    println!("  Panning right briefly...");
    camera
        .pan_tilt()
        .move_direction(
            PanTiltDirection::Right,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .block()?;
    sleep(Duration::from_millis(100));
    camera.pan_tilt().stop().block()?;
    sleep(Duration::from_secs(1));

    println!("  Tilting up briefly...");
    camera
        .pan_tilt()
        .move_direction(PanTiltDirection::Up, PanSpeed::new(0)?, TiltSpeed::new(10)?)
        .block()?;
    sleep(Duration::from_millis(100));
    camera.pan_tilt().stop().block()?;
    sleep(Duration::from_secs(1));
    println!("✓ Pan/tilt complete");
    println!();

    println!("═══ Advanced Positioning ═══");

    println!("Moving to absolute position (45°, 15°)...");
    camera
        .pan_tilt()
        .absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)
        .block()?;
    sleep(Duration::from_secs(3));
    println!("✓ Moved to position");

    println!("Moving relative (+10°, +5°)...");
    camera
        .pan_tilt()
        .relative(Degrees(10.0), Degrees(5.0), SpeedLevel::Medium)
        .block()?;

    sleep(Duration::from_secs(2));
    println!("✓ Relative movement complete");

    println!("Setting zoom to 50%...");
    camera.zoom().absolute(Normalized(0.5)).block()?;
    sleep(Duration::from_secs(2));
    println!("✓ Zoom at 50%");
    println!();

    println!();

    println!("═══ Focus Control ═══");

    println!("Setting auto focus...");
    camera.focus().auto().block()?;
    println!("✓ Auto focus enabled");

    println!("Testing manual focus...");
    camera.focus().manual().block()?;
    camera.focus().near(SpeedLevel::Medium).block()?;
    sleep(Duration::from_millis(100));
    camera.focus().stop().block()?;
    sleep(Duration::from_secs(1));

    camera.focus().far(SpeedLevel::Medium).block()?;
    sleep(Duration::from_millis(100));
    camera.focus().stop().block()?;
    sleep(Duration::from_secs(1));
    println!("✓ Manual focus complete");

    println!("Triggering one-push auto focus...");
    camera.focus().one_push().block()?;
    sleep(Duration::from_secs(2));
    println!("✓ One-push focus complete");
    camera.focus().auto().block()?;
    println!();

    println!("═══ Exposure & White Balance ═══");

    println!("Testing exposure modes...");
    camera.exposure().auto().block()?;
    println!("  ✓ Auto exposure");
    camera.exposure().manual().block()?;
    println!("  ✓ Manual exposure");
    camera.exposure().shutter_priority().block()?;
    println!("  ✓ Shutter priority");
    camera.exposure().auto().block()?;

    println!("Testing white balance modes...");
    camera.white_balance().auto().block()?;
    println!("  ✓ Auto white balance");
    camera.white_balance().indoor().block()?;
    println!("  ✓ Indoor");
    camera.white_balance().outdoor().block()?;
    println!("  ✓ Outdoor");
    camera.white_balance().one_push_trigger().block()?;
    println!("  ✓ One-push");
    camera.white_balance().auto().block()?;
    println!();

    println!();

    println!("═══ Preset Management ═══");

    println!("Saving preset positions...");

    camera
        .pan_tilt()
        .absolute(Degrees(0.0), Degrees(0.0), SpeedLevel::Medium)
        .block()?;
    camera.zoom().absolute(Normalized(0.0)).block()?;
    sleep(Duration::from_secs(3));
    camera.presets().set(1).block()?;
    println!("  ✓ Preset 1 (Wide Overview) saved");

    camera
        .pan_tilt()
        .absolute(Degrees(45.0), Degrees(-10.0), SpeedLevel::Medium)
        .block()?;
    camera.zoom().absolute(Normalized(0.3)).block()?;
    sleep(Duration::from_secs(3));
    camera.presets().set(2).block()?;
    println!("  ✓ Preset 2 (Right View) saved");

    camera
        .pan_tilt()
        .absolute(Degrees(-45.0), Degrees(-10.0), SpeedLevel::Medium)
        .block()?;
    camera.zoom().absolute(Normalized(0.3)).block()?;
    sleep(Duration::from_secs(3));
    camera.presets().set(3).block()?;
    println!("  ✓ Preset 3 (Left View) saved");

    println!("Testing preset recall...");
    for i in 1..=3 {
        println!("  Recalling Preset {i}...");
        camera.presets().recall(i).block()?;
        sleep(Duration::from_secs(3));
    }
    println!("✓ Preset recall complete");

    println!("Clearing Preset 3...");
    camera.presets().reset(3).block()?;
    println!("✓ Preset 3 cleared");
    println!();

    println!("═══ Finishing Demo ═══");
    println!("Restoring camera to home position...");

    camera.pan_tilt().home().block()?;
    sleep(Duration::from_secs(3));
    camera.zoom().absolute(Normalized(0.0)).block()?;
    sleep(Duration::from_secs(2));
    println!("✓ Camera at home position");

    println!();
    // Explicit cleanup (optional - will auto-close on drop)
    camera.close()?;

    println!("✨ Demo complete!");
    println!();
    println!("Demonstrated features:");
    println!("  ✓ Basic movement (pan/tilt/zoom)");
    println!("  ✓ Advanced positioning (absolute, relative)");
    println!("  ✓ Focus control (auto, manual, one-push)");
    println!("  ✓ Exposure & white balance modes");
    println!("  ✓ Image adjustments (flip)");
    println!("  ✓ Preset management (save, recall, clear)");
    println!();
    println!("Next steps:");
    println!("  - Try quickstart_async for the async version");
    println!("  - Check other examples for specific features");

    Ok(())
}

#[cfg(feature = "mode-async")]
fn main() {
    println!("This example requires blocking mode. Run without the async feature:");
    println!("  cargo run --example quickstart --no-default-features");
}
