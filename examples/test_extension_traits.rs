//! Test program to verify extension traits work with ViscaClient

use grafton_visca::{
    ViscaClient, ViscaError,
    // Import all extension traits
    ViscaTransportExt,
    ViscaZoomExt,
    ViscaPowerExt,
    ViscaPresetExt,
    ViscaExposureExt,
    ViscaImageExt,
    ViscaInquiryExt,
    ViscaWhiteBalanceExt,
    ViscaPositionExt,
};

fn main() -> Result<(), ViscaError> {
    env_logger::init();

    // Create a client
    let mut client = ViscaClient::connect_udp("192.168.1.100:5678")?;
    
    // Test ViscaTransportExt methods
    println!("Testing ViscaTransportExt...");
    client.power_on()?;
    client.home()?;
    
    // Test ViscaZoomExt methods
    println!("Testing ViscaZoomExt...");
    client.zoom_to(0x2000)?;
    ViscaZoomExt::zoom_in(&mut client, Some(5))?;  // Disambiguate
    client.stop_zoom()?;
    
    // Test ViscaTransportExt pan/tilt methods
    println!("Testing pan/tilt methods from ViscaTransportExt...");
    client.home()?;  // This is the home method from ViscaTransportExt
    client.move_stop()?;
    
    // Test ViscaPowerExt methods
    println!("Testing ViscaPowerExt...");
    let is_on = client.is_powered_on()?;
    println!("Camera is powered: {}", if is_on { "ON" } else { "OFF" });
    
    // Test ViscaPresetExt methods
    println!("Testing ViscaPresetExt...");
    ViscaPresetExt::save_preset(&mut client, 1)?;  // Disambiguate
    ViscaPresetExt::recall_preset(&mut client, 1)?;  // Disambiguate
    
    // Test ViscaTransportExt focus methods
    println!("Testing focus methods from ViscaTransportExt...");
    client.set_focus_auto()?;
    client.set_focus_manual()?;
    
    // Test ViscaExposureExt methods
    println!("Testing ViscaExposureExt...");
    ViscaExposureExt::set_exposure_mode(&mut client, grafton_visca::command::exposure::ExposureMode::Auto)?;
    ViscaTransportExt::set_iris(&mut client, 10)?;  // Disambiguate
    
    // Test ViscaImageExt methods
    println!("Testing ViscaImageExt...");
    client.set_brightness(0)?;
    client.set_contrast(0)?;
    
    // Test ViscaInquiryExt methods
    println!("Testing ViscaInquiryExt...");
    let (pan, tilt) = client.get_pan_tilt_position()?;
    println!("Current position - Pan: {}, Tilt: {}", pan, tilt);
    
    let zoom = client.get_zoom_position()?;
    println!("Current zoom position: 0x{:04X}", zoom);
    
    // Test ViscaWhiteBalanceExt methods
    println!("Testing ViscaWhiteBalanceExt...");
    client.set_white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::Auto)?;
    
    // Test ViscaPositionExt methods
    println!("Testing ViscaPositionExt...");
    client.move_to_degrees(0.0, 0.0, Some((10, 10)))?;  // Pass speeds as Option<(u8, u8)>
    
    println!("\nAll extension traits are working correctly!");
    
    Ok(())
}