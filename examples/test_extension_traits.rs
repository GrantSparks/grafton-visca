//! Test program demonstrating the comprehensive Camera API
//!
//! This example shows how the Camera API provides all control methods
//! directly without needing extension traits.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::{
        exposure::ExposureMode, image::ImageFlipMode, pan_tilt::PanTiltDirection,
        white_balance::WhiteBalanceMode,
    },
    transport::create,
    types::{
        BrightnessLevel, ContrastLevel, FocusPosition, HueLevel, IrisLevel,
        NoiseReduction2DLevel, NoiseReduction3DLevel, SaturationLevel, SharpnessLevel,
        ShutterSpeed, ZoomPosition,
    },
    Error,
};
use std::time::Duration;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    // Create a camera with the new API
    let transport = create::udp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    println!("=== Testing Comprehensive Camera API ===\n");

    // Power control
    println!("Testing power control...");
    camera.power_on().await?;
    time::sleep(Duration::from_secs(2)).await;

    // Movement control
    println!("\nTesting movement control...");
    camera.home().await?;
    time::sleep(Duration::from_secs(2)).await;

    camera
        .move_continuous(PanTiltDirection::Right, 10, 0)
        .await?;
    time::sleep(Duration::from_millis(500)).await;
    camera.stop().await?;

    // Zoom control
    println!("\nTesting zoom control...");
    camera.set_zoom(ZoomPosition::new(0x2000)?).await?;
    time::sleep(Duration::from_secs(1)).await;

    camera.zoom_in().await?;
    time::sleep(Duration::from_millis(500)).await;
    camera.zoom_stop().await?;

    // Preset management
    println!("\nTesting preset management...");
    use grafton_visca::camera::profiles::G2PresetId;
    let preset = G2PresetId::new(1)?;
    camera.set_preset(preset).await?;
    time::sleep(Duration::from_millis(500)).await;

    camera.home().await?;
    time::sleep(Duration::from_secs(2)).await;

    camera.recall_preset(preset).await?;
    time::sleep(Duration::from_secs(2)).await;

    // Focus control
    println!("\nTesting focus control...");
    camera.focus_auto().await?;
    time::sleep(Duration::from_millis(500)).await;

    camera.focus_manual().await?;
    camera.set_focus(FocusPosition::new(0x5000)?).await?;
    time::sleep(Duration::from_millis(500)).await;

    camera.focus_auto().await?;

    // Exposure control
    println!("\nTesting exposure control...");
    camera.set_exposure_mode(ExposureMode::Auto).await?;
    camera.set_iris(IrisLevel::new(10)?).await?;
    camera.set_shutter(ShutterSpeed::new(15)?).await?;
    camera.backlight_on().await?;
    time::sleep(Duration::from_millis(500)).await;
    camera.backlight_off().await?;

    // Image quality control
    println!("\nTesting image quality control...");
    camera.set_brightness(BrightnessLevel::new(8)?).await?;
    camera.set_contrast(ContrastLevel::new(8)?).await?;
    camera.set_sharpness(SharpnessLevel::new(8)?).await?;
    camera.set_saturation(SaturationLevel::new(8)?).await?;
    camera.set_hue(HueLevel::new(7)?).await?;

    // White balance control
    println!("\nTesting white balance control...");
    camera
        .set_white_balance_mode(WhiteBalanceMode::Auto)
        .await?;
    time::sleep(Duration::from_millis(500)).await;

    camera
        .set_white_balance_mode(WhiteBalanceMode::Indoor)
        .await?;
    time::sleep(Duration::from_millis(500)).await;

    camera
        .set_white_balance_mode(WhiteBalanceMode::Outdoor)
        .await?;
    time::sleep(Duration::from_millis(500)).await;

    camera.one_push_white_balance().await?;

    // Advanced image features
    println!("\nTesting advanced image features...");
    camera.set_noise_reduction_2d(NoiseReduction2DLevel::new(3)?).await?;
    camera.set_noise_reduction_3d(NoiseReduction3DLevel::new(2)?).await?;
    camera.set_image_flip(ImageFlipMode::Off).await?;
    camera.black_white_off().await?;

    // Position control with different unit types
    println!("\nTesting position control with different units...");
    use grafton_visca::camera::units::{Degrees, Normalized, ViscaUnits};

    // Using degrees
    camera.set_position(Degrees(45.0), Degrees(15.0)).await?;
    time::sleep(Duration::from_secs(2)).await;

    // Using VISCA units
    camera
        .set_position_units(ViscaUnits(1000), ViscaUnits(500))
        .await?;
    time::sleep(Duration::from_secs(2)).await;

    // Using normalized coordinates
    camera
        .set_position_normalized(Normalized(0.0), Normalized(0.0))
        .await?;
    time::sleep(Duration::from_secs(2)).await;

    // Gain control with profile-specific values
    println!("\nTesting gain control...");
    use grafton_visca::camera::profiles::G2Gain;
    camera.set_gain(G2Gain::Gain0dB).await?;
    camera.set_gain(G2Gain::Gain12dB).await?;
    camera.set_gain_limit(4).await?; // 12dB limit

    // Dynamic range and color temperature
    println!("\nTesting dynamic range and color temperature...");
    camera.set_dynamic_range(5).await?;
    camera.set_color_temperature(0x20).await?;

    println!("\n=== All Camera API Methods Tested Successfully! ===");
    println!("\nKey advantages over extension traits:");
    println!("• All methods directly on Camera struct");
    println!("• Type-safe with camera profile constraints");
    println!("• No trait imports needed");
    println!("• Consistent async API");
    println!("• Profile-specific types for presets and gain");

    Ok(())
}
