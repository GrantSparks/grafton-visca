//! Camera inquiry and state querying demonstration.
//!
//! This example shows how to query camera state and settings:
//! - Power state
//! - Position information (pan/tilt/zoom)
//! - Focus settings
//! - Exposure settings
//! - White balance
//! - Image quality parameters
//! - Profile-specific conversions (degrees vs VISCA units)
//!
//! Run with:
//! - Blocking: cargo run --example camera_inquiry
//! - Async: cargo run --example camera_inquiry --features tokio

#[cfg(not(feature = "async"))]
fn main() -> grafton_visca::Result<()> {
    use grafton_visca::{camera::profiles::PTZOpticsG2, CameraBuilder};

    env_logger::init();

    println!("=== Camera Inquiry Demo (Blocking) ===\n");

    // Get camera address from command line or use default
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("Connecting to camera at {camera_addr}...");
    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()?;

    // Query power state
    println!("\n--- Power State ---");
    match camera.power_inquiry() {
        Ok(is_on) => {
            let state = if is_on { "ON" } else { "OFF" };
            println!("Power: {state}");
        }
        Err(e) => println!("Failed to get power state: {e}"),
    }

    // Query position
    println!("\n--- Position ---");
    match camera.pan_tilt_position_inquiry() {
        Ok((pan, tilt)) => {
            println!("Pan: {:?}", pan);
            println!("Tilt: {:?}", tilt);

            // Convert to degrees using the built-in methods
            println!("Pan: {:.2}°", pan.to_degrees());
            println!("Tilt: {:.2}°", tilt.to_degrees());
        }
        Err(e) => println!("Failed to get position: {e}"),
    }

    // Query zoom
    println!("\n--- Zoom ---");
    match camera.zoom_position_inquiry() {
        Ok(zoom) => {
            println!("Zoom: {:?}", zoom);
            // Note: The inner value is not publicly accessible,
            // but the Debug format shows the hex value
        }
        Err(e) => println!("Failed to get zoom: {e}"),
    }

    // Query focus
    println!("\n--- Focus ---");
    match camera.focus_mode_inquiry() {
        Ok(mode) => println!("Focus Mode: {mode:?}"),
        Err(e) => println!("Failed to get focus mode: {e}"),
    }

    match camera.focus_position_inquiry() {
        Ok(focus) => println!("Focus Position: {:?}", focus),
        Err(e) => println!("Failed to get focus position: {e}"),
    }

    // Query exposure
    println!("\n--- Exposure ---");
    match camera.exposure_mode_inquiry() {
        Ok(mode) => println!("Exposure Mode: {mode:?}"),
        Err(e) => println!("Failed to get exposure mode: {e}"),
    }

    match camera.iris_inquiry() {
        Ok(iris) => println!("Iris: {:?}", iris),
        Err(e) => println!("Failed to get iris: {e}"),
    }

    match camera.shutter_inquiry() {
        Ok(speed) => println!("Shutter: {:?}", speed),
        Err(e) => println!("Failed to get shutter: {e}"),
    }

    match camera.gain_inquiry() {
        Ok(gain) => println!("Gain: {:?}", gain),
        Err(e) => println!("Failed to get gain: {e}"),
    }

    // Query white balance
    println!("\n--- White Balance ---");
    match camera.white_balance_mode_inquiry() {
        Ok(mode) => println!("WB Mode: {mode:?}"),
        Err(e) => println!("Failed to get WB mode: {e}"),
    }

    match camera.color_temperature_inquiry() {
        Ok(temp) => println!("Color Temp: {temp}K"),
        Err(e) => println!("Failed to get color temp: {e}"),
    }

    // Query image adjustments
    println!("\n--- Image Adjustments ---");
    match camera.brightness_inquiry() {
        Ok(val) => println!("Brightness: {:?}", val),
        Err(e) => println!("Failed to get brightness: {e}"),
    }

    // contrast_inquiry(), saturation_inquiry(), and sharpness_inquiry() have been commented out pending verification
    // These are not standard VISCA inquiries according to the protocol documentation
    // match camera.contrast_inquiry() {
    //     Ok(val) => println!("Contrast: {:?}", val),
    //     Err(e) => println!("Failed to get contrast: {e}"),
    // }

    // match camera.saturation_inquiry() {
    //     Ok(val) => println!("Saturation: {:?}", val),
    //     Err(e) => println!("Failed to get saturation: {e}"),
    // }

    // match camera.sharpness_inquiry() {
    //     Ok(val) => println!("Sharpness: {:?}", val),
    //     Err(e) => println!("Failed to get sharpness: {e}"),
    // }

    match camera.hue_inquiry() {
        Ok(val) => println!("Hue: {:?}", val),
        Err(e) => println!("Failed to get hue: {e}"),
    }

    // Query flip status
    println!("\n--- Image Orientation ---");
    match camera.image_flip_inquiry() {
        Ok(mode) => println!("Flip Mode: {mode:?}"),
        Err(e) => println!("Failed to get flip mode: {e}"),
    }

    println!("\n✓ Inquiry demo completed!");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    use grafton_visca::{camera::profiles::PTZOpticsG2, CameraBuilder};

    env_logger::init();

    println!("=== Camera Inquiry Demo (Async) ===\n");

    // Get camera address from command line or use default
    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("Connecting to camera at {camera_addr}...");
    let camera = CameraBuilder::tokio_tcp(&camera_addr)
        .profile::<PTZOpticsG2>()
        .build()
        .await?;

    println!("\n--- Querying All States Concurrently ---");

    use tokio::join;

    // Run multiple inquiries concurrently for better performance
    let (
        power,
        position,
        zoom,
        focus_mode,
        focus_pos,
        exposure_mode,
        iris,
        shutter,
        gain,
        wb_mode,
        color_temp,
        brightness,
        contrast,
        sharpness,
        saturation,
        hue,
        flip,
    ) = join!(
        camera.power_inquiry(),
        camera.pan_tilt_position_inquiry(),
        camera.zoom_position_inquiry(),
        camera.focus_mode_inquiry(),
        camera.focus_position_inquiry(),
        camera.exposure_mode_inquiry(),
        camera.iris_inquiry(),
        camera.shutter_inquiry(),
        camera.gain_inquiry(),
        camera.white_balance_mode_inquiry(),
        camera.color_temperature_inquiry(),
        camera.brightness_inquiry(),
        camera.contrast_inquiry(),
        camera.sharpness_inquiry(),
        camera.saturation_inquiry(),
        camera.hue_inquiry(),
        camera.image_flip_inquiry(),
    );

    println!("\n--- Power State ---");
    match power {
        Ok(is_on) => println!("Power: {}", if is_on { "ON" } else { "OFF" }),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Position ---");
    match position {
        Ok((pan, tilt)) => {
            println!("Pan: {:?}", pan);
            println!("Tilt: {:?}", tilt);
            // Convert to degrees using the built-in methods
            println!("Pan: {:.2}°", pan.to_degrees());
            println!("Tilt: {:.2}°", tilt.to_degrees());
        }
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Zoom ---");
    match zoom {
        Ok(z) => {
            println!("Zoom: {:?}", z);
            // Note: The inner value is not publicly accessible,
            // but the Debug format shows the hex value
        }
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Focus ---");
    match focus_mode {
        Ok(mode) => println!("Focus Mode: {mode:?}"),
        Err(e) => println!("Failed: {e}"),
    }
    match focus_pos {
        Ok(pos) => println!("Focus Position: {:?}", pos),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Exposure ---");
    match exposure_mode {
        Ok(mode) => println!("Exposure Mode: {mode:?}"),
        Err(e) => println!("Failed: {e}"),
    }
    match iris {
        Ok(val) => println!("Iris: {:?}", val),
        Err(e) => println!("Failed: {e}"),
    }
    match shutter {
        Ok(val) => println!("Shutter: {:?}", val),
        Err(e) => println!("Failed: {e}"),
    }
    match gain {
        Ok(val) => println!("Gain: {:?}", val),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- White Balance ---");
    match wb_mode {
        Ok(mode) => println!("WB Mode: {mode:?}"),
        Err(e) => println!("Failed: {e}"),
    }
    match color_temp {
        Ok(temp) => println!("Color Temperature: {temp}K"),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Image Adjustments ---");
    match brightness {
        Ok(val) => println!("Brightness: {:?}", val),
        Err(e) => println!("Failed: {e}"),
    }
    match contrast {
        Ok(val) => println!("Contrast: {:?}", val),
        Err(e) => println!("Failed: {e}"),
    }
    match sharpness {
        Ok(val) => println!("Sharpness: {:?}", val),
        Err(e) => println!("Failed: {e}"),
    }
    match saturation {
        Ok(val) => println!("Saturation: {:?}", val),
        Err(e) => println!("Failed: {e}"),
    }
    match hue {
        Ok(val) => println!("Hue: {:?}", val),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Image Orientation ---");
    match flip {
        Ok(mode) => println!("Flip Mode: {mode:?}"),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n✓ Async inquiry demo completed!");
    println!("Note: Concurrent queries are much faster than sequential!");

    Ok(())
}

#[cfg(all(feature = "async", not(feature = "rt-tokio")))]
fn main() {
    println!("This example requires either blocking mode or tokio runtime:");
    println!("  cargo run --example camera_inquiry --no-default-features");
    println!("  cargo run --example camera_inquiry --features rt-tokio");
}
