//! Example demonstrating Camera API inquiry commands.
//!
//! This example shows:
//! - How to use inquiry commands with the Camera<P> API
//! - Profile-aware position conversions
//! - Getting comprehensive camera state
//! - Querying various camera parameters
//!
//! The Camera API now supports full inquiry functionality through the
//! send_and_receive() method, making it a complete replacement for the Client API.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, units::Degrees, Camera},
    transport::AsyncUdpTransport,
    Error,
};
use std::env;
use tokio::time::{sleep, Duration};

#[cfg(not(feature = "async-client"))]
fn main() {
    eprintln!("This example requires the 'async-client' feature.");
    eprintln!("Run with: cargo run --example async_inquiry_demo --features async-client");
}

#[cfg(feature = "async-client")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Get camera address from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <camera_ip:port>", args[0]);
        eprintln!("Example: {} 192.168.1.100:52381", args[0]);
        std::process::exit(1);
    }

    // Connect to camera
    let camera_addr = &args[1];
    println!("Connecting to camera at {}...", camera_addr);

    // Create camera with async transport
    let transport = AsyncUdpTransport::new(camera_addr).await?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    println!("\n=== Camera API Inquiry Commands Demo ===\n");

    // 1. Basic Status Inquiries
    println!("1. Basic Status Inquiries:");

    // Power status
    match camera.get_power_state().await {
        Ok(is_on) => println!("   - Power: {}", if is_on { "ON" } else { "OFF" }),
        Err(e) => println!("   - Power inquiry failed: {}", e),
    }

    // Note: VISCA protocol doesn't support querying auto-focus status
    // Applications must track this based on the last focus command sent

    // 2. Position Inquiries with Profile-Aware Conversion
    println!("\n2. Position Inquiries (Profile-Aware):");

    // Get position in degrees (automatic conversion)
    match camera.get_position().await {
        Ok((pan, tilt)) => {
            println!(
                "   - Current position: pan={:.1}°, tilt={:.1}°",
                pan.0, tilt.0
            );
        }
        Err(e) => println!("   - Position inquiry failed: {}", e),
    }

    // Get position in raw VISCA units
    match camera.get_position_units().await {
        Ok((pan, tilt)) => {
            println!(
                "   - Position (VISCA units): pan={}, tilt={}",
                pan.0, tilt.0
            );
        }
        Err(e) => println!("   - Position units inquiry failed: {}", e),
    }

    // 3. Zoom and Focus Inquiries
    println!("\n3. Zoom and Focus Status:");

    match camera.get_zoom_position().await {
        Ok(zoom) => println!("   - Zoom position: {:.1}x", zoom),
        Err(e) => println!("   - Zoom inquiry failed: {}", e),
    }

    match camera.get_focus_position().await {
        Ok(focus) => println!("   - Focus position: {} (raw units)", focus),
        Err(e) => println!("   - Focus inquiry failed: {}", e),
    }

    // 4. Exposure Settings
    println!("\n4. Exposure Settings:");

    match camera.get_exposure_mode().await {
        Ok(mode) => println!("   - Exposure mode: {:?}", mode),
        Err(e) => println!("   - Exposure mode inquiry failed: {}", e),
    }

    match camera.get_shutter_speed().await {
        Ok(speed) => println!("   - Shutter speed: {:?}", speed),
        Err(e) => println!("   - Shutter inquiry failed: {}", e),
    }

    match camera.get_iris_position().await {
        Ok(iris) => println!("   - Iris: {:?}", iris),
        Err(e) => println!("   - Iris inquiry failed: {}", e),
    }

    match camera.get_gain().await {
        Ok(gain) => println!("   - Gain: {:?}", gain),
        Err(e) => println!("   - Gain inquiry failed: {}", e),
    }

    // 5. White Balance Settings
    println!("\n5. White Balance Settings:");

    match camera.get_white_balance_mode().await {
        Ok(mode) => println!("   - White balance mode: {:?}", mode),
        Err(e) => println!("   - WB mode inquiry failed: {}", e),
    }

    match camera.get_red_gain().await {
        Ok(gain) => println!("   - Red gain: {}", gain),
        Err(e) => println!("   - Red gain inquiry failed: {}", e),
    }

    match camera.get_blue_gain().await {
        Ok(gain) => println!("   - Blue gain: {}", gain),
        Err(e) => println!("   - Blue gain inquiry failed: {}", e),
    }

    // 6. Image Processing Settings
    println!("\n6. Image Processing:");

    match camera.get_brightness().await {
        Ok(val) => println!("   - Brightness: {}", val),
        Err(e) => println!("   - Brightness inquiry failed: {}", e),
    }

    match camera.get_sharpness().await {
        Ok(val) => println!("   - Sharpness: {}", val),
        Err(e) => println!("   - Sharpness inquiry failed: {}", e),
    }

    // 7. Comprehensive State Query
    println!("\n7. Comprehensive Camera State:");

    match camera.get_camera_state().await {
        Ok(state) => {
            println!("   Complete camera state retrieved:");
            println!("   - Power: {}", if state.power { "ON" } else { "OFF" });
            println!(
                "   - Position: pan={:.1}°, tilt={:.1}°",
                state.position.pan_degrees, state.position.tilt_degrees
            );
            println!("   - Zoom: {} (units)", state.optics.zoom);
            println!("   - Focus: {} (units)", state.optics.focus);
            println!("   - Exposure Mode: {:?}", state.exposure.mode);
            if let Some(shutter) = state.exposure.shutter {
                println!("   - Shutter: {:?}", shutter);
            }
            if let Some(iris) = state.exposure.iris {
                println!("   - Iris: {:?}", iris);
            }
            if let Some(gain) = state.exposure.gain {
                println!("   - Gain: {:?}", gain);
            }
            println!("   - WB Mode: {:?}", state.white_balance.mode);
            if let Some(red_gain) = state.white_balance.red_gain {
                println!("   - Red Gain: {}", red_gain);
            }
            if let Some(blue_gain) = state.white_balance.blue_gain {
                println!("   - Blue Gain: {}", blue_gain);
            }
            println!("   - Image Settings:");
            println!("     - Luminance: {}", state.image.luminance);
            println!("     - Contrast: {}", state.image.contrast);
            println!("     - Sharpness: {}", state.image.sharpness);
            println!("     - Saturation: {}", state.image.saturation);
            println!("     - Hue: {}", state.image.hue);
        }
        Err(e) => println!("   - State query failed: {}", e),
    }

    // 8. Demonstrating Control with Inquiry Feedback
    println!("\n8. Control with Inquiry Feedback:");

    // Move to a specific position and verify
    println!("   - Moving to pan=45°, tilt=-15°...");
    camera.set_position(Degrees(45.0), Degrees(-15.0)).await?;

    // Wait for movement to complete
    sleep(Duration::from_secs(3)).await;

    // Verify the position
    match camera.get_position().await {
        Ok((pan, tilt)) => {
            println!(
                "   - Verified position: pan={:.1}°, tilt={:.1}°",
                pan.0, tilt.0
            );
        }
        Err(e) => println!("   - Position verification failed: {}", e),
    }

    // Change zoom and verify
    println!("\n   - Setting zoom to position 16384 (mid-range)...");
    camera.set_zoom(16384).await?;

    sleep(Duration::from_secs(2)).await;

    match camera.get_zoom_position().await {
        Ok(zoom) => println!("   - Verified zoom position: {}", zoom),
        Err(e) => println!("   - Zoom verification failed: {}", e),
    }

    println!("\n=== Summary ===");
    println!("The Camera API now provides:");
    println!("✓ Type-safe camera control");
    println!("✓ Profile-aware operations");
    println!("✓ Compile-time validation");
    println!("✓ Full inquiry command support");
    println!("✓ Automatic unit conversions");
    println!("\nThe Camera<P> API is now a complete replacement for the Client API!");

    Ok(())
}
