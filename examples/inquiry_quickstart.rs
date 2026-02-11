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
//! ## Concurrent Inquiry Behavior
//!
//! The async version fires inquiries via `tokio::join!`, but actual concurrency depends
//! on the camera protocol:
//!
//! - **Sony cameras** (SonyFR7, SonyBRCH900): Use sequence-numbered protocol, allowing
//!   true concurrent execution. All inquiries run in parallel for maximum speed.
//!
//! - **Raw VISCA cameras** (GenericVisca, PtzOpticsG2): No sequence numbers, so the
//!   runtime automatically serializes inquiries to ensure reliable response matching.
//!   `tokio::join!` still provides clean async semantics, but execution is sequential.
//!
//! Run with:
//! - Blocking: cargo run --example inquiry_quickstart
//! - Async (Tokio): cargo run --example inquiry_quickstart --features runtime-tokio
//!
//! Set the CAMERA_IP environment variable to override the default (192.168.0.110):
//! CAMERA_IP=192.168.1.100 cargo run --example inquiry_quickstart

// Blocking implementation
#[cfg(not(feature = "mode-async"))]
fn main() -> grafton_visca::Result<()> {
    use grafton_visca::camera::{profiles::PtzOpticsG2, Connect};

    tracing_subscriber::fmt::init();

    println!("=== Camera Inquiry Quickstart (Blocking) ===\n");

    // Get camera IP from environment or use default
    let camera_addr = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.110".to_string());

    println!("Connecting to camera at {camera_addr}...");
    let camera = Connect::open_tcp_blocking::<PtzOpticsG2>(camera_addr)?;

    // PTZOptics cameras support a subset of VISCA inquiries.
    // This example queries only the commonly-supported ones.

    println!("\n--- Power ---");
    match camera.power_state() {
        Ok(is_on) => {
            let state = if is_on { "ON" } else { "OFF" };
            println!("Power: {state}");
        }
        Err(e) => println!("Power: Failed - {e}"),
    }

    println!("\n--- Position ---");
    match camera.pan_tilt_position() {
        Ok(pos) => {
            println!("Pan: {:?}", pos.pan);
            println!("Tilt: {:?}", pos.tilt);

            use grafton_visca::inquiry_conversions::PanTiltPositionRaw;
            let raw = PanTiltPositionRaw::new(pos.pan, pos.tilt);
            let degrees = raw.as_degrees();
            println!(
                "  → Pan: {:.1}°, Tilt: {:.1}°",
                degrees.pan.0, degrees.tilt.0
            );
        }
        Err(e) => println!("Pan/Tilt: Failed - {e}"),
    }

    match camera.zoom_position() {
        Ok(zoom) => {
            println!("Zoom: {:?}", zoom);

            use grafton_visca::{inquiry_conversions::ZoomDomain, ZoomPositionExt};
            // PtzOpticsG2 profile constants
            let optical_max = 0x4000u16;
            let digital_max = Some(0x7000u16);
            let optical = zoom.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max);
            let full =
                zoom.normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max);
            println!(
                "  → Optical: {:.1}%, Full range: {:.1}%",
                optical.0 * 100.0,
                full.0 * 100.0
            );
        }
        Err(e) => println!("Zoom: Failed - {e}"),
    }

    println!("\n--- Focus ---");
    match camera.focus_mode() {
        Ok(mode) => println!("Focus Mode: {mode:?}"),
        Err(e) => println!("Focus Mode: Failed - {e}"),
    }

    match camera.focus_position() {
        Ok(focus) => println!("Focus Position: {:?}", focus),
        Err(e) => println!("Focus Position: Failed - {e}"),
    }

    println!("\n--- Exposure ---");
    match camera.exposure_mode() {
        Ok(mode) => println!("Exposure Mode: {mode:?}"),
        Err(e) => println!("Exposure Mode: Failed - {e}"),
    }

    println!("\n--- White Balance ---");
    match camera.white_balance_mode() {
        Ok(mode) => println!("WB Mode: {mode:?}"),
        Err(e) => println!("WB Mode: Failed - {e}"),
    }

    println!("\n✓ Inquiry completed!");

    Ok(())
}

