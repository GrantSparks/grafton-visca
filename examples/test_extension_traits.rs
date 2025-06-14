//! Example program

//! Test program to verify extension traits work with `Client`

#[cfg(feature = "blocking-client")]
use grafton_visca::{
    command::{
        pan_tilt::{PanSpeed, TiltSpeed},
        zoom::ZoomSpeed,
    },
    Client,
    Error,
    ExposureExt,
    ImageExt,
    InquiryExt,
    PositionExt,
    PowerExt,
    PresetExt,
    // Import all extension traits
    TransportExt,
    WhiteBalanceExt,
    ZoomExt,
};

#[cfg(feature = "blocking-client")]
fn main() -> Result<(), Error> {
    env_logger::init();

    // Create a client
    let mut client = Client::connect_udp("192.168.1.100:5678")?;

    // Test TransportExt methods
    println!("Testing TransportExt...");
    let was_already_on = client.ensure_powered_on()?;
    if !was_already_on {
        println!("Camera was powered off, now powered on");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    client.home()?;

    // Test ZoomExt methods
    println!("Testing ZoomExt...");
    client.zoom_to(0x2000)?;
    ZoomExt::zoom_in_speed(&mut client, Some(ZoomSpeed::new(5)?))?; // Disambiguate
    client.stop_zoom()?;

    // Test TransportExt pan/tilt methods
    println!("Testing pan/tilt methods from TransportExt...");
    client.home()?; // This is the home method from TransportExt
    client.move_stop()?;

    // Test PowerExt methods
    println!("Testing PowerExt...");
    let is_on = client.is_powered_on()?;
    println!("Camera is powered: {}", if is_on { "ON" } else { "OFF" });

    // Test PresetExt methods
    println!("Testing PresetExt...");
    PresetExt::set_preset(&mut client, 1)?; // Disambiguate
    PresetExt::recall_preset(&mut client, 1)?; // Disambiguate

    // Test TransportExt focus methods
    println!("Testing focus methods from TransportExt...");
    client.set_focus_auto()?;
    client.set_focus_manual()?;

    // Test ExposureExt methods
    println!("Testing ExposureExt...");
    ExposureExt::set_exposure_mode(
        &mut client,
        grafton_visca::command::exposure::ExposureMode::Auto,
    )?;
    TransportExt::set_iris(&mut client, 10)?; // Disambiguate

    // Test ImageExt methods
    println!("Testing ImageExt...");
    client.set_brightness(0)?;
    client.set_contrast(0)?;

    // Test InquiryExt methods
    println!("Testing InquiryExt...");
    let (pan, tilt) = client.get_pan_tilt_position()?;
    println!("Current position - Pan: {pan}, Tilt: {tilt}");

    let zoom = client.get_zoom_position()?;
    println!("Current zoom position: 0x{zoom:04X}");

    // Test WhiteBalanceExt methods
    println!("Testing WhiteBalanceExt...");
    client.set_white_balance_mode(grafton_visca::command::white_balance::WhiteBalanceMode::Auto)?;

    // Test PositionExt methods
    println!("Testing PositionExt...");
    client.move_to_degrees(0.0, 0.0, Some((PanSpeed::new(10)?, TiltSpeed::new(10)?)))?; // Pass speeds as typed values

    println!("\nAll extension traits are working correctly!");

    Ok(())
}

#[cfg(not(feature = "blocking-client"))]
fn main() {
    println!("This example requires the 'blocking-client' feature to be enabled.");
    println!("Run with: cargo run --example test_extension_traits --features blocking-client");
}
