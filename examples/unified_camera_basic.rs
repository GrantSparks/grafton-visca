//! Basic example of using the Camera API.
//!
//! This example demonstrates the simplicity of the new Camera API
//! which eliminates the need for generic type parameters.

use grafton_visca::{Camera, Error, ProfileId};

#[cfg(not(feature = "tokio"))]
use grafton_visca::transport::blocking::Tcp;

#[cfg(feature = "tokio")]
use grafton_visca::transport::tokio::Tcp;

#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== Camera Basic Example (Blocking) ===\n");

    // Create a camera with automatic profile detection (defaults to GenericVisca)
    let transport = Tcp::connect("192.168.1.100:52381")?;
    let camera = Camera::new_blocking(transport);

    println!("Connected to: {}", camera.model_name());
    println!("Profile info: {}", camera.profile_info());

    // Create a camera with specific profile
    let transport = Tcp::connect("192.168.1.100:52381")?;
    let camera = Camera::with_profile_blocking(ProfileId::PTZOpticsG2, transport);

    println!("\nUsing specific profile: {}", camera.model_name());

    // Check capabilities
    println!("\nCamera capabilities:");
    println!("  - Pan/Tilt: {}", camera.supports_capability("pan_tilt"));
    println!("  - Zoom: {}", camera.supports_capability("zoom"));
    println!("  - Focus: {}", camera.supports_capability("focus"));
    println!("  - Presets: {}", camera.supports_capability("presets"));
    println!("  - ND Filter: {}", camera.supports_capability("nd_filter"));

    // TODO: Add command examples once extension traits are implemented
    // camera.power_on_blocking()?;
    // camera.zoom_in_blocking()?;

    println!("\n✅ Example complete!");

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== Camera Basic Example (Async) ===\n");

    // Create a camera with automatic profile detection (defaults to GenericVisca)
    let transport = Tcp::connect("192.168.1.100:52381").await?;
    let camera = Camera::new(transport);

    println!("Connected to: {}", camera.model_name());
    println!("Profile info: {}", camera.profile_info());

    // Create a camera with specific profile
    let transport = Tcp::connect("192.168.1.100:52381").await?;
    let camera = Camera::with_profile(ProfileId::PTZOpticsG2, transport);

    println!("\nUsing specific profile: {}", camera.model_name());

    // Check capabilities
    println!("\nCamera capabilities:");
    println!("  - Pan/Tilt: {}", camera.supports_capability("pan_tilt"));
    println!("  - Zoom: {}", camera.supports_capability("zoom"));
    println!("  - Focus: {}", camera.supports_capability("focus"));
    println!("  - Presets: {}", camera.supports_capability("presets"));
    println!("  - ND Filter: {}", camera.supports_capability("nd_filter"));

    // TODO: Add command examples once extension traits are implemented
    // camera.power_on().await?;
    // camera.zoom_in().await?;

    println!("\n✅ Example complete!");

    Ok(())
}
