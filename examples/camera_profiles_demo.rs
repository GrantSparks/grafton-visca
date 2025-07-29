//! Camera profiles demonstration.
//!
//! This example shows how to use different camera profiles with the unified Camera API
//! and demonstrates the capability-based API design.
//!
//! Run with: cargo run --example camera_profiles_demo --features tokio

#[cfg(feature = "tokio")]
use grafton_visca::{
    types::SpeedLevel, Camera, CameraModel, Degrees, Error, Normalized, PresetNumber,
};

#[cfg(feature = "tokio")]
use grafton_visca::r#async::{FocusOps, PanTiltOps, PowerOps, PresetsOps, ZoomOps};
#[cfg(feature = "tokio")]
use grafton_visca::transport::tokio::Tcp;

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== VISCA Camera Profiles Demo ===\n");

    // Note: Replace these with your actual camera IPs
    demonstrate_ptzoptics_g2().await?;
    demonstrate_sony_fr7().await?;
    demonstrate_generic_visca().await?;

    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_ptzoptics_g2() -> Result<(), Error> {
    println!("=== PTZOptics G2 Demo ===");

    // Connect to camera
    let transport = Tcp::connect("192.168.1.100:5678").await?;
    let unified_camera = Camera::with_profile(CameraModel::PTZOpticsG2, transport);

    println!("Connected to: {}", unified_camera.model_name());
    println!("Profile info: {}", unified_camera.profile_info());

    // Check capabilities at runtime (from unified camera)
    println!("\nCapabilities:");
    println!(
        "  ✓ Pan/Tilt: {}",
        unified_camera.supports_capability("pan_tilt")
    );
    println!("  ✓ Zoom: {}", unified_camera.supports_capability("zoom"));
    println!("  ✓ Focus: {}", unified_camera.supports_capability("focus"));
    println!(
        "  ✓ Presets: {} (max: {})",
        unified_camera.supports_capability("presets"),
        unified_camera.max_presets()
    );
    println!(
        "  ✗ ND Filter: {}",
        unified_camera.supports_capability("nd_filter")
    );

    let power_on_time = unified_camera.power_on_time();
    let camera = unified_camera.r#async();

    // Use the camera with high-level API
    println!("\nPerforming operations:");

    // Power on and wait for initialization
    camera.power_on().await?;
    println!(
        "  - Powered on (waiting {} seconds)",
        power_on_time.as_secs()
    );
    tokio::time::sleep(power_on_time).await;

    // Move to home position
    camera.pan_tilt_home().await?;
    println!("  - Moved to home position");

    // Absolute positioning using degrees
    camera
        .pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::from(12))
        .await?;
    println!("  - Moved to Pan: 45°, Tilt: 15° at speed 12");

    // Zoom operations
    camera.zoom_absolute(Normalized::new(0.5)).await?; // 50% zoom
    println!("  - Set zoom to 50%");

    // Focus operations
    camera.focus_auto().await?;
    println!("  - Enabled auto-focus");

    // Preset operations (G2 supports up to 255 presets)
    camera.preset_set(PresetNumber::new(1)?).await?;
    println!("  - Saved current position as preset 1");

    // Note: ND filter operations would fail at runtime for G2
    // since it doesn't support them

    println!();
    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_sony_fr7() -> Result<(), Error> {
    println!("=== Sony FR7 Demo ===");

    let transport = Tcp::connect("192.168.1.101:5678").await?;
    let unified_camera = Camera::with_profile(CameraModel::SonyFR7, transport);

    println!("Connected to: {}", unified_camera.model_name());
    println!("Profile info: {}", unified_camera.profile_info());

    // FR7 has additional capabilities
    println!("\nCapabilities:");
    println!("  ✓ All standard features");
    println!(
        "  ✓ ND Filter: {} (mode: {:?})",
        unified_camera.supports_capability("nd_filter"),
        unified_camera.nd_filter_mode()
    );

    let power_on_time = unified_camera.power_on_time();
    let supports_nd_filter = unified_camera.supports_capability("nd_filter");
    let camera = unified_camera.r#async();

    println!("\nPerforming FR7-specific operations:");

    // Standard operations work the same
    camera.power_on().await?;
    tokio::time::sleep(power_on_time).await;

    // FR7 can also use ND filters
    if supports_nd_filter {
        // Note: These methods would need to be implemented
        // camera.set_nd_filter(1).await?;
        println!("  - ND filter operations available");
    }

    println!();
    Ok(())
}

#[cfg(feature = "tokio")]
async fn demonstrate_generic_visca() -> Result<(), Error> {
    println!("=== Generic VISCA Demo ===");

    let transport = Tcp::connect("192.168.1.102:5678").await?;
    let unified_camera = Camera::new(transport); // Defaults to GenericVisca

    println!("Connected to: {}", unified_camera.model_name());
    println!("Profile info: {}", unified_camera.profile_info());

    // Generic profile only guarantees basic VISCA operations
    println!("\nCapabilities (minimal guarantee):");
    println!("  ✓ Power: {}", unified_camera.supports_capability("power"));
    println!(
        "  ✓ Pan/Tilt: {}",
        unified_camera.supports_capability("pan_tilt")
    );
    println!("  ✓ Zoom: {}", unified_camera.supports_capability("zoom"));
    println!("  ? Other features: implementation-dependent");

    let supports_focus = unified_camera.supports_capability("focus");
    let camera = unified_camera.r#async();

    // Safe to use basic operations
    camera.power_on().await?;
    camera.pan_tilt_home().await?;

    // Check capabilities before using advanced features
    if supports_focus {
        camera.focus_auto().await?;
        println!("  - Focus available on this camera");
    } else {
        println!("  - Focus not available");
    }

    println!();
    Ok(())
}

// Example of writing code that works with any camera
// The unified Camera API means we don't need generics -
// just check capabilities at runtime if needed
#[cfg(feature = "tokio")]
#[allow(dead_code)]
async fn capture_preset(
    camera: &mut grafton_visca::r#async::Camera,
    preset_id: u8,
) -> Result<(), grafton_visca::Error> {
    use grafton_visca::r#async::{PanTiltOps, PowerOps, PresetsOps};
    // Power on if needed
    camera.power_on().await?;
    // Note: power_on_time() is only available on unified Camera
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Move to home and save as preset
    camera.pan_tilt_home().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Save preset
    // Note: supports_capability() is only available on unified Camera
    // For async Camera, we just try the operation
    camera.preset_set(PresetNumber::new(preset_id)?).await?;
    println!("  - Saved position as preset {}", preset_id);

    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature to be enabled.");
    eprintln!("Run with: cargo run --example camera_profiles_demo --features tokio");
}
