//! Example program

//! Example demonstrating image quality and gain control features.

use grafton_visca::{
    camera::{
        profiles::{G2Gain, PTZOpticsG2},
        Camera,
    },
    command::gain::AntiFlickerMode,
    transport::create,
    types::{ContrastLevel, GainLimit, LuminanceLevel, SharpnessLevel},
    Error,
};
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logger for debugging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Debug)
        .init();

    // Create camera with PTZOptics G2 profile
    let transport = create::udp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    println!("Connected to camera. Demonstrating image and gain controls...");

    // Anti-flicker control
    println!("\n--- Anti-Flicker Control ---");
    camera.set_anti_flicker(AntiFlickerMode::Off).await?;
    println!("Anti-flicker: OFF");
    time::sleep(Duration::from_secs(1)).await;

    camera.set_anti_flicker(AntiFlickerMode::Hz60).await?;
    println!("Anti-flicker: 60Hz (for US/Canada)");
    time::sleep(Duration::from_secs(1)).await;

    // Luminance control
    println!("\n--- Luminance Control ---");
    camera.set_luminance(LuminanceLevel::new(7)?).await?;
    println!("Luminance: Default (7)");
    time::sleep(Duration::from_secs(1)).await;

    camera.set_luminance(LuminanceLevel::new(10)?).await?;
    println!("Luminance: Bright (10)");
    time::sleep(Duration::from_secs(1)).await;

    camera.set_luminance(LuminanceLevel::new(4)?).await?;
    println!("Luminance: Dark (4)");
    time::sleep(Duration::from_secs(1)).await;

    // Contrast control
    println!("\n--- Contrast Control ---");
    camera.set_contrast(ContrastLevel::new(7)?).await?;
    println!("Contrast: Default (7)");
    time::sleep(Duration::from_secs(1)).await;

    camera.set_contrast(ContrastLevel::new(12)?).await?;
    println!("Contrast: High (12)");
    time::sleep(Duration::from_secs(1)).await;

    // Sharpness control
    println!("\n--- Sharpness Control ---");
    camera.set_sharpness(SharpnessLevel::new(7)?).await?;
    println!("Sharpness: Default (7)");
    time::sleep(Duration::from_secs(1)).await;

    camera.sharpness_up().await?;
    println!("Sharpness: Up");
    time::sleep(Duration::from_secs(1)).await;

    camera.sharpness_down().await?;
    camera.sharpness_down().await?;
    println!("Sharpness: Down x2");
    time::sleep(Duration::from_secs(1)).await;

    // Gain control
    println!("\n--- Gain Control ---");
    camera.set_gain(G2Gain::Gain0dB).await?;
    println!("Gain: Minimum (0 dB)");
    time::sleep(Duration::from_secs(1)).await;

    camera.gain_up().await?;
    camera.gain_up().await?;
    println!("Gain: Up x2");
    time::sleep(Duration::from_secs(1)).await;

    camera.set_gain_limit(GainLimit::new(0x08)?).await?; // 0x08 corresponds to 24dB limit
    println!("Gain Limit: Set to 24 dB");
    time::sleep(Duration::from_secs(1)).await;

    println!("\nDemo complete! All image and gain controls have been demonstrated.");

    Ok(())
}
