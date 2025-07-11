//! Demo of the new GAT-based transport architecture.
//! 
//! This example shows how the unified transport trait works for both
//! blocking and async implementations.

// Example usage with blocking transport
#[cfg(not(feature = "tokio"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        camera::{CameraAsync, CameraBlocking, BlockingExt},
        profiles::PTZOpticsG2,
        transport::blocking::tcp_gat::TcpGat,
    };

    // Create a blocking TCP transport
    let transport = TcpGat::connect("192.168.1.100:5678")?;
    
    // Create camera with async interface
    let camera = CameraAsync::<PTZOpticsG2, _>::new(transport);
    
    // Convert to blocking interface
    let camera = camera.blocking();
    
    println!("Camera model: {}", camera.profile_info());
    
    // Future camera methods would be used like:
    // camera.power_on()?;
    // camera.zoom_stop()?;
    
    Ok(())
}

// Example usage with async transport
#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        camera::CameraAsync,
        profiles::PTZOpticsG2,
        transport::tokio::tcp_gat::TcpGat,
    };

    // Create an async TCP transport
    let transport = TcpGat::connect("192.168.1.100:5678").await?;
    
    // Create camera with async interface
    let camera = CameraAsync::<PTZOpticsG2, _>::new(transport);
    
    println!("Camera model: {}", camera.profile_info());
    
    // Future camera methods would be used like:
    // camera.power_on().await?;
    // camera.zoom_stop().await?;
    
    Ok(())
}