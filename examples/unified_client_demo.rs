//! Demo of the unified Camera API in both blocking and async contexts.
//!
//! This example shows how the new Camera API works seamlessly with
//! different transport adapters for blocking and async usage.

use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::pan_tilt::PanTiltDirection,
    transport::{create, TcpTransport, UdpTransport, ViscaTransport},
    Error,
};
use std::time::Duration;

async fn udp_example() -> Result<(), Error> {
    println!("=== UDP Example ===");

    // Create a camera with UDP transport
    let transport = create::udp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // All operations are async
    println!("Powering on camera...");
    camera.power_on().await?;

    println!("Moving to home position...");
    camera.home().await?;

    println!("Zooming in...");
    camera.zoom_in().await?;
    tokio::time::sleep(Duration::from_secs(1)).await;
    camera.zoom_stop().await?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn async_tcp_example() -> Result<(), Error> {
    println!("\n=== Async TCP Example ===");

    // Create a camera with async TCP transport
    let transport = create::tcp("192.168.1.100:5678").await?;
    let camera = Camera::<PTZOpticsG2>::new(transport);

    // All operations are naturally async
    println!("Powering on camera...");
    camera.power_on().await?;

    println!("Setting position...");
    use grafton_visca::camera::units::Degrees;
    camera.set_position(Degrees(45.0), Degrees(15.0)).await?;

    println!("Adjusting focus...");
    camera.focus_auto().await?;

    Ok(())
}

async fn transport_flexibility_example() -> Result<(), Error> {
    println!("\n=== Transport Flexibility Example ===");

    // The Camera API works with any transport implementation

    // Example 1: UDP with blocking adapter
    {
        let udp = create::udp("192.168.1.100:5678").await?;
        let camera = Camera::<PTZOpticsG2>::new(udp);

        println!("UDP camera - moving up...");
        camera.move_continuous(PanTiltDirection::Up, 0, 10).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        camera.stop().await?;
    }

    // Example 2: TCP async
    #[cfg(feature = "async-client")]
    {
        let tcp = tcp_transport("192.168.1.100:5678").await?;
        let camera = Camera::<PTZOpticsG2>::new(tcp);

        println!("TCP camera - moving down...");
        camera
            .move_continuous(PanTiltDirection::Down, 0, 10)
            .await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        camera.stop().await?;
    }

    Ok(())
}

async fn profile_switching_example() -> Result<(), Error> {
    println!("\n=== Profile Switching Example ===");

    // You can use different profiles for different camera models
    let transport = common::blocking::udp_transport("192.168.1.100:5678")?;

    // PTZOptics G2 camera
    {
        use grafton_visca::camera::profiles::G2PresetId;
        let transport2 = create::udp("192.168.1.100:5678").await?;
        let g2_camera = Camera::<PTZOpticsG2>::new(transport2);

        println!("G2 Camera - saving preset 1...");
        let preset = G2PresetId::new(1)?;
        g2_camera.set_preset(preset).await?;
    }

    // Generic VISCA camera (wider compatibility)
    {
        use grafton_visca::camera::profiles::{GenericPresetId, GenericVisca};
        let generic_camera = Camera::<GenericVisca>::new(transport);

        println!("Generic Camera - recalling preset 0...");
        let preset = GenericPresetId::new(0);
        generic_camera.recall_preset(preset).await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();

    println!("=== Unified Camera API Demo ===\n");
    println!("This demo shows how the Camera API works with different transports");
    println!("and in both blocking and async contexts.\n");

    // Run examples
    udp_example().await?;
    async_tcp_example().await?;

    transport_flexibility_example().await?;
    profile_switching_example().await?;

    println!("\n=== Demo Complete ===");
    println!("\nKey takeaways:");
    println!("• Camera API provides async operations");
    println!("• Works with any Transport implementation (UDP, TCP, custom)");
    println!("• Supports multiple camera profiles with type safety");
    println!("• Profile system provides type-safe camera-specific features");

    Ok(())
}
