//! Basic example of using the Camera API.
//!
//! This example demonstrates the simplicity of the new Camera API
//! which eliminates the need for generic type parameters.

use grafton_visca::Error;

#[cfg(not(feature = "tokio"))]
use grafton_visca::prelude::blocking::{GenericViscaCam, PTZOpticsG2Cam};

#[cfg(feature = "tokio")]
use grafton_visca::prelude::r#async::{GenericViscaCam, PTZOpticsG2Cam};

#[cfg(not(feature = "tokio"))]
use grafton_visca::transport::blocking::Tcp;

#[cfg(feature = "tokio")]
use grafton_visca::transport::tokio::Tcp;

#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== Camera Basic Example (Blocking) ===\n");

    // Create a camera with automatic profile detection (defaults to GenericVisca)
    // Generic VISCA cameras use raw protocol on port 5678 (TCP) or 1259 (UDP)
    let transport = Tcp::connect("192.168.0.110:5678")?;
    let _camera = GenericViscaCam::new(transport);

    println!("Using default profile: GenericVisca");

    // Create a camera with specific profile
    // PTZOptics cameras use raw VISCA protocol on port 5678 (TCP) or 1259 (UDP)
    let transport = Tcp::connect("192.168.0.110:5678")?;
    let _camera = PTZOpticsG2Cam::new(transport);

    println!("\nUsing specific profile: PTZOpticsG2");

    // Note: capability checking not available on blocking camera
    println!("\nCamera capabilities:");
    println!("  - Pan/Tilt: supported");
    println!("  - Zoom: supported");
    println!("  - Focus: supported");
    println!("  - Presets: supported");
    println!("  - ND Filter: camera-specific");

    // TODO: Add command examples once extension traits are implemented
    // camera.zoom_in()?;
    // camera.pan_tilt_home()?;

    println!("\n✅ Example complete!");

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== Camera Basic Example (Async) ===\n");

    // Create a camera with automatic profile detection (defaults to GenericVisca)
    // Generic VISCA cameras use raw protocol on port 5678 (TCP) or 1259 (UDP)
    let transport = Tcp::connect("192.168.0.110:5678").await?;
    let _camera = GenericViscaCam::new(transport);

    println!("Using default profile: GenericVisca");

    // Create a camera with specific profile
    // PTZOptics cameras use raw VISCA protocol on port 5678 (TCP) or 1259 (UDP)
    let transport = Tcp::connect("192.168.0.110:5678").await?;
    let _camera = PTZOpticsG2Cam::new(transport);

    println!("\nUsing specific profile: PTZOpticsG2");

    // Note: capability checking not yet implemented for async camera
    println!("\nCamera capabilities:");
    println!("  - Pan/Tilt: supported");
    println!("  - Zoom: supported");
    println!("  - Focus: supported");
    println!("  - Presets: supported");
    println!("  - ND Filter: camera-specific");

    // TODO: Add command examples once extension traits are implemented
    // camera.power_on().await?;
    // camera.zoom_in().await?;

    println!("\n✅ Example complete!");

    Ok(())
}
