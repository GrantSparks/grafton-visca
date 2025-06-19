//! Example program

//! Example demonstrating image quality and gain control features.

use grafton_visca::{
    camera::{
        profiles::{G2Gain, PTZOpticsG2},
        Camera,
    },
    command::AntiFlickerMode,
    transport::create,
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
    let udp_transport = UdpTransport::new("192.168.1.100:5678")?;
    let camera = Camera::<PTZOpticsG2>::new(udp_transport);

    println!("Connected to camera. Demonstrating image and gain controls...");

    // Anti-flicker control
    println!("\n--- Anti-Flicker Control ---");
    block_on(camera.set_anti_flicker(AntiFlickerMode::Off))?;
    println!("Anti-flicker: OFF");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.set_anti_flicker(AntiFlickerMode::Hz60))?;
    println!("Anti-flicker: 60Hz (for US/Canada)");
    thread::sleep(Duration::from_secs(1));

    // Luminance control
    println!("\n--- Luminance Control ---");
    block_on(camera.set_luminance(7))?;
    println!("Luminance: Default (7)");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.set_luminance(10))?;
    println!("Luminance: Bright (10)");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.set_luminance(4))?;
    println!("Luminance: Dark (4)");
    thread::sleep(Duration::from_secs(1));

    // Contrast control
    println!("\n--- Contrast Control ---");
    block_on(camera.set_contrast(7))?;
    println!("Contrast: Default (7)");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.set_contrast(12))?;
    println!("Contrast: High (12)");
    thread::sleep(Duration::from_secs(1));

    // Sharpness control
    println!("\n--- Sharpness Control ---");
    block_on(camera.set_sharpness(7))?;
    println!("Sharpness: Default (7)");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.sharpness_up())?;
    println!("Sharpness: Up");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.sharpness_down())?;
    block_on(camera.sharpness_down())?;
    println!("Sharpness: Down x2");
    thread::sleep(Duration::from_secs(1));

    // Gain control
    println!("\n--- Gain Control ---");
    block_on(camera.set_gain(G2Gain::Gain0dB))?;
    println!("Gain: Minimum (0 dB)");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.gain_up())?;
    block_on(camera.gain_up())?;
    println!("Gain: Up x2");
    thread::sleep(Duration::from_secs(1));

    block_on(camera.set_gain_limit(0x08))?; // 0x08 corresponds to 24dB limit
    println!("Gain Limit: Set to 24 dB");
    thread::sleep(Duration::from_secs(1));

    println!("\nDemo complete! All image and gain controls have been demonstrated.");

    Ok(())
}
