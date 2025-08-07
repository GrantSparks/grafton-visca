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

#[cfg(not(feature = "tokio"))]
fn main() -> grafton_visca::Result<()> {
    use grafton_visca::{camera::profiles::PTZOpticsG2, prelude::blocking::*, CameraBuilder};

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
    match camera.get_power_state() {
        Ok(is_on) => {
            let state = if is_on { "ON" } else { "OFF" };
            println!("Power: {state}");
        }
        Err(e) => println!("Failed to get power state: {e}"),
    }

    // Query position
    println!("\n--- Position ---");
    match camera.get_pan_tilt_position() {
        Ok((pan, tilt)) => {
            println!("Pan: {pan} units");
            println!("Tilt: {tilt} units");

            // Convert to degrees if profile supports it
            let pan_deg = pan as f32 * 0.0146484375;
            let tilt_deg = tilt as f32 * 0.0146484375;
            println!("Pan: {pan_deg:.2}°");
            println!("Tilt: {tilt_deg:.2}°");
        }
        Err(e) => println!("Failed to get position: {e}"),
    }

    // Query zoom
    println!("\n--- Zoom ---");
    match camera.get_zoom_position() {
        Ok(zoom) => {
            println!("Zoom: 0x{zoom:04X} (raw)");
            let zoom_pct = (zoom as f32 / 0x4000 as f32) * 100.0;
            println!("Zoom: {zoom_pct:.1}%");
        }
        Err(e) => println!("Failed to get zoom: {e}"),
    }

    // Query focus
    println!("\n--- Focus ---");
    match camera.get_focus_mode() {
        Ok(mode) => println!("Focus Mode: {mode:?}"),
        Err(e) => println!("Failed to get focus mode: {e}"),
    }

    match camera.get_focus_position() {
        Ok(focus) => println!("Focus Position: 0x{focus:04X}"),
        Err(e) => println!("Failed to get focus position: {e}"),
    }

    // Query exposure
    println!("\n--- Exposure ---");
    match camera.get_exposure_mode() {
        Ok(mode) => println!("Exposure Mode: {mode:?}"),
        Err(e) => println!("Failed to get exposure mode: {e}"),
    }

    match camera.get_iris_position() {
        Ok(iris) => println!("Iris: F{iris}"),
        Err(e) => println!("Failed to get iris: {e}"),
    }

    match camera.get_shutter_speed() {
        Ok(speed) => println!("Shutter: 1/{speed}"),
        Err(e) => println!("Failed to get shutter: {e}"),
    }

    match camera.get_gain() {
        Ok(gain) => println!("Gain: {gain} dB"),
        Err(e) => println!("Failed to get gain: {e}"),
    }

    // Query white balance
    println!("\n--- White Balance ---");
    match camera.get_white_balance_mode() {
        Ok(mode) => println!("WB Mode: {mode:?}"),
        Err(e) => println!("Failed to get WB mode: {e}"),
    }

    match camera.get_color_temperature() {
        Ok(temp) => println!("Color Temp: {temp}K"),
        Err(e) => println!("Failed to get color temp: {e}"),
    }

    // Query image adjustments
    println!("\n--- Image Adjustments ---");
    match camera.get_brightness() {
        Ok(val) => println!("Brightness: {val}/100"),
        Err(e) => println!("Failed to get brightness: {e}"),
    }

    match camera.get_contrast() {
        Ok(val) => println!("Contrast: {val}/100"),
        Err(e) => println!("Failed to get contrast: {e}"),
    }

    match camera.get_saturation() {
        Ok(val) => println!("Saturation: {val}/100"),
        Err(e) => println!("Failed to get saturation: {e}"),
    }

    match camera.get_sharpness() {
        Ok(val) => println!("Sharpness: {val}/100"),
        Err(e) => println!("Failed to get sharpness: {e}"),
    }

    match camera.get_hue() {
        Ok(val) => println!("Hue: {val}°"),
        Err(e) => println!("Failed to get hue: {e}"),
    }

    // Query flip status
    println!("\n--- Image Orientation ---");
    match camera.get_flip_horizontal() {
        Ok(flipped) => println!("H-Flip: {}", if flipped { "ON" } else { "OFF" }),
        Err(e) => println!("Failed to get H-flip: {e}"),
    }

    match camera.get_flip_vertical() {
        Ok(flipped) => println!("V-Flip: {}", if flipped { "ON" } else { "OFF" }),
        Err(e) => println!("Failed to get V-flip: {e}"),
    }

    println!("\n✓ Inquiry demo completed!");

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    use grafton_visca::{camera::profiles::PTZOpticsG2, prelude::r#async::*, CameraBuilder};

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

    let (power, position, zoom, focus_mode, exposure_mode, wb_mode) = join!(
        camera.get_power_state(),
        camera.get_pan_tilt_position(),
        camera.get_zoom_position(),
        camera.get_focus_mode(),
        camera.get_exposure_mode(),
        camera.get_white_balance_mode()
    );

    println!("\n--- Power State ---");
    match power {
        Ok(is_on) => println!("Power: {}", if is_on { "ON" } else { "OFF" }),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Position ---");
    match position {
        Ok((pan, tilt)) => {
            println!("Pan: {pan} units");
            println!("Tilt: {tilt} units");
            let pan_deg = pan as f32 * 0.0146484375;
            let tilt_deg = tilt as f32 * 0.0146484375;
            println!("Pan: {pan_deg:.2}°");
            println!("Tilt: {tilt_deg:.2}°");
        }
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Zoom ---");
    match zoom {
        Ok(z) => {
            println!("Zoom: 0x{z:04X} (raw)");
            let zoom_pct = (z as f32 / 0x4000 as f32) * 100.0;
            println!("Zoom: {zoom_pct:.1}%");
        }
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Focus ---");
    match focus_mode {
        Ok(mode) => println!("Focus Mode: {mode:?}"),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Exposure ---");
    match exposure_mode {
        Ok(mode) => println!("Exposure Mode: {mode:?}"),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- White Balance ---");
    match wb_mode {
        Ok(mode) => println!("WB Mode: {mode:?}"),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Detailed Image Settings ---");

    if let Ok(brightness) = camera.get_brightness().await {
        println!("Brightness: {brightness}/100");
    }

    if let Ok(contrast) = camera.get_contrast().await {
        println!("Contrast: {contrast}/100");
    }

    if let Ok(saturation) = camera.get_saturation().await {
        println!("Saturation: {saturation}/100");
    }

    if let Ok(sharpness) = camera.get_sharpness().await {
        println!("Sharpness: {sharpness}/100");
    }

    if let Ok(hue) = camera.get_hue().await {
        println!("Hue: {hue}°");
    }

    println!("\n✓ Async inquiry demo completed!");
    println!("Note: Concurrent queries are much faster than sequential!");

    Ok(())
}
