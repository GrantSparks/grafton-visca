//! CameraBuilder API demonstration.
//!
//! This example shows all the ways to create cameras using the builder pattern:
//! - Blocking vs async transports
//! - TCP vs UDP protocols
//! - Different camera profiles
//! - Automatic port selection
//! - Custom configurations
//!
//! Run with:
//! - Blocking: cargo run --example builder_api
//! - Async: cargo run --example builder_api --features tokio

use grafton_visca::{
    camera::profiles::{PTZOpticsG2, SonyBRC300, SonyFR7},
    CameraBuilder, Result,
};

#[cfg(not(feature = "tokio"))]
fn main() -> Result<()> {
    env_logger::init();

    println!("=== CameraBuilder API Demo (Blocking) ===\n");

    println!("--- Example 1: Simple TCP with Auto Port ---");
    let _camera = CameraBuilder::tcp("192.168.0.110")
        .profile::<PTZOpticsG2>()
        .build()?;
    println!("✓ Created PTZOptics G2 camera on TCP port 5678");

    println!("\n--- Example 2: Explicit Port ---");
    let _camera = CameraBuilder::tcp("192.168.0.110:5678")
        .profile::<PTZOpticsG2>()
        .build()?;
    println!("✓ Created camera with explicit port 5678");

    println!("\n--- Example 3: UDP Transport ---");
    let _camera = CameraBuilder::udp("192.168.0.110")
        .profile::<PTZOpticsG2>()
        .build()?;
    println!("✓ Created camera on UDP port 1259");

    println!("\n--- Example 4: Camera Profiles ---");

    let _generic = CameraBuilder::tcp("192.168.1.100")
        .profile::<PTZOpticsG2>()
        .build()?;
    println!("✓ PTZOptics G2 camera (used as generic example)");

    let _sony_brc = CameraBuilder::tcp("192.168.1.101:52381")
        .profile::<SonyBRC300>()
        .build()?;
    println!("✓ Sony BRC-300 camera (encapsulated protocol)");

    let _sony_fr7 = CameraBuilder::tcp("192.168.1.102")
        .profile::<SonyFR7>()
        .build()?;
    println!("✓ Sony FR7 camera (ND filter support)");

    println!("\n--- Example 5: Type Safety ---");
    println!("The builder enforces correct usage at compile time:");
    println!("- Must call .profile() before .build()");
    println!("- Cannot call .profile() twice");
    println!("- Profile must implement the Profile trait");
    println!("- Strongly typed path prevents transport mismatches");

    println!("\n✓ All builder examples completed successfully!");

    Ok(())
}

#[cfg(feature = "tokio")]
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    println!("=== CameraBuilder API Demo (Async) ===\n");

    println!("--- Example 1: Tokio TCP ---");
    let _camera = CameraBuilder::tokio_tcp("192.168.0.110")
        .profile::<PTZOpticsG2>()
        .build()
        .await?;
    println!("✓ Created async TCP camera with tokio");

    println!("\n--- Example 2: Tokio UDP ---");
    let _camera = CameraBuilder::tokio_udp("192.168.0.110")
        .profile::<PTZOpticsG2>()
        .build()
        .await?;
    println!("✓ Created async UDP camera with tokio");

    println!("\n--- Example 3: Concurrent Creation ---");
    use tokio::join;

    let (cam1, cam2, cam3) = join!(
        CameraBuilder::tokio_tcp("192.168.0.110")
            .profile::<PTZOpticsG2>()
            .build(),
        CameraBuilder::tokio_tcp("192.168.1.101")
            .profile::<SonyBRC300>()
            .build(),
        CameraBuilder::tokio_tcp("192.168.1.102")
            .profile::<SonyFR7>()
            .build()
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
    match CameraBuilder::tokio_tcp("invalid.host:5678")
        .profile::<PTZOpticsG2>()
        .build()
        .await
    {
        Ok(_) => println!("Unexpected success"),
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
