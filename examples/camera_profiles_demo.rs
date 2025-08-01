//! Camera profiles demonstration.
//!
//! This example shows how to use different camera profiles with compile-time safety
//! and demonstrates the capability-based API design.
//!
//! Run with: cargo run --example camera_profiles_demo --features tokio

#[cfg(feature = "tokio")]
use grafton_visca::{
    prelude::r#async::{GenericViscaCam, PTZOpticsG2Cam, SonyFR7Cam},
    r#async::prelude::*,
    transport::TcpTransport,
    types::{PanSpeed, SpeedLevel, TiltSpeed},
    Degrees, Error, MotionSyncMode, NDFilterMode, Normalized, PanTiltDirection, PresetNumber, Raw,
};

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== VISCA Camera Profiles Demo ===\n");

    // Note: Replace these with your actual camera IPs
    demonstrate_ptzoptics_g2().await?;
    demonstrate_sony_fr7().await?;
    demonstrate_generic_visca().await?;
    demonstrate_compile_time_safety();

    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_ptzoptics_g2() -> Result<(), Error> {
    println!("=== PTZOptics G2 Demo ===");

    // Connect to camera
    let transport = TcpTransport::connect("192.168.1.100:5678").await?;
    let camera = PTZOpticsG2Cam::new_async(transport);

    println!("Connected to: PTZOptics G2");

    // PTZOptics G2 supports standard VISCA features
    println!("\nDemonstrating PTZOptics G2 features:");

    // Power control
    camera.power_on().await?;
    println!("✓ Power on");

    // Pan/tilt operations
    camera.pan_tilt_home().await?;
    println!("✓ Pan/tilt home");

    // Position control with speed
    let pan = Degrees::new(45.0);
    let tilt = Degrees::new(10.0);
    camera
        .pan_tilt_absolute(pan, tilt, SpeedLevel::from(24))
        .await?;
    println!("✓ Moved to position: pan=45°, tilt=10°");

    // Zoom control
    camera.zoom_absolute(Normalized::new(0.5)).await?;
    println!("✓ Set zoom to 50%");

    // Preset operations
    camera.preset_recall(PresetNumber::new(1).unwrap()).await?;
    println!("✓ Recalled preset 1");

    println!();
    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_sony_fr7() -> Result<(), Error> {
    println!("=== Sony FR7 Demo ===");

    // Connect to camera
    let transport = TcpTransport::connect("192.168.1.101:5678").await?;
    let camera = SonyFR7Cam::new_async(transport);

    println!("Connected to: Sony FR7");
    println!("\nDemonstrating Sony FR7 advanced features:");

    // Basic operations (available on all cameras)
    camera.power_on().await?;
    println!("✓ Power on");

    // Sony FR7 specific: ND Filter control
    camera.set_nd_filter_mode(NDFilterMode::Preset).await?;
    println!("✓ Set ND filter to Preset mode");

    camera.set_nd_filter_mode(NDFilterMode::Variable).await?;
    println!("✓ Set ND filter to Variable mode");

    // Sony FR7 also supports motion sync
    camera.set_motion_sync_mode(MotionSyncMode::On).await?;
    println!("✓ Motion sync enabled");

    // And variable speed pan/tilt
    let pan_speed = PanSpeed::from(Raw(0x18)); // Medium-fast speed
    let tilt_speed = TiltSpeed::from(Raw(0x14)); // Medium speed
    camera
        .pan_tilt_move(PanTiltDirection::UpRight, pan_speed, tilt_speed)
        .await?;
    println!("✓ Variable speed pan/tilt movement");

    println!();
    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_generic_visca() -> Result<(), Error> {
    println!("=== Generic VISCA Demo ===");

    // Connect to unknown camera model
    let transport = TcpTransport::connect("192.168.1.102:5678").await?;
    let camera = GenericViscaCam::new_async(transport);

    println!("Connected to: Generic VISCA");
    println!("\nUsing conservative feature set for compatibility:");

    // Generic VISCA supports basic operations
    camera.power_on().await?;
    println!("✓ Power on");

    camera.zoom_in().await?;
    println!("✓ Zoom in");

    camera.focus_auto().await?;
    println!("✓ Auto focus");

    camera.pan_tilt_home().await?;
    println!("✓ Pan/tilt home");

    // Generic VISCA uses conservative limits
    println!("\nGeneric VISCA uses safe defaults:");
    println!("- 6 presets (most cameras support at least this)");
    println!("- Standard zoom/focus ranges");
    println!("- Basic pan/tilt speeds");

    println!();
    Ok(())
}

#[cfg(feature = "tokio")]
fn demonstrate_compile_time_safety() {
    println!("=== Compile-Time Safety Demo ===");
    println!("\nThe new API provides compile-time guarantees:");
    println!("- Can't call unsupported methods on a camera");
    println!("- No runtime 'FeatureNotSupported' errors for known capabilities");
    println!("- Zero overhead - all decisions made at compile time");

    println!("\nExample code that won't compile:");
    println!("```rust");
    println!("// This would fail at compile time:");
    println!("// let generic_cam = GenericViscaCam::new(transport);");
    println!("// generic_cam.set_nd_filter_mode(NDFilterMode::Preset);");
    println!("// Error: GenericVisca doesn't implement NDFilter trait");
    println!("```");

    println!("\nBut this compiles and works:");
    println!("```rust");
    println!("let sony_cam = SonyFR7Cam::new(transport);");
    println!("sony_cam.set_nd_filter_mode(NDFilterMode::Preset); // ✓ OK");
    println!("```");

    println!();
}

#[cfg(not(feature = "tokio"))]
fn main() {
    println!("This example requires the 'tokio' feature.");
    println!("Run with: cargo run --example camera_profiles_demo --features tokio");
}
