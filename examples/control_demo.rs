//! Camera control demonstration.
//!
//! This example demonstrates the high-level control API for camera operations
//! using both blocking and async interfaces.
//!
//! Usage: cargo run --example control_demo [--features tokio] <camera_ip:port>

#[cfg(not(feature = "tokio"))]
use grafton_visca::{
    blocking::{Camera, FocusOps, PanTiltOps, PresetsOps, ZoomOps},
    command::preset::PresetNumber,
    types::SpeedLevel,
    CameraModel, Degrees, Error, Normalized,
};

#[cfg(feature = "tokio")]
use grafton_visca::{
    command::preset::PresetNumber,
    r#async::{FocusOps, PanTiltOps, PresetsOps, ZoomOps},
    types::SpeedLevel,
    Camera, CameraModel, Degrees, Error, Normalized,
};
use std::env;

#[cfg(not(feature = "tokio"))]
use grafton_visca::transport::blocking::Udp;

#[cfg(feature = "tokio")]
use grafton_visca::transport::tokio::Udp;

#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Error> {
    env_logger::init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <camera_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:5678", args[0]);
        std::process::exit(1);
    }

    let camera_addr = &args[1];
    println!("Connecting to camera at {} (blocking mode)...", camera_addr);

    let transport = Udp::connect(camera_addr)?;
    let mut camera =
        grafton_visca::Camera::with_profile(CameraModel::PTZOpticsG2, transport).blocking();

    println!("\n=== Camera Control Demo (Blocking) ===");
    println!("Using profile: PTZOpticsG2\n");

    // Pan/Tilt Control
    demonstrate_pan_tilt(&mut camera)?;

    // Zoom Control
    demonstrate_zoom(&mut camera)?;

    // Focus Control
    demonstrate_focus(&mut camera)?;

    // Preset Control
    demonstrate_presets(&mut camera)?;

    println!("\n✅ Demo complete!");
    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <camera_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:5678", args[0]);
        std::process::exit(1);
    }

    let camera_addr = &args[1];
    println!("Connecting to camera at {} (async mode)...", camera_addr);

    let transport = Udp::connect(camera_addr).await?;
    let mut camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);

    println!("\n=== Camera Control Demo (Async) ===");
    println!("Using profile: PTZOpticsG2\n");

    // Pan/Tilt Control
    demonstrate_pan_tilt(&mut camera).await?;

    // Zoom Control
    demonstrate_zoom(&mut camera).await?;

    // Focus Control
    demonstrate_focus(&mut camera).await?;

    // Preset Control
    demonstrate_presets(&mut camera).await?;

    println!("\n✅ Demo complete!");
    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn demonstrate_pan_tilt(camera: &mut Camera) -> Result<(), Error> {
    use std::{thread, time::Duration};

    println!("1. Pan/Tilt Control");
    println!("   - Moving to home position...");
    camera.pan_tilt_home()?;
    thread::sleep(Duration::from_secs(3));

    println!("   - Moving to absolute position (20°, -10°)...");
    camera.pan_tilt_absolute(Degrees(20.0), Degrees(-10.0), SpeedLevel::from(10))?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Relative movement (pan right, tilt up)...");
    camera.pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::from(15))?;
    thread::sleep(Duration::from_secs(2));

    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_pan_tilt(camera: &mut Camera) -> Result<(), Error> {
    use tokio::time::{sleep, Duration};

    println!("1. Pan/Tilt Control");
    println!("   - Moving to home position...");
    camera.pan_tilt_home().await?;
    sleep(Duration::from_secs(3)).await;

    println!("   - Moving to absolute position (20°, -10°)...");
    camera
        .pan_tilt_absolute(Degrees(20.0), Degrees(-10.0), SpeedLevel::from(10))
        .await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Relative movement (pan right, tilt up)...");
    camera
        .pan_tilt_relative(Degrees(10.0), Degrees(5.0), SpeedLevel::from(15))
        .await?;
    sleep(Duration::from_secs(2)).await;

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn demonstrate_zoom(camera: &mut Camera) -> Result<(), Error> {
    use std::{thread, time::Duration};

    println!("\n2. Zoom Control");

    println!("   - Zooming to 50%...");
    camera.zoom_absolute(Normalized::new(0.5))?;
    thread::sleep(Duration::from_secs(2));

    println!("   - Zooming in...");
    camera.zoom_in()?;
    thread::sleep(Duration::from_secs(1));
    camera.zoom_stop()?;

    println!("   - Zooming out...");
    camera.zoom_out()?;
    thread::sleep(Duration::from_secs(1));
    camera.zoom_stop()?;

    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_zoom(camera: &mut Camera) -> Result<(), Error> {
    use tokio::time::{sleep, Duration};

    println!("\n2. Zoom Control");

    println!("   - Zooming to 50%...");
    camera.zoom_absolute(Normalized::new(0.5)).await?;
    sleep(Duration::from_secs(2)).await;

    println!("   - Zooming in...");
    camera.zoom_in().await?;
    sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;

    println!("   - Zooming out...");
    camera.zoom_out().await?;
    sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn demonstrate_focus(camera: &mut Camera) -> Result<(), Error> {
    use std::{thread, time::Duration};

    println!("\n3. Focus Control");

    // Note: supports_capability not available on blocking Camera
    // Assume focus is supported
    if true {
        println!("   - Enabling auto-focus...");
        camera.focus_auto()?;
        thread::sleep(Duration::from_secs(2));

        println!("   - Manual focus adjustment...");
        camera.focus_manual()?;
        camera.focus_near(SpeedLevel::from(3))?;
        thread::sleep(Duration::from_secs(1));
        camera.focus_stop()?;
    } else {
        println!("   - Focus not supported by this camera profile");
    }

    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_focus(camera: &mut Camera) -> Result<(), Error> {
    use tokio::time::{sleep, Duration};

    println!("\n3. Focus Control");

    // Note: supports_capability not available on blocking Camera
    // Assume focus is supported
    if true {
        println!("   - Enabling auto-focus...");
        camera.focus_auto().await?;
        sleep(Duration::from_secs(2)).await;

        println!("   - Manual focus adjustment...");
        camera.focus_manual().await?;
        camera.focus_near(SpeedLevel::from(3)).await?;
        sleep(Duration::from_secs(1)).await;
        camera.focus_stop().await?;
    } else {
        println!("   - Focus not supported by this camera profile");
    }

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn demonstrate_presets(camera: &mut Camera) -> Result<(), Error> {
    use std::{thread, time::Duration};

    println!("\n4. Preset Control");

    // Note: supports_capability not available on blocking Camera
    // Assume presets are supported
    if true {
        println!("   - Saving current position as preset 1...");
        camera.preset_set(PresetNumber::new(1)?)?;
        thread::sleep(Duration::from_millis(500));

        println!("   - Moving to a different position...");
        camera.pan_tilt_absolute(Degrees(-20.0), Degrees(6.0), SpeedLevel::from(10))?;
        camera.zoom_absolute(Normalized::new(0.75))?;
        thread::sleep(Duration::from_secs(3));

        println!("   - Saving as preset 2...");
        camera.preset_set(PresetNumber::new(2)?)?;
        thread::sleep(Duration::from_millis(500));

        println!("   - Returning to home...");
        camera.pan_tilt_home()?;
        thread::sleep(Duration::from_secs(3));

        println!("   - Recalling preset 1...");
        camera.preset_recall(PresetNumber::new(1)?)?;
        thread::sleep(Duration::from_secs(3));

        println!("   - Recalling preset 2...");
        camera.preset_recall(PresetNumber::new(2)?)?;
        thread::sleep(Duration::from_secs(3));
    } else {
        println!("   - Presets not supported by this camera profile");
    }

    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_presets(camera: &mut Camera) -> Result<(), Error> {
    use tokio::time::{sleep, Duration};

    println!("\n4. Preset Control");

    // Note: supports_capability not available on blocking Camera
    // Assume presets are supported
    if true {
        println!("   - Saving current position as preset 1...");
        camera.preset_set(PresetNumber::new(1)?).await?;
        sleep(Duration::from_millis(500)).await;

        println!("   - Moving to a different position...");
        camera
            .pan_tilt_absolute(Degrees(-20.0), Degrees(6.0), SpeedLevel::from(10))
            .await?;
        camera.zoom_absolute(Normalized::new(0.75)).await?;
        sleep(Duration::from_secs(3)).await;

        println!("   - Saving as preset 2...");
        camera.preset_set(PresetNumber::new(2)?).await?;
        sleep(Duration::from_millis(500)).await;

        println!("   - Returning to home...");
        camera.pan_tilt_home().await?;
        sleep(Duration::from_secs(3)).await;

        println!("   - Recalling preset 1...");
        camera.preset_recall(PresetNumber::new(1)?).await?;
        sleep(Duration::from_secs(3)).await;

        println!("   - Recalling preset 2...");
        camera.preset_recall(PresetNumber::new(2)?).await?;
        sleep(Duration::from_secs(3)).await;
    } else {
        println!("   - Presets not supported by this camera profile");
    }

    Ok(())
}
