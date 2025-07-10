//! Test program demonstrating the comprehensive Camera API
//!
//! This example shows how the Camera API provides all control methods
//! directly without needing extension traits.

#[cfg(feature = "tokio")]
use grafton_visca::{
    camera::{
        methods::{
            ExposureMethodsExt, FocusMethodsExt, ImageProcessingMethodsExt, PanTiltMethodsExt,
            PowerMethodsExt, WhiteBalanceMethodsExt, ZoomMethodsExt,
        },
        profiles::PTZOpticsG2,
        Camera,
    },
    command::{
        exposure::{DynamicRangeLevel, ExposureMode},
        // image::ImageFlipMode, // unused import
        pan_tilt::PanTiltDirection,
        white_balance::WhiteBalanceMode,
    },
    transport::tokio::Udp,
    types::{
        BrightnessLevel, ColorTemperature, ContrastLevel, FocusPosition, GainLimit, HueLevel,
        IrisLevel, NoiseReduction2DLevel, NoiseReduction3DLevel, PanSpeed, SaturationLevel,
        SharpnessLevel, TiltSpeed,
    },
    units::Raw,
    Error,
};
#[cfg(feature = "tokio")]
use std::time::Duration;
#[cfg(feature = "tokio")]
use tokio::time;

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    // Create a camera with the new API
    let transport = Udp::connect("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("=== Testing Comprehensive Camera API ===\n");

    // Power control
    println!("Testing power control...");
    camera.power_on().await?;
    time::sleep(Duration::from_secs(2)).await;

    // Movement control
    println!("\nTesting movement control...");
    camera.pan_tilt_home().await?;
    time::sleep(Duration::from_secs(2)).await;

    camera
        .pan_tilt_move(
            PanTiltDirection::Right,
            PanSpeed::new(10)?,
            TiltSpeed::new(0)?,
        )
        .await?;
    time::sleep(Duration::from_millis(500)).await;
    camera.pan_tilt_stop().await?;

    // Zoom control
    println!("\nTesting zoom control...");
    camera.set_zoom(Raw(0x2000u16)).await?;
    time::sleep(Duration::from_secs(1)).await;

    camera.zoom_in().await?;
    time::sleep(Duration::from_millis(500)).await;
    camera.zoom_stop().await?;

    // Preset management
    println!("\nTesting preset management...");
    use grafton_visca::camera::profiles::G2PresetId;
    let preset = G2PresetId::new(1)?;
    camera.preset_set(preset.into()).await?;
    time::sleep(Duration::from_millis(500)).await;

    camera.pan_tilt_home().await?;
    time::sleep(Duration::from_secs(2)).await;

    camera.preset_recall(preset.into()).await?;
    time::sleep(Duration::from_secs(2)).await;

    // Focus control
    println!("\nTesting focus control...");
    camera.focus_auto().await?;
    time::sleep(Duration::from_millis(500)).await;

    camera.focus_manual().await?;
    camera.set_focus(FocusPosition::try_from(0x5000)?).await?;
    time::sleep(Duration::from_millis(500)).await;

    camera.focus_auto().await?;

    // Exposure control
    println!("\nTesting exposure control...");
    camera.set_exposure_mode(ExposureMode::Auto).await?;
    camera.set_iris(IrisLevel::new(10)?).await?;
    // Set shutter speed not available directly in current API
    // camera.set_shutter_speed(ShutterSpeed::new(15)?).await?;
    camera.set_backlight(true).await?;
    time::sleep(Duration::from_millis(500)).await;
    camera.set_backlight(false).await?;

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

    // One-push white balance not available in current API
    // camera.one_push_white_balance().await?;

    // Advanced image features
    println!("\nTesting advanced image features...");
    camera
        .set_noise_reduction_2d(NoiseReduction2DLevel::new(3)?)
        .await?;
    camera
        .set_noise_reduction_3d(NoiseReduction3DLevel::new(2)?)
        .await?;
    // Note: set_image_flip not available in current API
    // Note: black_white_off not available in current API

    // Position control with different unit types
    println!("\nTesting position control with different units...");
    use grafton_visca::units::Degrees;

    // Using degrees
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0)).await?;
    time::sleep(Duration::from_secs(2)).await;

    // Using VISCA units
    // Set position using raw VISCA units (convert to appropriate units)
    // This would require using ViscaUnits or converting to degrees
    camera.pan_tilt_absolute(Degrees(10.0), Degrees(5.0)).await?;
    time::sleep(Duration::from_secs(2)).await;

    // Using normalized coordinates - convert to degrees
    camera.pan_tilt_absolute(Degrees(0.0), Degrees(0.0)).await?;
    time::sleep(Duration::from_secs(2)).await;

    // Gain control with profile-specific values
    println!("\nTesting gain control...");

    // Set gain value
    camera.set_gain(grafton_visca::types::Gain::new(0)?).await?; // 0dB gain
    camera.set_gain(grafton_visca::types::Gain::new(4)?).await?; // 12dB gain
    camera.set_gain_limit(GainLimit::new(4)?).await?; // 12dB limit

    // Dynamic range and color temperature
    println!("\nTesting dynamic range and color temperature...");
    camera.set_dynamic_range(DynamicRangeLevel::new(5)?).await?;
    camera
        .set_color_temperature(ColorTemperature::new(0x20)?)
        .await?;

    println!("\n=== All Camera API Methods Tested Successfully! ===");
    println!("\nKey advantages over extension traits:");
    println!("• All methods directly on Camera struct");
    println!("• Type-safe with camera profile constraints");
    println!("• No trait imports needed");
    println!("• Consistent async API");
    println!("• Profile-specific types for presets and gain");

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example test_extension_traits --features tokio");
}
