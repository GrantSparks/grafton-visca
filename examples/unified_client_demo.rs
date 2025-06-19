//! Demo of the unified Camera API in both blocking and async contexts.
//!
//! This example shows how the new Camera API works seamlessly with
//! different transport adapters for blocking and async usage.

mod common;
use common::blocking::UdpTransport;
use common::r#async::AsyncTcpTransport;
use grafton_visca::{
    camera::{profiles::PTZOpticsG2, Camera},
    command::pan_tilt::PanTiltDirection,
    transport::BlockingAdapter,
    Error,
};
use std::time::Duration;

// Include the transport implementations from the example files
#[path = "udp_transport.rs"]
mod udp_transport;

#[cfg(feature = "async-client")]
#[path = "tcp_transport.rs"]
mod tcp_transport;
#[cfg(feature = "async-client")]

// Helper for using async code in sync context
fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(fut)
}

fn blocking_udp_example() -> Result<(), Error> {
    println!("=== Blocking UDP Example ===");

    // Create a camera with blocking UDP transport
    let transport = UdpTransport::new("192.168.1.100:5678")?;
    let camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(transport));

    // All operations are async but we use block_on for blocking execution
    println!("Powering on camera...");
    block_on(camera.power_on())?;

    println!("Moving to home position...");
    block_on(camera.home())?;

    println!("Zooming in...");
    block_on(camera.zoom_in())?;
    std::thread::sleep(Duration::from_secs(1));
    block_on(camera.zoom_stop())?;

    Ok(())
}

#[cfg(feature = "async-client")]
async fn async_tcp_example() -> Result<(), Error> {
    println!("\n=== Async TCP Example ===");

    // Create a camera with async TCP transport
    let transport = AsyncTcpTransport::new("192.168.1.100:5678").await?;
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
        let udp = UdpTransport::new("192.168.1.100:5678")?;
        let camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(udp));

        println!("UDP camera - moving up...");
        camera.move_continuous(PanTiltDirection::Up, 0, 10).await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        camera.stop().await?;
    }

    // Example 2: TCP async
    #[cfg(feature = "async-client")]
    {
        let tcp = AsyncTcpTransport::new("192.168.1.100:5678").await?;
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
    let transport = UdpTransport::new("192.168.1.100:5678")?;

    // PTZOptics G2 camera
    {
        use grafton_visca::camera::profiles::G2PresetId;
        let transport2 = UdpTransport::new("192.168.1.100:5678")?;
        let g2_camera = Camera::<PTZOpticsG2>::new(BlockingAdapter(transport2));

        println!("G2 Camera - saving preset 1...");
        let preset = G2PresetId::new(1)?;
        g2_camera.set_preset(preset).await?;
    }

    // Generic VISCA camera (wider compatibility)
    {
        use grafton_visca::camera::profiles::{GenericPresetId, GenericVisca};
        let generic_camera = Camera::<GenericVisca>::new(BlockingAdapter(transport));

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

    // Run blocking example
    blocking_udp_example()?;

    // Run async examples
    #[cfg(feature = "async-client")]
    async_tcp_example().await?;

    transport_flexibility_example().await?;
    profile_switching_example().await?;

    println!("\n=== Demo Complete ===");
    println!("\nKey takeaways:");
    println!("• Camera API is always async internally");
    println!("• BlockingAdapter allows sync usage with block_on");
    println!("• Works with any Transport implementation (UDP, TCP, custom)");
    println!("• Profile system provides type-safe camera-specific features");

    Ok(())
}
