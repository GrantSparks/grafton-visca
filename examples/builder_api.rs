//! CameraBuilder API demonstration.
//!
//! This example shows all the ways to create cameras using the builder pattern:
//! - Blocking vs async transports
//! - TCP vs UDP protocols
//! - Different camera profiles
//! - Custom configurations
//!
//! Run with:
//! - Blocking: cargo run --example builder_api
//! - Async: cargo run --example builder_api --features rt-tokio

use grafton_visca::{
    camera::profiles::{PTZOpticsG2, SonyBRC300, SonyFR7},
    CameraBuilder, Result,
};

#[cfg(not(feature = "rt-tokio"))]
fn main() -> Result<()> {
    use grafton_visca::transport::{BlockingTcp, BlockingUdp};

    env_logger::init();

    println!("=== CameraBuilder API Demo (Blocking) ===\n");

    println!("--- Example 1: Simple TCP with Default Port ---");
    let transport = BlockingTcp::connect("192.168.0.110:5678")?;
    let _camera = CameraBuilder::new().build_blocking::<PTZOpticsG2, _>(transport);
    println!("✓ Created PTZOptics G2 camera on TCP port 5678");

    println!("\n--- Example 2: TCP with Custom Port ---");
    let transport = BlockingTcp::connect("192.168.0.110:52381")?;
    let _camera = CameraBuilder::new().build_blocking::<PTZOpticsG2, _>(transport);
    println!("✓ Created camera with custom port 52381");

    println!("\n--- Example 3: UDP Transport ---");
    let transport = BlockingUdp::connect("192.168.0.110:1259")?;
    let _camera = CameraBuilder::new().build_blocking::<PTZOpticsG2, _>(transport);
    println!("✓ Created camera on UDP port 1259");

    println!("\n--- Example 4: Camera Profiles ---");

    let transport = BlockingTcp::connect("192.168.0.110:5678")?;
    let _generic = CameraBuilder::new().build_blocking::<PTZOpticsG2, _>(transport);
    println!("✓ PTZOptics G2 camera (used as generic example)");

    let transport = BlockingTcp::connect("192.168.0.109:52381")?;
    let _sony_brc = CameraBuilder::new().build_blocking::<SonyBRC300, _>(transport);
    println!("✓ Sony BRC-300 camera (encapsulated protocol)");

    let transport = BlockingTcp::connect("192.168.0.108:5678")?;
    let _sony_fr7 = CameraBuilder::new().build_blocking::<SonyFR7, _>(transport);
    println!("✓ Sony FR7 camera (ND filter support)");

    println!("\n--- Example 5: Type Safety ---");
    println!("The builder enforces correct usage at compile time:");
    println!("- Transport must match the mode (blocking/async)");
    println!("- Profile must implement the Profile trait");
    println!("- Executor required for async, not for blocking");
    println!("- Strongly typed to prevent runtime mismatches");

    println!("\n✓ All builder examples completed successfully!");

    Ok(())
}

#[cfg(feature = "rt-tokio")]
#[tokio::main]
async fn main() -> Result<()> {
    use grafton_visca::transport::tokio::{Tcp, Udp};

    env_logger::init();

    println!("=== CameraBuilder API Demo (Async) ===\n");

    println!("--- Example 1: Tokio TCP ---");
    let transport = Tcp::connect("192.168.0.110:5678").await?;
    let _camera = CameraBuilder::tokio()?
        .build_async::<PTZOpticsG2, _>(transport)
        .await?;
    println!("✓ Created async TCP camera with tokio");

    println!("\n--- Example 2: Tokio UDP ---");
    let transport = Udp::connect("192.168.0.110:1259").await?;
    let _camera = CameraBuilder::tokio()?
        .build_async::<PTZOpticsG2, _>(transport)
        .await?;
    println!("✓ Created async UDP camera with tokio");

    println!("\n--- Example 3: Concurrent Creation ---");
    use tokio::join;

    async fn create_camera<P: grafton_visca::capabilities::Profile>(addr: &str) -> Result<()> {
        let transport = Tcp::connect(addr).await?;
        let _camera = CameraBuilder::tokio()?
            .build_async::<P, _>(transport)
            .await?;
        Ok(())
    }

    let (cam1, cam2, cam3) = join!(
        create_camera::<PTZOpticsG2>("192.168.0.110:5678"),
        create_camera::<SonyBRC300>("192.168.0.111:52381"),
        create_camera::<SonyFR7>("192.168.0.112:5678")
    );

    let mut created = 0;
    if cam1.is_ok() {
        created += 1;
    }
    if cam2.is_ok() {
        created += 1;
    }
    if cam3.is_ok() {
        created += 1;
    }
    println!("✓ Created {created}/3 cameras concurrently");

    println!("\n--- Example 4: Connection Error Handling ---");
    match Tcp::connect("invalid.host:5678").await {
        Ok(transport) => match CameraBuilder::tokio()?
            .build_async::<PTZOpticsG2, _>(transport)
            .await
        {
            Ok(_) => println!("Unexpected success"),
            Err(e) => println!("✓ Handled camera build error: {e}"),
        },
        Err(e) => println!("✓ Handled connection error: {e}"),
    }

    println!("\n--- Example 5: Async Benefits ---");
    println!("Async builders enable:");
    println!("- Non-blocking I/O during connection");
    println!("- Concurrent camera initialization");
    println!("- Integration with async ecosystems");
    println!("- Better resource utilization");

    println!("\n✓ All async builder examples completed!");

    Ok(())
}
