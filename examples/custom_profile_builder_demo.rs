//! Example demonstrating the custom camera profile builder pattern.

use grafton_visca::{
    camera::{
        Camera, CameraProfile, CustomProfile, CustomProfileBuilder, CustomProfileTypedBuilder,
    },
    transport::create,
    Error,
};

// Include the transport implementations from the example files

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    run_demo().await
}

#[cfg(not(feature = "async"))]
fn main() -> Result<(), Error> {
    println!("This example requires the 'async' feature to be enabled.");
    println!("Run with: cargo run --features async --example custom_profile_builder_demo");
    Ok(())
}

#[cfg(feature = "async")]
async fn run_demo() -> Result<(), Error> {
    env_logger::init();

    println!("=== Custom Camera Profile Builder Demo ===\n");

    // Method 1: Simple builder pattern
    println!("1. Creating custom profile with simple builder:");
    let simple_profile = CustomProfileBuilder::new("MyCustomPTZ v2.0")
        .pan_range(-2000..=2000)
        .tilt_range(-500..=1500)
        .zoom_range(0x0000..=0x8000)
        .focus_range(0x1000..=0xF000)
        .pan_speed(20)
        .tilt_speed(16)
        .digital_zoom(true)
        .pan_degrees_range(-170.0..=170.0)
        .tilt_degrees_range(-30.0..=90.0)
        .max_preset_id(99)
        .build();

    println!("   Model: {}", simple_profile.model_name());
    println!(
        "   Pan range: {:?} degrees",
        simple_profile.pan_degree_range()
    );
    println!(
        "   Tilt range: {:?} degrees",
        simple_profile.tilt_degree_range()
    );
    println!(
        "   Digital zoom: {}",
        simple_profile.digital_zoom_supported()
    );
    println!("   Max preset ID: {}\n", simple_profile.get_max_preset_id());

    // Method 2: Typed builder pattern (compile-time validation)
    println!("2. Creating custom profile with typed builder:");
    let typed_profile = CustomProfileTypedBuilder::new()
        .model_name("SecurityCam Pro X1")
        .pan_range(-3000..=3000)
        .tilt_range(-600..=1800)
        .zoom_range(0x0000..=0xA000)
        .focus_range(0x2000..=0xE000)
        .digital_zoom(false)
        .pan_speed(24)
        .tilt_speed(20)
        .pan_degrees_range(-180.0..=180.0)
        .tilt_degrees_range(-45.0..=135.0)
        .max_preset_id(127)
        .build();

    println!("   Model: {}", typed_profile.model_name());
    println!(
        "   Pan range: {:?} degrees",
        typed_profile.pan_degree_range()
    );
    println!(
        "   Tilt range: {:?} degrees",
        typed_profile.tilt_degree_range()
    );
    println!(
        "   Max speeds: pan={}, tilt={}\n",
        typed_profile.max_pan_speed(),
        typed_profile.max_tilt_speed()
    );

    // Method 3: Using defaults with minimal configuration
    println!("3. Creating profile with minimal configuration:");
    let minimal_profile = CustomProfile::builder("Basic PTZ")
        .pan_degrees_range(-90.0..=90.0)
        .tilt_degrees_range(-20.0..=80.0)
        .build();

    println!("   Model: {}", minimal_profile.model_name());
    println!("   Using default VISCA ranges with custom degree mappings\n");

    // Demonstrate unit conversions
    println!("4. Testing unit conversions:");
    let test_profile = CustomProfileBuilder::new("Conversion Test Camera")
        .pan_range(-1000..=1000)
        .tilt_range(-500..=500)
        .pan_degrees_range(-100.0..=100.0)
        .tilt_degrees_range(-50.0..=50.0)
        .build();

    // Test pan conversions
    println!("   Pan conversions:");
    for units in [-1000, -500, 0, 500, 1000] {
        let degrees = test_profile.pan_units_to_degrees(units);
        let back_to_units = test_profile.pan_degrees_to_units(degrees);
        println!(
            "      {} units = {:.1}° (converts back to {} units)",
            units, degrees, back_to_units
        );
    }

    // Test tilt conversions
    println!("\n   Tilt conversions:");
    for units in [-500, -250, 0, 250, 500] {
        let degrees = test_profile.tilt_units_to_degrees(units);
        let back_to_units = test_profile.tilt_degrees_to_units(degrees);
        println!(
            "      {} units = {:.1}° (converts back to {} units)",
            units, degrees, back_to_units
        );
    }

    // Create a camera with the custom profile
    println!("\n5. Creating camera with custom profile:");
    let transport = create::tcp("192.168.1.100:5678").await?;
    let camera = Camera::with_profile(transport, simple_profile);

    let caps = camera.capabilities();
    println!("   Camera capabilities:");
    println!("   - Model: {}", caps.model_name);
    println!("   - Pan range: {:?} degrees", caps.pan_range_degrees);
    println!("   - Tilt range: {:?} degrees", caps.tilt_range_degrees);
    println!("   - Zoom steps: {}", caps.zoom_steps);
    println!("   - Focus steps: {}", caps.focus_steps);
    println!("   - Preset count: {}", caps.preset_count);
    println!("   - Digital zoom: {}", caps.supports_digital_zoom);
    println!(
        "   - Max speeds: pan={}, tilt={}",
        caps.max_pan_speed, caps.max_tilt_speed
    );

    println!("\n✓ Custom profile builder demo complete!");

    Ok(())
}
