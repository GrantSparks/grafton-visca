//! Example demonstrating image quality and gain control features.

use grafton_visca::{
    command::AntiFlickerMode, ViscaClient, ViscaError, ViscaExposureExt, ViscaImageExt,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), ViscaError> {
    // Initialize logger for debugging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();

    // Create client and connect to camera
    let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;

    println!("Connected to camera. Demonstrating image and gain controls...");

    // Anti-flicker control
    println!("\n--- Anti-Flicker Control ---");
    client.set_anti_flicker(AntiFlickerMode::Off)?;
    println!("Anti-flicker: OFF");
    thread::sleep(Duration::from_secs(1));

    client.set_anti_flicker(AntiFlickerMode::Hz60)?;
    println!("Anti-flicker: 60Hz (for US/Canada)");
    thread::sleep(Duration::from_secs(1));

    // Luminance control
    println!("\n--- Luminance Control ---");
    client.set_luminance(7)?;
    println!("Luminance: Default (7)");
    thread::sleep(Duration::from_secs(1));

    client.set_luminance(10)?;
    println!("Luminance: Bright (10)");
    thread::sleep(Duration::from_secs(1));

    client.set_luminance(4)?;
    println!("Luminance: Dark (4)");
    thread::sleep(Duration::from_secs(1));

    // Contrast control
    println!("\n--- Contrast Control ---");
    client.set_contrast(7)?;
    println!("Contrast: Default (7)");
    thread::sleep(Duration::from_secs(1));

    client.set_contrast(12)?;
    println!("Contrast: High (12)");
    thread::sleep(Duration::from_secs(1));

    // Sharpness control
    println!("\n--- Sharpness Control ---");
    client.set_sharpness(7)?;
    println!("Sharpness: Default (7)");
    thread::sleep(Duration::from_secs(1));

    client.sharpness_up()?;
    println!("Sharpness: Up");
    thread::sleep(Duration::from_secs(1));

    client.sharpness_down()?;
    client.sharpness_down()?;
    println!("Sharpness: Down x2");
    thread::sleep(Duration::from_secs(1));

    // Gain control
    println!("\n--- Gain Control ---");
    client.set_gain(0x00)?;
    println!("Gain: Minimum (0 dB)");
    thread::sleep(Duration::from_secs(1));

    client.gain_up()?;
    client.gain_up()?;
    println!("Gain: Up x2");
    thread::sleep(Duration::from_secs(1));

    client.set_gain_limit(0x08)?;
    println!("Gain Limit: Set to 24 dB");
    thread::sleep(Duration::from_secs(1));

    // Apply an image preset
    println!("\n--- Image Presets ---");
    client.apply_image_preset(grafton_visca::ImagePreset::Cinema)?;
    println!("Applied Cinema preset");
    thread::sleep(Duration::from_secs(2));

    client.apply_image_preset(grafton_visca::ImagePreset::Default)?;
    println!("Applied Default preset");

    println!("\nDemo complete! All image and gain controls have been demonstrated.");

    Ok(())
}