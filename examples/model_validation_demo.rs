//! Camera model validation demonstration
//!
//! This example demonstrates how the new `Camera<P>` API enforces model-specific
//! constraints at compile time and runtime. It shows how different camera profiles
//! (PTZOpticsG2, PTZOptics30X, SonyEVID70) have different ranges and capabilities.

#[cfg(feature = "async")]
use grafton_visca::{
    camera::{
        profiles::{
            G2Gain, G2PresetId, GenericPresetId, GenericVisca, PTZOptics30X, PTZOpticsG2,
            SonyEVID70,
        },
        units::{Degrees, ViscaUnits},
        CameraProfile, CameraTransport,
    },
    transport::TransportFuture,
    types::ZoomPosition,
    Camera, Command, Error,
};

#[cfg(feature = "async")]
/// Mock transport for demonstration purposes.
/// In real usage, you would use UdpTransport or TcpTransport.
#[derive(Debug)]
struct MockTransport;

#[cfg(feature = "async")]
impl CameraTransport for MockTransport {
    fn send_command<'a>(
        &'a mut self,
        _command: &'a dyn Command,
    ) -> TransportFuture<'a, grafton_visca::Response> {
        Box::pin(async { Ok(grafton_visca::Response::Completion) })
    }
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== Camera Model Validation Demo ===");
    println!("Demonstrating profile-specific validation with the new Camera<P> API");
    println!();

    // Example 1: PTZOptics G2 Camera
    demo_ptzoptics_g2().await?;

    // Example 2: PTZOptics 30X Camera
    demo_ptzoptics_30x().await?;

    // Example 3: Sony EVI-D70 Camera
    demo_sony_evid70().await?;

    // Example 4: Generic VISCA Camera (permissive ranges)
    demo_generic_visca().await?;

    println!("=== Demo Complete ===");
    println!();
    println!("Key takeaways:");
    println!("- Each camera profile enforces its specific constraints");
    println!("- Invalid operations are caught before being sent to the camera");
    println!("- Type-safe preset IDs and gain values prevent invalid configurations");
    println!("- Position conversions (degrees/units) are profile-specific");
    println!();
    println!("Note: This demo uses a mock transport. In real usage, you would use:");
    println!("- UdpTransport for UDP connections");
    println!("- TcpTransport for TCP connections");
    println!("- Or implement your own Transport trait");

    Ok(())
}

