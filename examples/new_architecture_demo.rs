//! Demo of the new GAT-based transport architecture.
//!
//! This example shows how the unified transport trait works for both
//! blocking and async implementations.

// Example usage with blocking transport
#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{profiles::PTZOpticsG2, transport::blocking::Tcp, CameraBlocking};

    // Create a blocking TCP transport
    let transport = Tcp::connect("192.168.1.100:5678")?;

    // Create camera with blocking interface
    let _camera = CameraBlocking::<PTZOpticsG2, _>::new(transport);

    println!("Camera model: PTZOptics G2");

    // Future camera methods would be used like:
    // camera.power_on()?;
    // camera.zoom_stop()?;

    Ok(())
}

// Example usage with async transport
#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{profiles::PTZOpticsG2, transport::tokio::tcp::Tcp, Camera};

    // Create an async TCP transport
    let transport = Tcp::connect("192.168.1.100:5678").await?;

    // Create camera with async interface
    let _camera = Camera::<PTZOpticsG2, _>::new(transport);

    println!("Camera model: PTZOptics G2");

    // Future camera methods would be used like:
    // camera.power_on().await?;
    // camera.zoom_stop().await?;

    Ok(())
}
