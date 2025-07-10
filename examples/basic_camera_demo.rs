//! Basic camera control demonstration.
//!
//! This example demonstrates common camera operations using the async API.

use grafton_visca::{
    camera::{
        methods::{
            ExposureMethodsExt, ImageProcessingMethodsExt, PanTiltMethodsExt, PowerMethodsExt,
            WhiteBalanceMethodsExt, ZoomMethodsExt,
        },
        profiles::PTZOpticsG2,
        Camera,
    },
    command::{
        exposure::ExposureMode, pan_tilt::PanTiltDirection, white_balance::WhiteBalanceMode,
    },
    transport::tokio::Tcp,
    types::{PanSpeed, TiltSpeed},
    units::Degrees,
    units::Raw,
    Error,
};
use std::time::Duration;
use tokio::time::sleep;

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature.");
    eprintln!("Run with: cargo run --example basic_camera_demo --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🎥 Basic Camera Demo");
    println!("====================\n");

    // Create async transport
    let transport = Tcp::connect_timeout("192.168.1.100:5678", Duration::from_secs(5)).await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Demo 1: Power Control
    demo_power_control(&camera).await?;

    // Demo 2: Pan/Tilt Movement
    demo_pan_tilt_movement(&camera).await?;

    // Demo 3: Zoom Control
    demo_zoom_control(&camera).await?;

    // Demo 4: Focus Control
    demo_focus_control(&camera).await?;

    // Demo 5: Exposure Settings
    demo_exposure_settings(&camera).await?;

    // Demo 6: White Balance
    demo_white_balance(&camera).await?;

    // Demo 7: Position Control
    demo_position_control(&camera).await?;

    println!("\n✅ All demos completed successfully!");
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_power_control<T: grafton_visca::transport::AsyncTransport>(
    camera: &Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    println!("📍 Demo 1: Power Control");
    println!("Powering on camera...");
    camera.power_on().await?;
    println!("✅ Camera powered on successfully");
    sleep(Duration::from_secs(2)).await;
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_pan_tilt_movement<T: grafton_visca::transport::AsyncTransport>(
    camera: &Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    println!("\n📍 Demo 2: Pan/Tilt Movement");

    println!("Moving to home position...");
    camera.pan_tilt_home().await?;
    sleep(Duration::from_secs(2)).await;

    println!("Moving camera up-right...");
    camera
        .move_continuous(
            PanTiltDirection::UpRight,
            PanSpeed::new(16)?,
            TiltSpeed::new(16)?,
        )
        .await?;
    sleep(Duration::from_secs(1)).await;

    println!("Stopping movement...");
    camera.pan_tilt_stop().await?;
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_zoom_control<T: grafton_visca::transport::AsyncTransport>(
    camera: &Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    println!("\n📍 Demo 3: Zoom Control");

    println!("Zooming in...");
    camera.zoom_in().await?;
    sleep(Duration::from_secs(1)).await;

    println!("Stopping zoom...");
    camera.zoom_stop().await?;

    println!("Setting zoom to 50%...");
    camera.set_zoom(Raw(0x3800u16)).await?; // Mid-range zoom
    sleep(Duration::from_secs(1)).await;

    println!("Resetting zoom...");
    camera.set_zoom(Raw(0x0000u16)).await?;
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_focus_control<T: grafton_visca::transport::AsyncTransport>(
    camera: &Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    println!("\n📍 Demo 4: Focus Control");

    println!("Setting auto-focus mode...");
    camera.focus_auto().await?;
    sleep(Duration::from_millis(500)).await;

    println!("Switching to manual focus...");
    camera.focus_manual().await?;
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_exposure_settings<T: grafton_visca::transport::AsyncTransport>(
    camera: &Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    println!("\n📍 Demo 5: Exposure Settings");

    println!("Setting exposure to auto...");
    camera.set_exposure_mode(ExposureMode::Auto).await?;
    sleep(Duration::from_millis(500)).await;

    println!("Switching to shutter priority mode...");
    camera.set_exposure_mode(ExposureMode::Shutter).await?;
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_white_balance<T: grafton_visca::transport::AsyncTransport>(
    camera: &Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    println!("\n📍 Demo 6: White Balance");

    println!("Setting white balance to auto...");
    camera
        .set_white_balance_mode(WhiteBalanceMode::Auto)
        .await?;
    sleep(Duration::from_millis(500)).await;

    println!("Switching to indoor mode...");
    camera
        .set_white_balance_mode(WhiteBalanceMode::Indoor)
        .await?;
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_position_control<T: grafton_visca::transport::AsyncTransport>(
    camera: &Camera<PTZOpticsG2, T>,
) -> Result<(), Error> {
    println!("\n📍 Demo 7: Position Control");

    println!("Moving to specific position (45°, 20°)...");
    camera
        .pan_tilt_absolute(Degrees(45.0), Degrees(20.0))
        .await?;
    sleep(Duration::from_secs(2)).await;

    println!("Moving to position (-30°, -10°)...");
    camera
        .pan_tilt_absolute(Degrees(-30.0), Degrees(-10.0))
        .await?;
    sleep(Duration::from_secs(2)).await;

    println!("Returning to home...");
    camera.pan_tilt_home().await?;
    sleep(Duration::from_secs(2)).await;
    Ok(())
}
