//! Test program demonstrating the comprehensive Camera API
//!
//! This example shows how the Camera API provides all control methods
//! directly without needing extension traits.

mod common;
use common::blocking::UdpTransport;

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::{
        exposure::ExposureMode, image::ImageFlipMode, pan_tilt::PanTiltDirection,
        white_balance::WhiteBalanceMode,
    },
    transport::BlockingAdapter,
    Error,
};
use std::thread;
use std::time::Duration;

// Helper for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn main() -> Result<(), Error> {
    env_logger::init();

    // Create a camera with the new API
    let transport = common::blocking::udp_transport("192.168.1.100:5678")?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    println!("=== Testing Comprehensive Camera API ===\n");

    // Power control
    println!("Testing power control...");
    block_on(camera.power_on())?;
    thread::sleep(Duration::from_secs(2));

    // Movement control
    println!("\nTesting movement control...");
    block_on(camera.home())?;
    thread::sleep(Duration::from_secs(2));

    block_on(camera.move_continuous(PanTiltDirection::Right, 10, 0))?;
    thread::sleep(Duration::from_millis(500));
    block_on(camera.stop())?;

    // Zoom control
    println!("\nTesting zoom control...");
    block_on(camera.set_zoom(0x2000))?;
    thread::sleep(Duration::from_secs(1));

    block_on(camera.zoom_in())?;
    thread::sleep(Duration::from_millis(500));
    block_on(camera.zoom_stop())?;

    // Preset management
    println!("\nTesting preset management...");
    use grafton_visca::camera::profiles::G2PresetId;
    let preset = G2PresetId::new(1)?;
    block_on(camera.set_preset(preset))?;
    thread::sleep(Duration::from_millis(500));

    block_on(camera.home())?;
    thread::sleep(Duration::from_secs(2));

    block_on(camera.recall_preset(preset))?;
    thread::sleep(Duration::from_secs(2));

    // Focus control
    println!("\nTesting focus control...");
    block_on(camera.focus_auto())?;
    thread::sleep(Duration::from_millis(500));

    block_on(camera.focus_manual())?;
    block_on(camera.set_focus(0x5000))?;
    thread::sleep(Duration::from_millis(500));

    block_on(camera.focus_auto())?;

    // Exposure control
    println!("\nTesting exposure control...");
    block_on(camera.set_exposure_mode(ExposureMode::Auto))?;
    block_on(camera.set_iris(10))?;
    block_on(camera.set_shutter(15))?;
    block_on(camera.backlight_on())?;
    thread::sleep(Duration::from_millis(500));
    block_on(camera.backlight_off())?;

    // Image quality control
    println!("\nTesting image quality control...");
    block_on(camera.set_brightness(8))?;
    block_on(camera.set_contrast(8))?;
    block_on(camera.set_sharpness(8))?;
    block_on(camera.set_saturation(8))?;
    block_on(camera.set_hue(7))?;

    // White balance control
    println!("\nTesting white balance control...");
    block_on(camera.set_white_balance_mode(WhiteBalanceMode::Auto))?;
    thread::sleep(Duration::from_millis(500));

    block_on(camera.set_white_balance_mode(WhiteBalanceMode::Indoor))?;
    thread::sleep(Duration::from_millis(500));

    block_on(camera.set_white_balance_mode(WhiteBalanceMode::Outdoor))?;
    thread::sleep(Duration::from_millis(500));

    block_on(camera.one_push_white_balance())?;

    // Advanced image features
    println!("\nTesting advanced image features...");
    block_on(camera.set_noise_reduction_2d(3))?;
    block_on(camera.set_noise_reduction_3d(2))?;
    block_on(camera.set_image_flip(ImageFlipMode::Off))?;
    block_on(camera.black_white_off())?;

    // Position control with different unit types
    println!("\nTesting position control with different units...");
    use grafton_visca::camera::units::{Degrees, Normalized, ViscaUnits};

    // Using degrees
    block_on(camera.set_position(Degrees(45.0), Degrees(15.0)))?;
    thread::sleep(Duration::from_secs(2));

    // Using VISCA units
    block_on(camera.set_position_units(ViscaUnits(1000), ViscaUnits(500)))?;
    thread::sleep(Duration::from_secs(2));

    // Using normalized coordinates
    block_on(camera.set_position_normalized(Normalized(0.0), Normalized(0.0)))?;
    thread::sleep(Duration::from_secs(2));

    // Gain control with profile-specific values
    println!("\nTesting gain control...");
    use grafton_visca::camera::profiles::G2Gain;
    block_on(camera.set_gain(G2Gain::Gain0dB))?;
    block_on(camera.set_gain(G2Gain::Gain12dB))?;
    block_on(camera.set_gain_limit(4))?; // 12dB limit

    // Dynamic range and color temperature
    println!("\nTesting dynamic range and color temperature...");
    block_on(camera.set_dynamic_range(5))?;
    block_on(camera.set_color_temperature(0x20))?;

    println!("\n=== All Camera API Methods Tested Successfully! ===");
    println!("\nKey advantages over extension traits:");
    println!("• All methods directly on Camera struct");
    println!("• Type-safe with camera profile constraints");
    println!("• No trait imports needed");
    println!("• Consistent async API");
    println!("• Profile-specific types for presets and gain");

    Ok(())
}
