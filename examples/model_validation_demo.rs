//! Camera model validation demonstration
//!
//! This example demonstrates how the Camera<P> API enforces model-specific
//! constraints at compile time and runtime. It shows how different camera profiles
//! (PTZOpticsG2, PTZOptics30X, SonyEVID70) have different ranges and capabilities.

#[cfg(feature = "async")]
use grafton_visca::transport::AsyncTransport;
use grafton_visca::{
    camera::{
        profiles::{G2PresetId, GenericVisca, PTZOptics30X, PTZOpticsG2, SonyEVID70},
        CameraProfile,
    },
    types::ZoomPosition,
    units::Degrees,
    units::Raw,
    Camera, Error,
};
use std::future::Future;
use std::pin::Pin;

/// Mock transport for demonstration purposes.
/// In real usage, you would use Udp or Tcp.
#[cfg(feature = "async")]
#[derive(Debug)]
struct MockTransport;

#[cfg(feature = "async")]
impl AsyncTransport for MockTransport {
    type SendFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;
    type ReceiveFuture<'a> = Pin<Box<dyn Future<Output = Result<Vec<u8>, Error>> + Send + 'a>>;

    fn send(&self, _data: &[u8]) -> Self::SendFuture<'_> {
        Box::pin(async { Ok(()) })
    }

    fn receive(&self) -> Self::ReceiveFuture<'_> {
        Box::pin(async { Ok(vec![0x90, 0x50, 0xFF]) }) // Mock completion response
    }
}

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the 'async' feature.");
    eprintln!("Run with: cargo run --example model_validation_demo --features async");
}

#[cfg(feature = "async")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Camera Model Validation Demo");
    println!("============================\n");

    // Demo different camera profiles
    demo_ptzoptics_g2().await?;
    demo_ptzoptics_30x().await?;
    demo_sony_evid70().await?;
    demo_generic_visca().await?;

    println!("\n✅ All model validations completed successfully!");
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_ptzoptics_g2() -> Result<(), Error> {
    println!("📸 PTZOptics G2 Camera");
    println!("----------------------");

    let transport = MockTransport;
    let camera = Camera::<PTZOpticsG2, _>::new(transport);

    // Get profile information
    let profile = camera.profile();
    println!("Model: {}", profile.model_name());
    println!("Pan range: {:?} degrees", profile.pan_degree_range());
    println!("Tilt range: {:?} degrees", profile.tilt_degree_range());

    // Valid operations for G2
    println!("\n✅ Valid operations:");

    // Zoom within G2 range (0x0000 - 0x7000)
    println!("  - Setting zoom to 0x4000 (within G2 range)");
    camera.set_zoom(Raw(0x4000u16)).await?;

    // Position within G2 range (-170° to +170° pan, -90° to +90° tilt)
    println!("  - Moving to position (100°, 45°)");
    camera.pan_tilt_absolute(Degrees(100.0), Degrees(45.0)).await?;

    // G2-specific preset (0-89)
    println!("  - Using G2-specific preset 15");
    let preset = G2PresetId::new(15)?;
    camera.preset_set(preset.into()).await?;

    println!();
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_ptzoptics_30x() -> Result<(), Error> {
    println!("📸 PTZOptics 30X Camera");
    println!("-----------------------");

    let transport = MockTransport;
    let camera = Camera::<PTZOptics30X, _>::new(transport);

    let profile = camera.profile();
    println!("Model: {}", profile.model_name());
    println!("Pan range: {:?} degrees", profile.pan_degree_range());
    println!("Tilt range: {:?} degrees", profile.tilt_degree_range());

    // 30X has larger zoom range than G2
    println!("\n✅ Valid operations:");
    println!("  - Setting zoom to 0x9000 (30X extended range)");
    camera.set_zoom(ZoomPosition::try_from(0x9000u16)?).await?;

    // Wider pan range than G2
    println!("  - Moving to position (175°, 45°)");
    camera.pan_tilt_absolute(Degrees(175.0), Degrees(45.0)).await?;

    println!();
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_sony_evid70() -> Result<(), Error> {
    println!("📸 Sony EVI-D70 Camera");
    println!("----------------------");

    let transport = MockTransport;
    let camera = Camera::<SonyEVID70, _>::new(transport);

    let profile = camera.profile();
    println!("Model: {}", profile.model_name());
    println!("Pan range: {:?} degrees", profile.pan_degree_range());
    println!("Tilt range: {:?} degrees", profile.tilt_degree_range());

    // Sony has different ranges
    println!("\n✅ Valid operations:");
    println!("  - Setting zoom to 0x5000");
    camera.set_zoom(ZoomPosition::try_from(0x5000u16)?).await?;

    // Sony has ±100° pan range
    println!("  - Moving to position (90°, 25°)");
    camera.pan_tilt_absolute(Degrees(90.0), Degrees(25.0)).await?;

    println!();
    Ok(())
}

#[cfg(feature = "async")]
async fn demo_generic_visca() -> Result<(), Error> {
    println!("📸 Generic VISCA Camera");
    println!("-----------------------");

    let transport = MockTransport;
    let camera = Camera::<GenericVisca, _>::new(transport);

    let profile = camera.profile();
    println!("Model: {}", profile.model_name());
    println!("Using generic VISCA defaults for unknown camera models");

    // Generic operations
    println!("\n✅ Generic operations:");
    camera.set_zoom(Raw(0x4000u16)).await?;
    camera.pan_tilt_absolute(Degrees(45.0), Degrees(30.0)).await?;

    // Generic presets
    println!("  - Using generic preset 1");
    camera.preset_recall(1).await?;

    println!();
    Ok(())
}
