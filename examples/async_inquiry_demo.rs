//! Example demonstrating Camera API limitations and inquiry command alternatives.
//!
//! This example shows:
//! - Why the Camera API doesn't support inquiry commands
//! - Alternative approaches for camera state management
//! - When to use the Client API vs Camera API
//!
//! Note: The Camera API is designed for type-safe camera control but does not
//! support inquiry commands. For applications requiring camera state queries,
//! use the Client API or implement state tracking in your application.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
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
        eprintln!("Example: {} 192.168.1.100:5678", args[0]);
        std::process::exit(1);
    }

    // Connect to camera
    let camera_addr = &args[1];
    println!("Connecting to camera at {}...", camera_addr);

    // Create camera with async transport
    let transport = AsyncUdpTransport::new(camera_addr).await?;
    let mut camera = Camera::<PTZOpticsG2>::new(transport);

    println!("\n=== Camera API vs Inquiry Commands ===\n");

    // Explain the limitation
    println!("IMPORTANT: The Camera API does not support inquiry commands.");
    println!("This is because:");
    println!("1. The UnifiedTransport trait only supports fire-and-forget commands");
    println!("2. Inquiry commands require response parsing which UnifiedTransport doesn't handle");
    println!("3. The Camera API focuses on type-safe control, not state queries\n");

    println!("For applications requiring inquiry commands, use the Client API instead.");
    println!("This example demonstrates Camera API alternatives.\n");

    // Demonstrate Camera API capabilities
    println!("=== Camera Control Demo (without inquiries) ===\n");

    // 1. Camera capabilities from profile
    println!("1. Camera Capabilities (from profile, not inquiry):");
    let caps = camera.capabilities();
    println!("   - Model: {}", caps.model_name);
    println!("   - Pan range: {:?} degrees", caps.pan_range_degrees);
    println!("   - Tilt range: {:?} degrees", caps.tilt_range_degrees);
    println!("   - Zoom range: {} steps", caps.zoom_steps);
    println!("   - Focus range: {} steps", caps.focus_steps);
    println!("   - Preset count: {}", caps.preset_count);
    println!("   - Supports digital zoom: {}", caps.supports_digital_zoom);
    println!("   - Max pan speed: {}", caps.max_pan_speed);
    println!("   - Max tilt speed: {}", caps.max_tilt_speed);

    // 2. State management approaches
    println!("\n2. State Management Approaches:");
    println!("   Since we can't query state, we can:");
    println!("   a) Maintain state in application");
    println!("   b) Always set known states");
    println!("   c) Use the Client API when queries are needed\n");

    // 3. Setting known states
    println!("3. Setting Known States:");

    // Power on (we assume it might be off)
    println!("   - Powering on camera...");
    camera.power_on().await?;
    sleep(Duration::from_secs(2)).await;

    // Set to home position (known state)
    println!("   - Moving to home position...");
    camera.home().await?;
    sleep(Duration::from_secs(2)).await;

    // Set specific zoom level
    println!("   - Setting zoom to minimum...");
    camera.set_zoom(0x0000).await?;

    // Set exposure mode
    println!("   - Setting exposure to auto...");
    use grafton_visca::command::exposure::ExposureMode;
    camera.set_exposure_mode(ExposureMode::Auto).await?;

    // Set white balance
    println!("   - Setting white balance to auto...");
    use grafton_visca::command::white_balance::WhiteBalanceMode;
    camera
        .set_white_balance_mode(WhiteBalanceMode::Auto)
        .await?;

    // 4. Application-level state tracking
    println!("\n4. Application-Level State Tracking Example:");

    // Example state structure
    #[derive(Debug)]
    struct CameraState {
        _power_on: bool,
        zoom_level: u16,
        at_home: bool,
        _exposure_mode: ExposureMode,
        _white_balance_mode: WhiteBalanceMode,
    }

    let mut state = CameraState {
        _power_on: true,    // We just powered it on
        zoom_level: 0x0000, // We set it to minimum
        at_home: true,      // We moved to home
        _exposure_mode: ExposureMode::Auto,
        _white_balance_mode: WhiteBalanceMode::Auto,
    };

    println!("   Current state: {:?}", state);

    // Update state as we control camera
    println!("\n   - Zooming in...");
    camera.zoom_in().await?;
    sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;
    state.zoom_level = 0x1000; // Estimate based on zoom time
    state.at_home = false; // No longer at exact home position

    println!("   Updated state: {:?}", state);

    // 5. When to use Client API
    println!("\n5. When to Use Client API Instead:");
    println!("   Use the Client API when you need:");
    println!("   - Power status queries");
    println!("   - Current position inquiries");
    println!("   - Zoom position queries");
    println!("   - Focus position queries");
    println!("   - Any other camera state information");

    // Demonstrate Client API for inquiries
    #[cfg(feature = "blocking-client")]
    {
        println!("\n6. Client API Inquiry Example:");
        use grafton_visca::{command::InquiryCommand, Client};

        // Create a Client for inquiries
        match Client::connect_udp(camera_addr) {
            Ok(client) => {
                // Now we can do inquiries
                match client.send(&InquiryCommand::Power) {
                    Ok(response) => println!("   Power inquiry response: {:?}", response),
                    Err(e) => println!("   Power inquiry failed: {}", e),
                }

                match client.send(&InquiryCommand::ZoomPosition) {
                    Ok(response) => println!("   Zoom position response: {:?}", response),
                    Err(e) => println!("   Zoom inquiry failed: {}", e),
                }
            }
            Err(e) => {
                println!("   Failed to create Client: {}", e);
            }
        }
    }

    println!("\n=== Summary ===");
    println!("The Camera API provides:");
    println!("✓ Type-safe camera control");
    println!("✓ Profile-aware operations");
    println!("✓ Compile-time validation");
    println!("✗ No inquiry command support");
    println!("\nFor full VISCA functionality including inquiries, use the Client API.");

    Ok(())
}
