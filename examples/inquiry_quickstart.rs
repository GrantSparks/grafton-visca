//! Camera inquiry quickstart demonstration.
//!
//! This example shows how to query camera state and settings using the VISCA protocol.
//! It demonstrates both blocking and async approaches to reading camera parameters:
//! - Power state and system information
//! - Position information (pan/tilt/zoom)
//! - Focus settings and position
//! - Exposure settings (mode, iris, shutter, gain)
//! - White balance and color temperature
//! - Image quality parameters
//! - Noise reduction levels
//!
//! The async version demonstrates concurrent inquiries for much faster data collection.
//!
//! Run with:
//! - Blocking: cargo run --example inquiry_quickstart
//! - Async (Tokio): cargo run --example inquiry_quickstart --features rt-tokio
//!
//! Set the CAMERA_IP environment variable to override the default (192.168.0.110):
//! CAMERA_IP=192.168.1.100 cargo run --example inquiry_quickstart

// Blocking implementation
#[cfg(not(feature = "async"))]
fn main() -> grafton_visca::Result<()> {
    use grafton_visca::{camera::profiles::GenericVisca, CameraBuilder};

    tracing_subscriber::fmt::init();

    println!("=== Camera Inquiry Quickstart (Blocking) ===\n");

    // Get camera IP from environment or use default
    let ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.110".to_string());
    let camera_addr = format!("{ip}:5678"); // Most profiles default to port 5678

    println!("Connecting to camera at {camera_addr}...");
    let camera = CameraBuilder::tcp(&camera_addr)
        .profile::<GenericVisca>() // Use GenericVisca for broadest compatibility
        // For specific cameras, you can use:
        // .profile::<PtzOpticsG2>() // PTZOptics cameras
        // .profile::<SonyEviD70>() // Sony EVI-D70
        .open()?;

    println!("\n--- System Information ---");
    match camera.get_power_state() {
        Ok(is_on) => {
            let state = if is_on { "ON" } else { "OFF" };
            println!("Power: {state}");
        }
        Err(e) => println!("Power: Failed - {e}"),
    }

    match camera.get_version() {
        Ok(version) => println!("Version: {version:?}"),
        Err(e) => println!("Version: Failed - {e}"),
    }

    match camera.get_resolution() {
        Ok(res) => println!("Resolution: {res:?}"),
        Err(e) => println!("Resolution: Failed - {e}"),
    }

    println!("\n--- Position ---");
    match camera.get_pan_tilt_position() {
        Ok(pos) => {
            println!("Pan: {:?}", pos.pan);
            println!("Tilt: {:?}", pos.tilt);
            // Note: Conversion to degrees depends on camera profile
        }
        Err(e) => println!("Pan/Tilt: Failed - {e}"),
    }

    match camera.get_zoom_position() {
        Ok(zoom) => {
            println!("Zoom: {:?}", zoom);
            // Note: The inner value is not publicly accessible,
            // but the Debug format shows the hex value
        }
        Err(e) => println!("Zoom: Failed - {e}"),
    }

    println!("\n--- Focus ---");
    match camera.get_focus_mode() {
        Ok(mode) => println!("Focus Mode: {mode:?}"),
        Err(e) => println!("Focus Mode: Failed - {e}"),
    }

    match camera.get_focus_position() {
        Ok(focus) => println!("Focus Position: {:?}", focus),
        Err(e) => println!("Focus Position: Failed - {e}"),
    }

    println!("\n--- Exposure ---");
    match camera.get_exposure_mode() {
        Ok(mode) => println!("Exposure Mode: {mode:?}"),
        Err(e) => println!("Exposure Mode: Failed - {e}"),
    }

    match camera.get_iris() {
        Ok(iris) => println!("Iris: {:?}", iris),
        Err(e) => println!("Iris: Failed - {e}"),
    }

    match camera.get_shutter() {
        Ok(speed) => println!("Shutter: {:?}", speed),
        Err(e) => println!("Shutter: Failed - {e}"),
    }

    match camera.get_gain() {
        Ok(gain) => println!("Gain: {:?}", gain),
        Err(e) => println!("Gain: Failed - {e}"),
    }

    println!("\n--- White Balance ---");
    match camera.get_white_balance_mode() {
        Ok(mode) => println!("WB Mode: {mode:?}"),
        Err(e) => println!("WB Mode: Failed - {e}"),
    }

    match camera.get_color_temperature() {
        Ok(temp) => println!("Color Temperature: {temp}K"),
        Err(e) => println!("Color Temperature: Failed - {e}"),
    }

    println!("\n--- Image Adjustments ---");
    match camera.get_saturation() {
        Ok(val) => println!("Saturation: {:?}", val),
        Err(e) => println!("Saturation: Failed - {e}"),
    }

    match camera.get_hue() {
        Ok(val) => println!("Hue: {:?}", val),
        Err(e) => println!("Hue: Failed - {e}"),
    }

    match camera.get_image_flip() {
        Ok(mode) => println!("Image Flip: {mode:?}"),
        Err(e) => println!("Image Flip: Failed - {e}"),
    }

    println!("\n--- Noise Reduction ---");
    match camera.get_noise_reduction_2d() {
        Ok(level) => println!("2D NR Level: {level:?}"),
        Err(e) => println!("2D NR: Failed - {e}"),
    }

    match camera.get_noise_reduction_3d() {
        Ok(level) => println!("3D NR Level: {level:?}"),
        Err(e) => println!("3D NR: Failed - {e}"),
    }

    println!("\n✓ Inquiry completed!");
    println!("Tip: Run with --features rt-tokio for faster concurrent queries!");

    Ok(())
}

