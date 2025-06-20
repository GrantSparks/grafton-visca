//! Example demonstrating the user-friendly enums for camera control
//!
//! This example shows how to use SpeedLevel, FStop, and NoiseReductionStrength
//! enums for more intuitive camera control.

#[cfg(not(feature = "async"))]
use grafton_visca::transport::blocking::create;
#[cfg(not(feature = "async"))]
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::{exposure::ExposureMode, pan_tilt::PanTiltDirection},
    types::{FStop, NoiseReductionStrength, SpeedLevel},
};
#[cfg(not(feature = "async"))]
use std::time::Duration;

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create camera with default address
    let transport = create::udp("192.168.1.100:1259")?;
    let mut camera: Camera<PTZOpticsG2> = Camera::new(transport);

    // Example 1: Using SpeedLevel for intuitive movement control
    println!("=== Speed Level Demo ===");

    // Move slowly for precise positioning
    let slow_pan = SpeedLevel::Slow.to_pan_speed();
    let slow_tilt = SpeedLevel::Slow.to_tilt_speed();
    camera.move_continuous(PanTiltDirection::UpRight, slow_pan, slow_tilt)?;
    std::thread::sleep(Duration::from_millis(500));
    camera.stop()?;

    // Move fast for quick repositioning
    let fast_pan = SpeedLevel::Fast.to_pan_speed();
    let fast_tilt = SpeedLevel::Fast.to_tilt_speed();
    camera.move_continuous(PanTiltDirection::DownLeft, fast_pan, fast_tilt)?;
    std::thread::sleep(Duration::from_millis(500));
    camera.stop()?;

    // Example 2: Using FStop for iris control
    println!("\n=== F-Stop Demo ===");

    // Set to manual exposure mode first
    camera.set_exposure_mode(ExposureMode::Manual)?;

    // Set specific F-stop values
    camera.set_iris(grafton_visca::types::IrisLevel::new(
        FStop::F2_8.to_iris_level(),
    )?)?;
    println!("Iris set to {}", FStop::F2_8);
    std::thread::sleep(Duration::from_secs(1));

    camera.set_iris(grafton_visca::types::IrisLevel::new(
        FStop::F5_6.to_iris_level(),
    )?)?;
    println!("Iris set to {}", FStop::F5_6);
    std::thread::sleep(Duration::from_secs(1));

    // Example 3: Using NoiseReductionStrength
    println!("\n=== Noise Reduction Demo ===");

    // Set 2D noise reduction to medium
    let nr_2d_level = NoiseReductionStrength::Medium.to_2d_level()?;
    camera.set_noise_reduction_2d(grafton_visca::types::NoiseReduction2DLevel::new(
        nr_2d_level,
    )?)?;
    println!("2D Noise Reduction set to Medium (level {})", nr_2d_level);

    // Set 3D noise reduction to strong
    let nr_3d_level = NoiseReductionStrength::Strong.to_3d_level()?;
    camera.set_noise_reduction_3d(grafton_visca::types::NoiseReduction3DLevel::new(
        nr_3d_level,
    )?)?;
    println!("3D Noise Reduction set to Strong (level {})", nr_3d_level);

    // Example 4: Zoom with speed levels
    println!("\n=== Zoom Speed Demo ===");

    // The new API uses zoom_in/zoom_out without speed control
    // For demonstration, we'll use basic zoom methods
    camera.zoom_in()?;
    std::thread::sleep(Duration::from_millis(500));
    camera.zoom_stop()?;

    // Example 5: Reading back F-stop values
    println!("\n=== Reading F-Stop Value ===");

    let state = camera.get_camera_state()?;
    if let Some(iris) = state.exposure.iris {
        if let Some(iris_level) = FStop::from_iris_level(iris) {
            println!("Current iris setting: {}", iris_level);
        } else {
            println!("Current iris level: {}", iris);
        }
    } else {
        println!("Iris level not available");
    }

    println!("\nDemo completed successfully!");
    Ok(())
}

#[cfg(not(not(feature = "async")))]
fn main() {
    eprintln!("This example requires the blocking mode (default) feature to be enabled.");
    eprintln!("Run with: cargo run --example user_friendly_api_demo --no-default-features --features blocking-client");
}