// Async implementation with concurrent inquiries
#[cfg(feature = "runtime-tokio")]
#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    use tokio::time::Instant;

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Connect},
        runtime::TokioRuntime,
    };

    tracing_subscriber::fmt::init();

    println!("=== Camera Inquiry Quickstart (Async) ===\n");

    // Get camera IP from environment or use default
    let camera_addr = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.110".to_string());

    println!("Connecting to camera at {camera_addr}...");
    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&camera_addr, runtime).await?;

    // Note: PtzOpticsG2 uses Raw VISCA protocol without sequence numbers,
    // so inquiries are automatically serialized by the runtime for reliable
    // response matching. For true concurrent execution, use a Sony camera
    // profile (SonyFR7, SonyBRCH900) which supports sequence-based correlation.
    //
    // PTZOptics cameras support a subset of VISCA inquiries. This example
    // queries only the commonly-supported ones.
    println!("\n⚡ Executing inquiries (serialized for PtzOpticsG2)...\n");

    let start = Instant::now();

    let power_acc = camera.power();
    let pan_tilt_acc = camera.pan_tilt();
    let zoom_acc = camera.zoom();
    let focus_acc = camera.focus();
    let exposure_acc = camera.exposure();
    let white_balance_acc = camera.white_balance();

    // Query only PTZOptics-supported inquiries
    let (power, pan_tilt, zoom, focus_mode, focus_pos, exposure_mode, wb_mode) = tokio::join!(
        power_acc.state(),
        pan_tilt_acc.position(),
        zoom_acc.position(),
        focus_acc.mode(),
        focus_acc.position(),
        exposure_acc.mode(),
        white_balance_acc.mode(),
    );

    let elapsed = start.elapsed();

    // Display results
    println!("--- Power ---");
    match power {
        Ok(is_on) => {
            let power_state = if is_on { "ON" } else { "OFF" };
            println!("Power: {power_state}");
        }
        Err(e) => println!("Power: Failed - {e}"),
    }

    println!("\n--- Position ---");
    match pan_tilt {
        Ok(pos) => {
            println!("Pan: {:?}", pos.pan);
            println!("Tilt: {:?}", pos.tilt);

            use grafton_visca::inquiry_conversions::PanTiltPositionRaw;
            let raw = PanTiltPositionRaw::new(pos.pan, pos.tilt);
            let degrees = raw.as_degrees();
            println!(
                "  → Pan: {:.1}°, Tilt: {:.1}°",
                degrees.pan.0, degrees.tilt.0
            );
        }
        Err(e) => println!("Pan/Tilt: Failed - {e}"),
    }

    match zoom {
        Ok(z) => {
            println!("Zoom: {z:?}");

            use grafton_visca::{inquiry_conversions::ZoomDomain, ZoomPositionExt};
            // PtzOpticsG2 profile constants
            let optical_max = 0x4000u16;
            let digital_max = Some(0x7000u16);
            let optical = z.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max);
            let full =
                z.normalize_with_max(ZoomDomain::OpticalPlusDigital, optical_max, digital_max);
            println!(
                "  → Optical: {:.1}%, Full range: {:.1}%",
                optical.0 * 100.0,
                full.0 * 100.0
            );
        }
        Err(e) => println!("Zoom: Failed - {e}"),
    }

    println!("\n--- Focus ---");
    match focus_mode {
        Ok(mode) => println!("Focus Mode: {mode:?}"),
        Err(e) => println!("Focus Mode: Failed - {e}"),
    }
    match focus_pos {
        Ok(pos) => println!("Focus Position: {pos:?}"),
        Err(e) => println!("Focus Position: Failed - {e}"),
    }

    println!("\n--- Exposure ---");
    match exposure_mode {
        Ok(mode) => println!("Exposure Mode: {mode:?}"),
        Err(e) => println!("Exposure Mode: Failed - {e}"),
    }

    println!("\n--- White Balance ---");
    match wb_mode {
        Ok(mode) => println!("WB Mode: {mode:?}"),
        Err(e) => println!("WB Mode: Failed - {e}"),
    }

    println!("\n✓ All inquiries completed in {:.2?}!", elapsed);
    println!("Note: PtzOpticsG2 serializes inquiries. Sony cameras run truly concurrent.");

    Ok(())
}

// Handle unsupported configurations
#[cfg(all(feature = "mode-async", not(feature = "runtime-tokio")))]
fn main() {
    println!("This example requires either blocking mode or tokio runtime:");
    println!("  cargo run --example inquiry_quickstart");
    println!("  cargo run --example inquiry_quickstart --features runtime-tokio");
}