// Async implementation with concurrent inquiries
#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    use tokio::time::Instant;

    use grafton_visca::{
        camera::{
            controls::inquiry::{InquiryControl, PanTiltInquiryControl},
            profiles::GenericVisca,
        },
        runtime_adapters::tokio::TcpTransport as Tcp,
        runtime_trait::TokioRuntime,
        CameraBuilder,
    };

    tracing_subscriber::fmt::init();

    println!("=== Camera Inquiry Quickstart (Async/Concurrent) ===\n");

    // Get camera IP from environment or use default
    let ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.110".to_string());
    let camera_addr = format!("{ip}:5678"); // Most profiles default to port 5678

    println!("Connecting to camera at {camera_addr}...");
    let transport = Tcp::connect(&camera_addr).await?;
    let runtime = TokioRuntime::from_current()?;
    let camera = CameraBuilder::with_executor(runtime)
        .open_async::<GenericVisca, _>(transport) // Use GenericVisca for broadest compatibility
        // For specific cameras, you can use:
        // .open_async::<PtzOpticsG2, _>(transport) // PTZOptics cameras
        // .open_async::<SonyEviD70, _>(transport) // Sony EVI-D70
        .await?;

    println!("\n⚡ Executing all inquiries concurrently...\n");

    let start = Instant::now();

    // Execute all inquiries concurrently using tokio::join!
    // This is much faster than sequential queries
    let (
        power,
        version,
        resolution,
        pan_tilt,
        zoom,
        focus_mode,
        focus_pos,
        exposure_mode,
        iris,
        shutter,
        gain,
        wb_mode,
        color_temp,
        saturation,
        hue,
        flip,
        nr_2d,
        nr_3d,
    ) = tokio::join!(
        camera.get_power_state(),
        camera.get_version(),
        camera.get_resolution(),
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
        camera.get_saturation(),
        camera.get_hue(),
        camera.get_image_flip(),
        camera.get_noise_reduction_2d(),
        camera.get_noise_reduction_3d(),
    );

    let elapsed = start.elapsed();

    // Display results
    println!("--- System Information ---");
    match power {
        Ok(is_on) => println!("Power: {}", if is_on { "ON" } else { "OFF" }),
        Err(e) => println!("Power: Failed - {e}"),
    }
    if let Ok(ver) = version {
        println!("Version: {ver:?}");
    }
    if let Ok(res) = resolution {
        println!("Resolution: {res:?}");
    }

    println!("\n--- Position ---");
    if let Ok(pos) = pan_tilt {
        println!("Pan: {:?}", pos.pan);
        println!("Tilt: {:?}", pos.tilt);
    }
    if let Ok(z) = zoom {
        println!("Zoom: {z:?}");
    }

    println!("\n--- Focus ---");
    if let Ok(mode) = focus_mode {
        println!("Focus Mode: {mode:?}");
    }
    if let Ok(pos) = focus_pos {
        println!("Focus Position: {pos:?}");
    }

    println!("\n--- Exposure ---");
    if let Ok(mode) = exposure_mode {
        println!("Exposure Mode: {mode:?}");
    }
    if let Ok(i) = iris {
        println!("Iris: {i:?}");
    }
    if let Ok(s) = shutter {
        println!("Shutter: {s:?}");
    }
    if let Ok(g) = gain {
        println!("Gain: {g:?}");
    }

    println!("\n--- White Balance ---");
    if let Ok(mode) = wb_mode {
        println!("WB Mode: {mode:?}");
    }
    if let Ok(temp) = color_temp {
        println!("Color Temperature: {temp}K");
    }

    println!("\n--- Image Adjustments ---");
    if let Ok(sat) = saturation {
        println!("Saturation: {sat:?}");
    }
    if let Ok(h) = hue {
        println!("Hue: {h:?}");
    }
    if let Ok(f) = flip {
        println!("Image Flip: {f:?}");
    }

    println!("\n--- Noise Reduction ---");
    if let Ok(nr) = nr_2d {
        println!("2D NR Level: {nr:?}");
    }
    if let Ok(nr) = nr_3d {
        println!("3D NR Level: {nr:?}");
    }

    println!("\n✓ All inquiries completed in {:.2?}!", elapsed);
    println!("🚀 Concurrent execution is much faster than sequential!");

    Ok(())
}

// Handle unsupported configurations
#[cfg(all(feature = "async", not(feature = "rt-tokio")))]
fn main() {
    println!("This example requires either blocking mode or tokio runtime:");
    println!("  cargo run --example inquiry_quickstart");
    println!("  cargo run --example inquiry_quickstart --features rt-tokio");
}
