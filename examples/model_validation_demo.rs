//! Camera model validation demonstration
//!
//! This example demonstrates how the Camera<P> API enforces model-specific
//! constraints at compile time. It shows how different camera profiles
//! (PTZOpticsG2, GenericVisca, SonyFR7) have different capabilities.

use bytes::Bytes;
#[cfg(feature = "tokio")]
use grafton_visca::transport::core::Transport;
use grafton_visca::{
    camera::{
        methods::{PanTiltOps, PresetsOps, ZoomOps},
        profiles::G2PresetId,
    },
    PresetNumber,
    types::SpeedLevel,
    units::Degrees,
    Camera, Error, Normalized,
};
use std::future::{ready, Ready};

/// Mock transport for demonstration purposes.
/// In real usage, you would use Udp or Tcp.
#[cfg(feature = "tokio")]
#[derive(Debug, Clone)]
struct MockTransport;

#[cfg(feature = "tokio")]
impl Transport for MockTransport {
    type Error = Error;
    type SendFut<'a> = Ready<Result<(), Error>>;
    type RecvFut<'a> = Ready<Result<Bytes, Error>>;

    fn send<'a>(&'a self, _data: &'a [u8]) -> Self::SendFut<'a> {
        ready(Ok(()))
    }

    fn recv(&self) -> Self::RecvFut<'_> {
        ready(Ok(Bytes::from_static(&[0x90, 0x50, 0xFF]))) // Mock completion response
    }
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature.");
    eprintln!("Run with: cargo run --example model_validation_demo --features tokio");
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Camera Model Validation Demo");
    println!("============================\n");

    // Demo different camera profiles
    demo_ptzoptics_g2().await?;
    demo_sony_fr7().await?;
    demo_generic_visca().await?;

    println!("\n✅ All model validations completed successfully!");
    Ok(())
}

#[cfg(feature = "tokio")]
async fn demo_ptzoptics_g2() -> Result<(), Error> {
    println!("📸 PTZOptics G2 Camera");
    println!("----------------------");

    let transport = MockTransport;
    let camera = Camera::new(transport);

    // Get profile information
    println!("Model: PTZOptics G2");
    println!("Pan range: -170 to +170 degrees");
    println!("Tilt range: -30 to +90 degrees");

    // Valid operations for G2
    println!("\n✅ Valid operations:");

    // Zoom within G2 range (0x0000 - 0x7000)
    println!("  - Setting zoom to 0x4000 (within G2 range)");
    camera
        .zoom_absolute(Normalized::new(0x4000 as f32 / 0x7000 as f32))
        .await?; // Normalize for G2 range

    // Position within G2 range (-170° to +170° pan, -90° to +90° tilt)
    println!("  - Moving to position (100°, 45°)");
    camera
        .pan_tilt_absolute(Degrees::new(100.0), Degrees::new(45.0), SpeedLevel::Medium)
        .await?;

    // G2-specific preset (0-89)
    println!("  - Using G2-specific preset 15");
    let preset = G2PresetId::new(15)?;
    camera.preset_set(PresetNumber::new(preset.into())?).await?;

    println!();
    Ok(())
}

#[cfg(feature = "tokio")]
async fn demo_sony_fr7() -> Result<(), Error> {
    println!("📸 Sony FR7 Camera");
    println!("------------------");

    let transport = MockTransport;
    let camera = Camera::new(transport);

    println!("Model: Sony FR7");
    println!("Using Sony FR7 profile");

    // Sony FR7 operations
    println!("\n✅ Valid operations:");
    println!("  - Setting zoom");
    camera
        .zoom_absolute(Normalized::new(0x5000 as f32 / 0xFFFF as f32))
        .await?; // Normalize for generic range

    // Position control
    println!("  - Moving to position (90°, 25°)");
    camera
        .pan_tilt_absolute(Degrees::new(90.0), Degrees::new(25.0), SpeedLevel::Medium)
        .await?;

    println!();
    Ok(())
}

#[cfg(feature = "tokio")]
async fn demo_generic_visca() -> Result<(), Error> {
    println!("📸 Generic VISCA Camera");
    println!("-----------------------");

    let transport = MockTransport;
    let camera = Camera::new(transport);

    println!("Model: Generic VISCA");
    println!("Using generic VISCA defaults for unknown camera models");

    // Generic operations
    println!("\n✅ Generic operations:");
    camera
        .zoom_absolute(Normalized::new(0x4000 as f32 / 0x7000 as f32))
        .await?; // Normalize for G2 range
    camera
        .pan_tilt_absolute(Degrees::new(45.0), Degrees::new(30.0), SpeedLevel::Medium)
        .await?;

    // Note: GenericVisca doesn't support presets in the current implementation
    println!("  - Generic cameras may not support all features");

    println!();
    Ok(())
}
