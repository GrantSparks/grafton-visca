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
    use grafton_visca::{
        camera::{
            methods::inquiry::{InquiryControlBlocking, PanTiltInquiryControlBlocking},
            profiles::PtzOpticsG2,
        },
        CameraBuilder,
    };

    tracing_subscriber::fmt::init();

    println!("=== Camera Inquiry Demo (Blocking) ===\n");

    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:5678".to_string());

    println!("Connecting to camera at {camera_addr}...");
    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<PtzOpticsG2>()
        .build()?;

    println!("\n--- Power State ---");
    match camera.get_power_state() {
        Ok(is_on) => {
            let state = if is_on { "ON" } else { "OFF" };
            println!("Power: {state}");
        }
        Err(e) => println!("Failed to get power state: {e}"),
    }

    println!("\n--- Position ---");
    match camera.get_pan_tilt_position() {
        Ok((pan, tilt)) => {
            println!("Pan: {:?}", pan);
            println!("Tilt: {:?}", tilt);

            println!("Pan: {:.2}°", pan.to_degrees());
            println!("Tilt: {:.2}°", tilt.to_degrees());
        }
        Err(e) => println!("Failed to get position: {e}"),
    }

    println!("\n--- Zoom ---");
    match camera.get_zoom_position() {
        Ok(zoom) => {
            println!("Zoom: {:?}", zoom);
            // Note: The inner value is not publicly accessible,
            // but the Debug format shows the hex value
        }
        Err(e) => println!("Failed to get zoom: {e}"),
    }

    println!("\n--- Focus ---");
    match camera.get_focus_mode() {
        Ok(mode) => println!("Focus Mode: {mode:?}"),
        Err(e) => println!("Failed to get focus mode: {e}"),
    }

    match camera.get_focus_position() {
        Ok(focus) => println!("Focus Position: {:?}", focus),
        Err(e) => println!("Failed to get focus position: {e}"),
    }

    println!("\n--- Exposure ---");
    match camera.get_exposure_mode() {
        Ok(mode) => println!("Exposure Mode: {mode:?}"),
        Err(e) => println!("Failed to get exposure mode: {e}"),
    }

    match camera.get_iris() {
        Ok(iris) => println!("Iris: {:?}", iris),
        Err(e) => println!("Failed to get iris: {e}"),
    }

    match camera.get_shutter() {
        Ok(speed) => println!("Shutter: {:?}", speed),
        Err(e) => println!("Failed to get shutter: {e}"),
    }

    match camera.get_gain() {
        Ok(gain) => println!("Gain: {:?}", gain),
        Err(e) => println!("Failed to get gain: {e}"),
    }

    println!("\n--- White Balance ---");
    match camera.get_white_balance_mode() {
        Ok(mode) => println!("WB Mode: {mode:?}"),
        Err(e) => println!("Failed to get WB mode: {e}"),
    }

    match camera.get_color_temperature() {
        Ok(temp) => println!("Color Temp: {temp}K"),
        Err(e) => println!("Failed to get color temp: {e}"),
    }

    println!("\n--- Image Adjustments ---");
    match camera.get_brightness() {
        Ok(val) => println!("Brightness: {:?}", val),
        Err(e) => println!("Failed to get brightness: {e}"),
    }

    match camera.get_hue() {
        Ok(val) => println!("Hue: {:?}", val),
        Err(e) => println!("Failed to get hue: {e}"),
    }

    println!("\n--- Image Orientation ---");
    match camera.get_image_flip() {
        Ok(mode) => println!("Flip Mode: {mode:?}"),
        Err(e) => println!("Failed to get flip mode: {e}"),
    }

    println!("\n✓ Inquiry demo completed!");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    use grafton_visca::{
        camera::{
            controls::inquiry::{InquiryControl, PanTiltInquiryControl},
            profiles::PtzOpticsG2,
        },
        runtime_adapters::tokio::TcpTransport as Tcp,
        CameraBuilder,
    };

    tracing_subscriber::fmt::init();

    println!("=== Camera Inquiry Demo (Async) ===\n");

    let camera_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:5678".to_string());

    println!("Connecting to camera at {camera_addr}...");
    let transport = Tcp::connect(&camera_addr).await?;
    let camera = CameraBuilder::tokio()?
        .build_async::<PtzOpticsG2, _>(transport)
        .await?;

    println!("\n--- Querying All States Concurrently ---");

    use tokio::join;
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
        saturation,
        hue,
        flip,
    ) = join!(
        camera.get_power_state(),
        camera.get_pan_tilt_position(),
        camera.get_zoom_position(),
        camera.get_focus_mode(),
        camera.get_focus_position(),
        camera.get_exposure_mode(),
        camera.get_iris(),
        camera.get_shutter(),
        camera.get_gain(),
        camera.get_white_balance_mode(),
        camera.get_color_temperature(),
        camera.get_brightness(),
        camera.get_saturation(),
        camera.get_hue(),
        camera.get_image_flip(),
    );

    println!("\n--- Power State ---");
    match power {
        Ok(is_on) => println!("Power: {}", if is_on { "ON" } else { "OFF" }),
        Err(e) => println!("Failed: {e}"),
    }

    println!("\n--- Position ---");
    match position {
        Ok(pos) => {
            println!("Pan raw value: 0x{:04X}", pos.pan as u16);
            println!("Tilt raw value: 0x{:04X}", pos.tilt as u16);
            // Note: Pan/tilt units vary by camera model
            // Some use signed values (0x0000 = center), others use unsigned (0x8000 = center)
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
