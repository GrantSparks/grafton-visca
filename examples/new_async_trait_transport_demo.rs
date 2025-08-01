//! Demo of the new async-trait based transport API.
//!
//! This example demonstrates how to use the new async-trait based transports
//! that eliminate GATs and unsafe code.

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    use grafton_visca::{
        camera::methods::PanTiltOps,
        prelude::r#async::PTZOpticsG2Cam,
        transport::tokio_adapter::{TcpTransport, UdpTransport},
        types::SpeedLevel,
        units::Degrees,
    };

    println!("=== New Async-Trait Transport API Demo ===");

    // Connect using the new TCP transport
    let tcp_transport = TcpTransport::connect("192.168.1.100:1259").await?;
    println!("Connected via TCP (new async-trait transport)");

    // Create camera using the new transport API
    let camera = PTZOpticsG2Cam::new_async(tcp_transport);

    // Stop any ongoing movement
    println!("Stopping camera movement...");
    camera.pan_tilt_stop().await?;

    // Move to center position
    println!("Moving to center position...");
    camera
        .pan_tilt_absolute(Degrees::new(0.0), Degrees::new(0.0), SpeedLevel::Medium)
        .await?;

    // Move camera to home position
    println!("Moving to home position...");
    camera.pan_tilt_home().await?;

    println!("\n=== UDP Transport Demo ===");
    
    // Connect using UDP transport
    let udp_transport = UdpTransport::connect("0.0.0.0:0", "192.168.1.100:52381").await?;
    println!("Connected via UDP (new async-trait transport)");

    let udp_camera = PTZOpticsG2Cam::new_async(udp_transport);
    
    // Test with UDP
    println!("Testing UDP transport...");
    udp_camera.pan_tilt_stop().await?;

    println!("\n=== Blocking Transport Demo ===");
    
    // For blocking transports, we can use the blocking adapters
    use grafton_visca::transport::blocking_adapter::TcpTransportBlocking;
    
    let blocking_transport = TcpTransportBlocking::connect("192.168.1.100:1259")?;
    println!("Connected via blocking TCP");
    
    let blocking_camera = PTZOpticsG2Cam::new_blocking(blocking_transport);
    
    // Since this is an async example, we'll use the async methods
    println!("Testing blocking transport with async methods...");
    blocking_camera.pan_tilt_stop().await?;

    println!("\nDemo completed successfully!");
    Ok(())
}

#[cfg(not(feature = "tokio"))]
fn main() {
    eprintln!("This example requires the 'tokio' feature");
}