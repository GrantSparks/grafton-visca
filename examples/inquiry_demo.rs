//! Comprehensive camera inquiry demonstration.
//!
//! This example demonstrates all available inquiry commands to query
//! the complete state of a PTZ camera. It shows how to:
//! - Query power and system information
//! - Get current position (pan/tilt/zoom/focus)
//! - Read exposure settings (mode, iris, shutter, gain)
//! - Check image settings (white balance, brightness, contrast, etc.)
//! - Retrieve noise reduction levels
//! - Query image orientation and resolution
//!
//! Run with:
//! ```bash
//! # Blocking mode
//! cargo run --example inquiry_demo 192.168.0.110
//!
//! # Async mode (much faster with concurrent queries)
//! cargo run --example inquiry_demo --features rt-tokio 192.168.0.110
//! ```

#[cfg(not(feature = "async"))]
fn main() -> grafton_visca::Result<()> {
    use grafton_visca::CameraBuilder;

    env_logger::init();

    println!("=== Comprehensive Camera Inquiry Demo (Blocking) ===\n");

    // Get camera address from command line or use default
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("Connecting to camera at {camera_addr}...");
    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<grafton_visca::camera::profiles::GenericVisca>()
        .build()?;

    println!("\n📊 Querying all camera parameters...\n");

    // System Information
    println!("── System Information ──");
    match camera.power_inquiry() {
        Ok(is_on) => println!("  Power: {}", if is_on { "🟢 ON" } else { "🔴 OFF" }),
        Err(e) => println!("  Power: ❌ {e}"),
    }

    match camera.version_inquiry() {
        Ok(version) => println!("  Version: {version:?}"),
        Err(e) => println!("  Version: ❌ {e}"),
    }

    match camera.resolution_inquiry() {
        Ok(res) => println!("  Resolution: {res:?}"),
        Err(e) => println!("  Resolution: ❌ {e}"),
    }

    // Position Information
    println!("\n── Position ──");
    match camera.pan_tilt_position_inquiry() {
        Ok((pan, tilt)) => {
            println!("  Pan: {pan:?}");
            println!("  Tilt: {tilt:?}");
        }
        Err(e) => println!("  Pan/Tilt: ❌ {e}"),
    }

    match camera.zoom_position_inquiry() {
        Ok(zoom) => {
            println!("  Zoom: {zoom:?}");
            // Note: The inner value is not publicly accessible
        }
        Err(e) => println!("  Zoom: ❌ {e}"),
    }

    // Focus Settings
    println!("\n── Focus ──");
    match camera.focus_mode_inquiry() {
        Ok(mode) => println!("  Mode: {mode:?}"),
        Err(e) => println!("  Mode: ❌ {e}"),
    }

    // auto_focus_inquiry() has been commented out pending verification
    // match camera.auto_focus_inquiry() {
    //     Ok(enabled) => println!("  Auto Focus: {}", if enabled { "✓" } else { "✗" }),
    //     Err(e) => println!("  Auto Focus: ❌ {e}"),
    // }

    match camera.focus_position_inquiry() {
        Ok(pos) => println!("  Position: {pos:?}"),
        Err(e) => println!("  Position: ❌ {e}"),
    }

    match camera.focus_near_limit_inquiry() {
        Ok(limit) => println!("  Near Limit: 0x{limit:04X}"),
        Err(e) => println!("  Near Limit: ❌ {e}"),
    }

    // Exposure Settings
    println!("\n── Exposure ──");
    match camera.exposure_mode_inquiry() {
        Ok(mode) => println!("  Mode: {mode:?}"),
        Err(e) => println!("  Mode: ❌ {e}"),
    }

    match camera.iris_inquiry() {
        Ok(iris) => println!("  Iris: {iris:?}"),
        Err(e) => println!("  Iris: ❌ {e}"),
    }

    match camera.shutter_inquiry() {
        Ok(shutter) => println!("  Shutter: {shutter:?}"),
        Err(e) => println!("  Shutter: ❌ {e}"),
    }

    match camera.gain_inquiry() {
        Ok(gain) => println!("  Gain: {gain:?}"),
        Err(e) => println!("  Gain: ❌ {e}"),
    }

    match camera.gain_limit_inquiry() {
        Ok(limit) => println!("  Gain Limit: {limit:?}"),
        Err(e) => println!("  Gain Limit: ❌ {e}"),
    }

    match camera.brightness_inquiry() {
        Ok(val) => println!("  Brightness: {val:?}"),
        Err(e) => println!("  Brightness: ❌ {e}"),
    }

    match camera.exposure_compensation_inquiry() {
        Ok(enabled) => println!("  Compensation: {}", if enabled != 0 { "✓" } else { "✗" }),
        Err(e) => println!("  Compensation: ❌ {e}"),
    }

    match camera.exposure_compensation_mode_inquiry() {
        Ok(val) => println!("  Compensation Level: {val:?}"),
        Err(e) => println!("  Compensation Level: ❌ {e}"),
    }

    match camera.backlight_inquiry() {
        Ok(enabled) => println!("  Backlight Comp: {}", if enabled { "✓" } else { "✗" }),
        Err(e) => println!("  Backlight Comp: ❌ {e}"),
    }

    // White Balance
    println!("\n── White Balance ──");
    match camera.white_balance_mode_inquiry() {
        Ok(mode) => println!("  Mode: {mode:?}"),
        Err(e) => println!("  Mode: ❌ {e}"),
    }

    match camera.color_temperature_inquiry() {
        Ok(temp) => println!("  Color Temperature: {temp}K"),
        Err(e) => println!("  Color Temperature: ❌ {e}"),
    }

    // Image Adjustments
    println!("\n── Image Adjustments ──");
    // sharpness_inquiry() and contrast_inquiry() have been commented out pending verification
    // These are not standard VISCA inquiries according to the protocol documentation
    // match camera.sharpness_inquiry() {
    //     Ok(val) => println!("  Sharpness: {val:?}"),
    //     Err(e) => println!("  Sharpness: ❌ {e}"),
    // }

    // match camera.contrast_inquiry() {
    //     Ok(val) => println!("  Contrast: {val:?}"),
    //     Err(e) => println!("  Contrast: ❌ {e}"),
    // }

    match camera.saturation_inquiry() {
        Ok(val) => println!("  Saturation: {val:?}"),
        Err(e) => println!("  Saturation: ❌ {e}"),
    }

    match camera.hue_inquiry() {
        Ok(val) => println!("  Hue: {val:?}"),
        Err(e) => println!("  Hue: ❌ {e}"),
    }

    match camera.image_flip_inquiry() {
        Ok(flip) => println!("  Image Flip: {flip:?}"),
        Err(e) => println!("  Image Flip: ❌ {e}"),
    }

    // Noise Reduction
    println!("\n── Noise Reduction ──");
    match camera.noise_reduction_2d_inquiry() {
        Ok(level) => println!("  2D NR Level: {level:?}"),
        Err(e) => println!("  2D NR: ❌ {e}"),
    }

    match camera.noise_reduction_3d_inquiry() {
        Ok(level) => println!("  3D NR Level: {level:?}"),
        Err(e) => println!("  3D NR: ❌ {e}"),
    }

    println!("\n✅ Inquiry demo completed!");
    println!("💡 Tip: Run with --features rt-tokio for faster concurrent queries!");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    use grafton_visca::CameraBuilder;
    use tokio::time::{Duration, Instant};

    env_logger::init();

    println!("=== Comprehensive Camera Inquiry Demo (Async) ===\n");

    // Get camera address from command line or use default
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("Connecting to camera at {camera_addr}...");
    let camera = CameraBuilder::tokio_tcp(&camera_addr)
        .profile::<grafton_visca::camera::profiles::GenericVisca>()
        .build()
        .await?;

    println!("\n⚡ Executing all inquiries concurrently...\n");

    let start = Instant::now();

    // Execute all inquiries concurrently using tokio::join!
    let (
        power,
        version,
        resolution,
        pan_tilt,
        zoom,
        focus_mode,
        // auto_focus, // Not documented in VISCA specs
        focus_pos,
        focus_near,
        exposure_mode,
        iris,
        shutter,
        gain,
        gain_limit,
        brightness,
        exp_comp,
        exp_comp_mode,
        backlight,
        wb_mode,
        color_temp,
        // sharpness, // Not documented in VISCA specs
        // contrast,  // Not documented in VISCA specs
        saturation,
        hue,
        flip,
        nr_2d,
        nr_3d,
    ) = tokio::join!(
        camera.power_inquiry(),
        camera.version_inquiry(),
        camera.resolution_inquiry(),
        camera.pan_tilt_position_inquiry(),
        camera.zoom_position_inquiry(),
        camera.focus_mode_inquiry(),
        // camera.auto_focus_inquiry(), // Not documented in VISCA specs
        camera.focus_position_inquiry(),
        camera.focus_near_limit_inquiry(),
        camera.exposure_mode_inquiry(),
        camera.iris_inquiry(),
        camera.shutter_inquiry(),
        camera.gain_inquiry(),
        camera.gain_limit_inquiry(),
        camera.brightness_inquiry(),
        camera.exposure_compensation_inquiry(),
        camera.exposure_compensation_mode_inquiry(),
        camera.backlight_inquiry(),
        camera.white_balance_mode_inquiry(),
        camera.color_temperature_inquiry(),
        // camera.sharpness_inquiry(), // Not documented in VISCA specs
        // camera.contrast_inquiry(),  // Not documented in VISCA specs
        camera.saturation_inquiry(),
        camera.hue_inquiry(),
        camera.image_flip_inquiry(),
        camera.noise_reduction_2d_inquiry(),
        camera.noise_reduction_3d_inquiry(),
    );

    let elapsed = start.elapsed();

    // Display results
    println!("── System Information ──");
    println!(
        "  Power: {}",
        power.map_or("❌".to_string(), |on| if on {
            "🟢 ON".to_string()
        } else {
            "🔴 OFF".to_string()
        })
    );
    println!("  Version: {:?}", version.ok());
    println!("  Resolution: {:?}", resolution.ok());

    println!("\n── Position ──");
    if let Ok((pan, tilt)) = pan_tilt {
        println!("  Pan: {pan:?}");
        println!("  Tilt: {tilt:?}");
    } else {
        println!("  Pan/Tilt: ❌");
    }

    if let Ok(z) = zoom {
        println!("  Zoom: {z:?}");
        // Note: The inner value is not publicly accessible
    } else {
        println!("  Zoom: ❌");
    }

    println!("\n── Focus ──");
    println!("  Mode: {:?}", focus_mode.ok());
    // NOTE: auto_focus inquiry is not documented in VISCA specs
    // println!(
    //     "  Auto Focus: {}",
    //     auto_focus.map_or("❌".to_string(), |e| if e { "✓" } else { "✗" })
    // );
    println!("  Position: {:?}", focus_pos.ok());
    println!(
        "  Near Limit: {}",
        focus_near.map_or("❌".to_string(), |l| format!("0x{l:04X}"))
    );

    println!("\n── Exposure ──");
    println!("  Mode: {:?}", exposure_mode.ok());
    println!("  Iris: {:?}", iris.ok());
    println!("  Shutter: {:?}", shutter.ok());
    println!("  Gain: {:?}", gain.ok());
    println!("  Gain Limit: {:?}", gain_limit.ok());
    println!("  Brightness: {:?}", brightness.ok());
    println!(
        "  Compensation: {}",
        exp_comp.map_or("❌".to_string(), |e| format!("{}", e))
    );
    println!("  Compensation Level: {:?}", exp_comp_mode.ok());
    println!(
        "  Backlight Comp: {}",
        backlight.map_or("❌".to_string(), |e| if e {
            "✓".to_string()
        } else {
            "✗".to_string()
        })
    );

    println!("\n── White Balance ──");
    println!("  Mode: {:?}", wb_mode.ok());
    println!("  Color Temperature: {}K", color_temp.unwrap_or(0));

    println!("\n── Image Adjustments ──");
    // NOTE: sharpness and contrast inquiries are not documented in VISCA specs
    // println!("  Sharpness: {:?}", sharpness.unwrap_or_default());
    // println!("  Contrast: {:?}", contrast.unwrap_or_default());
    println!("  Saturation: {:?}", saturation.ok());
    println!("  Hue: {:?}", hue.ok());
    println!("  Image Flip: {:?}", flip.ok());

    println!("\n── Noise Reduction ──");
    println!("  2D NR Level: {:?}", nr_2d.ok());
    println!("  3D NR Level: {:?}", nr_3d.ok());

    println!("\n✅ All inquiries completed in {:.2?}!", elapsed);
    println!(
        "🚀 Concurrent execution is {} faster than sequential!",
        if elapsed < Duration::from_secs(1) {
            "much"
        } else {
            "significantly"
        }
    );

    Ok(())
}

#[cfg(all(feature = "async", not(feature = "rt-tokio")))]
fn main() {
    println!("This example requires either blocking mode or tokio runtime:");
    println!("  cargo run --example inquiry_demo");
    println!("  cargo run --example inquiry_demo --features rt-tokio");
}