#[cfg(feature = "async")]
async fn demo_ptzoptics_g2() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== PTZOptics G2 Demo ===");
    println!("Model: 20X optical zoom, ±170° pan, -30° to +90° tilt");
    println!();

    // Create a G2 camera (simulated connection)
    let transport = MockTransport;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // 1. Valid zoom position for G2 (20X optical)
    println!("1. Testing valid zoom position (20X):");
    match camera.set_zoom(ZoomPosition::new(0x7000)?).await {
        Ok(_) => println!("   ✓ Zoom to 20X position would be sent"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // 2. Invalid zoom position for G2 (beyond 20X)
    println!("2. Testing invalid zoom position (beyond 20X range):");
    match ZoomPosition::new(0x7AC0) {
        Ok(pos) => match camera.set_zoom(pos).await {
            Ok(_) => println!("   ✗ Command sent (shouldn't happen)"),
            Err(e) => println!("   ✗ Unexpected error: {}", e),
        },
        Err(Error::ParameterOutOfRange { parameter, .. }) => {
            println!("   ✓ Validation prevented invalid command");
            println!("     Parameter '{}' out of valid zoom range", parameter);
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }

    // 3. Valid pan/tilt position in degrees
    println!("3. Testing valid pan/tilt position (100°, 45°):");
    match camera.set_position(Degrees(100.0), Degrees(45.0)).await {
        Ok(_) => println!("   ✓ Position command would be sent"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // 4. Invalid pan position (beyond ±170°)
    println!("4. Testing invalid pan position (200° - beyond range):");
    match camera.set_position(Degrees(200.0), Degrees(0.0)).await {
        Ok(_) => println!("   ✗ Command sent (shouldn't happen)"),
        Err(Error::ParameterOutOfRange { parameter, .. }) => {
            println!("   ✓ Validation prevented invalid command");
            println!("     Parameter '{}' out of G2's pan range", parameter);
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }

    // 5. G2-specific preset (0-89)
    println!("5. Testing valid G2 preset (89):");
    match G2PresetId::new(89) {
        Ok(preset) => match camera.recall_preset(preset).await {
            Ok(_) => println!("   ✓ Preset 89 recall would be sent"),
            Err(e) => println!("   ✗ Error: {}", e),
        },
        Err(e) => println!("   ✗ Error creating preset: {}", e),
    }

    // 6. Invalid preset for G2 (>89)
    println!("6. Testing invalid G2 preset (90):");
    match G2PresetId::new(90) {
        Ok(_) => println!("   ✗ Preset created (shouldn't happen)"),
        Err(e) => println!("   ✓ Validation prevented invalid preset: {}", e),
    }

    // 7. G2-specific gain values
    println!("7. Testing G2 gain values:");
    match camera.set_gain(G2Gain::Gain12dB).await {
        Ok(_) => println!("   ✓ 12dB gain would be set"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    println!();
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_ptzoptics_30x() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== PTZOptics 30X Demo ===");
    println!("Model: 30X optical zoom, ±180° pan, -90° to +120° tilt");
    println!();

    let transport = MockTransport;
    let camera = Camera::<PTZOptics30X>::new(transport);

    // 1. 30X can handle higher zoom values than G2
    println!("1. Testing 30X zoom position:");
    match camera.set_zoom(0x4000).await {
        Ok(_) => println!("   ✓ 30X zoom position would be sent"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // 2. 30X has wider tilt range
    println!("2. Testing 30X tilt range (+100°):");
    match camera.set_position(Degrees(0.0), Degrees(100.0)).await {
        Ok(_) => println!("   ✓ Tilt to +100° would be sent (valid for 30X)"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // 3. 30X supports standard VISCA presets (0-255)
    println!("3. Testing 30X preset range:");
    // Note: The current PresetNumber type is limited to 0-89 (G2 specific)
    // This is a limitation that should be addressed in the future
    let preset = GenericPresetId::new(89);
    match camera.recall_preset(preset).await {
        Ok(_) => println!("   ✓ Preset 89 would be recalled"),
        Err(e) => println!("   ✗ Error: {}", e),
    }
    println!("   (Note: 30X actually supports presets 0-255, but PresetNumber type is currently limited to 0-89)");

    println!();
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_sony_evid70() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Sony EVI-D70 Demo ===");
    println!("Model: 18X optical zoom, ±100° pan, ±25° tilt");
    println!();

    let transport = MockTransport;
    let camera = Camera::<SonyEVID70>::new(transport);

    // 1. EVI-D70 has limited pan range
    println!("1. Testing EVI-D70 pan limits:");
    match camera.set_position(Degrees(90.0), Degrees(0.0)).await {
        Ok(_) => println!("   ✓ Pan to 90° would be sent (within ±100°)"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    match camera.set_position(Degrees(150.0), Degrees(0.0)).await {
        Ok(_) => println!("   ✗ Command sent (shouldn't happen)"),
        Err(Error::ParameterOutOfRange { parameter, .. }) => {
            println!("   ✓ Validation prevented pan beyond ±100°");
            println!("     Parameter '{}' out of EVI-D70's range", parameter);
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }

    // 2. EVI-D70 has very limited tilt range
    println!("2. Testing EVI-D70 tilt limits:");
    match camera.set_position(Degrees(0.0), Degrees(20.0)).await {
        Ok(_) => println!("   ✓ Tilt to 20° would be sent (within ±25°)"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    match camera.set_position(Degrees(0.0), Degrees(50.0)).await {
        Ok(_) => println!("   ✗ Command sent (shouldn't happen)"),
        Err(Error::ParameterOutOfRange { parameter, .. }) => {
            println!("   ✓ Validation prevented tilt beyond ±25°");
            println!("     Parameter '{}' out of EVI-D70's range", parameter);
        }
        Err(e) => println!("   ✗ Unexpected error: {}", e),
    }

    // 3. EVI-D70 only supports presets 0-5
    println!("3. Testing EVI-D70 preset limitations:");
    let preset = GenericPresetId::new(5);
    match camera.recall_preset(preset).await {
        Ok(_) => println!("   ✓ Preset 5 would be recalled (max for EVI-D70)"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    // Note: The profile indicates max_preset_id is 5, but the current API
    // doesn't enforce this at the type level for non-G2 cameras
    println!("   (Note: EVI-D70 only supports presets 0-5)");

    // 4. Check EVI-D70 capabilities
    println!("4. EVI-D70 capability queries:");
    println!(
        "   - Wide Dynamic Range: {}",
        camera.profile().supports_wide_dynamic_range()
    );
    println!(
        "   - Image Stabilization: {}",
        camera.profile().supports_image_stabilization()
    );
    println!(
        "   - Image Flip: {}",
        camera.profile().supports_image_flip()
    );
    println!(
        "   - White Balance Modes: {}",
        camera.profile().white_balance_mode_count()
    );

    println!();
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_generic_visca() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Generic VISCA Demo ===");
    println!("Model: Unknown camera with permissive ranges");
    println!();

    let transport = MockTransport;
    let camera = Camera::<GenericVisca>::new(transport);

    // Generic VISCA allows wide ranges for compatibility
    println!("1. Generic camera accepts wide ranges:");
    match camera
        .set_position_units(ViscaUnits(30000), ViscaUnits(20000))
        .await
    {
        Ok(_) => println!("   ✓ Large position values accepted for generic camera"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    println!("2. Generic camera accepts full zoom range:");
    match camera.set_zoom(0xFFFF).await {
        Ok(_) => println!("   ✓ Maximum zoom value accepted"),
        Err(e) => println!("   ✗ Error: {}", e),
    }

    println!();
    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature to be enabled.");
    eprintln!("Run with: cargo run --example model_validation_demo --features async");
}
