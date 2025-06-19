//! Example demonstrating the user-friendly enums for camera control
//!
//! This example shows how to use SpeedLevel, FStop, and NoiseReductionStrength
//! enums for more intuitive camera control.

mod common;
use common::blocking::UdpTransport;

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::{exposure::ExposureMode, pan_tilt::PanTiltDirection},
    transport::BlockingAdapter,
    types::{FStop, NoiseReductionStrength, SpeedLevel},
};
use std::time::Duration;

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Create camera with default address
    let transport = UdpTransport::new("192.168.1.100:1259")?;
    let camera: Camera<PTZOpticsG2> = Camera::new(BlockingAdapter(transport));

    // Example 1: Using SpeedLevel for intuitive movement control
    println!("=== Speed Level Demo ===");

    // Move slowly for precise positioning
    let slow_pan = SpeedLevel::Slow.to_pan_speed();
    let slow_tilt = SpeedLevel::Slow.to_tilt_speed();
    block_on(camera.move_continuous(PanTiltDirection::UpRight, slow_pan, slow_tilt))?;
    std::thread::sleep(Duration::from_millis(500));
    block_on(camera.stop())?;

    // Move fast for quick repositioning
    let fast_pan = SpeedLevel::Fast.to_pan_speed();
    let fast_tilt = SpeedLevel::Fast.to_tilt_speed();
    block_on(camera.move_continuous(PanTiltDirection::DownLeft, fast_pan, fast_tilt))?;
    std::thread::sleep(Duration::from_millis(500));
    block_on(camera.stop())?;

    // Example 2: Using FStop for iris control
    println!("\n=== F-Stop Demo ===");

    // Set to manual exposure mode first
    block_on(camera.set_exposure_mode(ExposureMode::Manual))?;

    // Set specific F-stop values
    block_on(camera.set_iris(FStop::F2_8.to_iris_level()))?;
    println!("Iris set to {}", FStop::F2_8);
    std::thread::sleep(Duration::from_secs(1));

    block_on(camera.set_iris(FStop::F5_6.to_iris_level()))?;
    println!("Iris set to {}", FStop::F5_6);
    std::thread::sleep(Duration::from_secs(1));

    // Example 3: Using NoiseReductionStrength
    println!("\n=== Noise Reduction Demo ===");

    // Set 2D noise reduction to medium
    let nr_2d_level = NoiseReductionStrength::Medium.to_2d_level()?;
    block_on(camera.set_noise_reduction_2d(nr_2d_level))?;
    println!("2D Noise Reduction set to Medium (level {})", nr_2d_level);

    // Set 3D noise reduction to strong
    let nr_3d_level = NoiseReductionStrength::Strong.to_3d_level()?;
    block_on(camera.set_noise_reduction_3d(nr_3d_level))?;
    println!("3D Noise Reduction set to Strong (level {})", nr_3d_level);

    // Example 4: Zoom with speed levels
    println!("\n=== Zoom Speed Demo ===");

    // The new API uses zoom_in/zoom_out without speed control
    // For demonstration, we'll use basic zoom methods
    block_on(camera.zoom_in())?;
    std::thread::sleep(Duration::from_millis(500));
    block_on(camera.zoom_stop())?;

    // Example 5: Reading back F-stop values
    println!("\n=== Reading F-Stop Value ===");

    let state = block_on(camera.get_camera_state())?;
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
