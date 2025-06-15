//! Basic camera control demonstration using the new Camera API

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::{
        exposure::ExposureMode, gain::AntiFlickerMode, image::ImageFlipMode,
        pan_tilt::PanTiltDirection, white_balance::WhiteBalanceMode,
    },
    transport::{BlockingAdapter, UdpTransport},
};
use std::thread;
use std::time::Duration;

// Use a minimal tokio runtime for blocking execution
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn demo_power_control(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 1: Power Control");
    println!("Powering on camera...");
    block_on(camera.power_on())?;
    println!("✅ Camera powered on successfully");
    thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn demo_pan_tilt_movement(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 2: Pan/Tilt Movement");

    println!("Moving to home position...");
    block_on(camera.home())?;
    thread::sleep(Duration::from_secs(2));

    println!("Moving camera up-right...");
    block_on(camera.move_continuous(PanTiltDirection::UpRight, 16, 16))?;
    thread::sleep(Duration::from_secs(1));

    println!("Stopping movement...");
    block_on(camera.stop())?;
    Ok(())
}

fn demo_zoom_control(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 3: Zoom Control");

    println!("Zooming in...");
    block_on(camera.zoom_in())?;
    thread::sleep(Duration::from_secs(1));

    block_on(camera.zoom_stop())?;

    println!("Zooming out...");
    block_on(camera.zoom_out())?;
    thread::sleep(Duration::from_secs(1));

    block_on(camera.zoom_stop())?;

    println!("Setting zoom to 25%...");
    block_on(camera.set_zoom(0x1C00))?; // 25% of G2's max zoom
    Ok(())
}

fn demo_focus_control(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 4: Focus Control");

    println!("Setting auto focus...");
    block_on(camera.focus_auto())?;
    thread::sleep(Duration::from_secs(1));

    println!("Setting manual focus...");
    block_on(camera.focus_manual())?;
    block_on(camera.set_focus(0x6000))?;
    thread::sleep(Duration::from_secs(1));

    println!("Returning to auto focus...");
    block_on(camera.focus_auto())?;
    Ok(())
}

fn demo_preset_positions(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::camera::profiles::G2PresetId;

    println!("\n📍 Demo 5: Preset Positions");

    println!("Saving current position as preset 1...");
    let preset1 = G2PresetId::new(1)?;
    block_on(camera.set_preset(preset1))?;
    thread::sleep(Duration::from_secs(1));

    println!("Moving camera to a different position...");
    block_on(camera.move_continuous(PanTiltDirection::DownLeft, 16, 16))?;
    thread::sleep(Duration::from_secs(1));
    block_on(camera.stop())?;

    println!("Recalling preset 1...");
    block_on(camera.recall_preset(preset1))?;
    thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn demo_exposure_control(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 6: Exposure Control");

    println!("Setting manual exposure mode...");
    block_on(camera.set_exposure_mode(ExposureMode::Manual))?;

    println!("Adjusting iris to F4.0...");
    block_on(camera.set_iris(6))?;

    println!("Setting shutter speed...");
    block_on(camera.set_shutter(10))?;

    println!("Returning to auto exposure...");
    block_on(camera.set_exposure_mode(ExposureMode::Auto))?;
    Ok(())
}

fn demo_color_adjustments(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 7: Color Adjustments");

    println!("Setting white balance to auto...");
    block_on(camera.set_white_balance_mode(WhiteBalanceMode::Auto))?;

    println!("Adjusting saturation...");
    block_on(camera.set_saturation(8))?;

    println!("Adjusting hue...");
    block_on(camera.set_hue(7))?;

    println!("Setting color temperature to 5600K...");
    block_on(camera.set_color_temperature(0x20))?; // Approximate 5600K
    Ok(())
}

fn demo_image_quality(camera: &mut Camera<PTZOpticsG2>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 8: Image Quality Settings");

    println!("Setting luminance...");
    block_on(camera.set_luminance(8))?;

    println!("Setting contrast...");
    block_on(camera.set_contrast(8))?;

    println!("Setting sharpness...");
    block_on(camera.set_sharpness(8))?;

    println!("Setting brightness...");
    block_on(camera.set_brightness(8))?;
    Ok(())
}

fn demo_camera_info(camera: &Camera<PTZOpticsG2>) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 9: Camera Information");

    let caps = camera.capabilities();
    println!("Camera Model: {}", caps.model_name);
    println!("Pan Range: {:?} degrees", caps.pan_range_degrees);
    println!("Tilt Range: {:?} degrees", caps.tilt_range_degrees);
    println!("Zoom Steps: {}", caps.zoom_steps);
    println!("Focus Steps: {}", caps.focus_steps);
    println!("Preset Count: {}", caps.preset_count);
    println!("Supports Digital Zoom: {}", caps.supports_digital_zoom);
    println!("Max Pan Speed: {}", caps.max_pan_speed);
    println!("Max Tilt Speed: {}", caps.max_tilt_speed);
    Ok(())
}

fn demo_advanced_features(
    camera: &mut Camera<PTZOpticsG2>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📍 Demo 10: Advanced Features");

    println!("Setting 2D noise reduction...");
    block_on(camera.set_noise_reduction_2d(3))?;

    println!("Setting backlight compensation...");
    block_on(camera.backlight_on())?;

    println!("Setting image flip (horizontal)...");
    block_on(camera.set_image_flip(ImageFlipMode::Horizontal))?;
    thread::sleep(Duration::from_secs(1));

    println!("Resetting image flip...");
    block_on(camera.set_image_flip(ImageFlipMode::Off))?;

    println!("Setting anti-flicker mode...");
    block_on(camera.set_anti_flicker(AntiFlickerMode::Hz60))?;

    println!("Disabling backlight compensation...");
    block_on(camera.backlight_off())?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    // Get camera IP from environment or use default
    let camera_ip = std::env::var("CAMERA_IP").unwrap_or_else(|_| "192.168.0.100:5678".to_string());

    println!("🎥 Grafton VISCA Demo - Connecting to camera at {camera_ip}");
    println!("{}", "=".repeat(50));

    // Create camera with UDP transport
    let transport = UdpTransport::new(&camera_ip)?;
    let mut camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(transport));

    // Run all demos
    demo_power_control(&mut camera)?;
    demo_pan_tilt_movement(&mut camera)?;
    demo_zoom_control(&mut camera)?;
    demo_focus_control(&mut camera)?;
    demo_preset_positions(&mut camera)?;
    demo_exposure_control(&mut camera)?;
    demo_color_adjustments(&mut camera)?;
    demo_image_quality(&mut camera)?;
    demo_camera_info(&camera)?;
    demo_advanced_features(&mut camera)?;

    // Return to home position
    println!("\n🏁 Demo complete! Returning to home position...");
    block_on(camera.home())?;

    println!("\n✨ All demos completed successfully!");
    println!("{}", "=".repeat(50));

    Ok(())
}
