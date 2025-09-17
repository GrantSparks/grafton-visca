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
//! - Async: cargo run --example builder_api --features runtime-tokio

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{
    camera::profiles::{PtzOpticsG2, SonyBRC300, SonyFR7},
    CameraBuilder, Result,
};

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<()> {
    // Direct transport types are no longer available - use CameraBuilder instead

    tracing_subscriber::fmt::init();

    println!("=== CameraBuilder API Demo (Blocking) ===\n");

    println!("--- Example 1: Simple TCP with Default Port ---");
    let _camera = CameraBuilder::tcp("192.168.0.110:5678")
        .profile::<PtzOpticsG2>()
        .open()?;
    println!("✓ Created PTZOptics G2 camera on TCP port 5678");

    println!("\n--- Example 2: TCP with Custom Port ---");
    let _camera = CameraBuilder::tcp("192.168.0.110:52381")
        .profile::<PtzOpticsG2>()
        .open()?;
    println!("✓ Created camera with custom port 52381");

    println!("\n--- Example 3: UDP Transport ---");
    let _camera = CameraBuilder::udp("192.168.0.110:1259")
        .profile::<PtzOpticsG2>()
        .open()?;
    println!("✓ Created camera on UDP port 1259");

    println!("\n--- Example 4: Camera Profiles ---");

    let _generic = CameraBuilder::tcp("192.168.0.110:5678")
        .profile::<PtzOpticsG2>()
        .open()?;
    println!("✓ PTZOptics G2 camera (used as generic example)");

    let _sony_brc = CameraBuilder::tcp("192.168.0.109:52381")
        .profile::<SonyBRC300>()
        .open()?;
    println!("✓ Sony BRC-300 camera (encapsulated protocol)");

    let _sony_fr7 = CameraBuilder::tcp("192.168.0.108:5678")
        .profile::<SonyFR7>()
        .open()?;
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

#[cfg(all(
    feature = "mode-async",
    not(any(
        feature = "runtime-tokio",
        feature = "runtime-async-std",
        feature = "runtime-smol"
    ))
))]
fn main() -> grafton_visca::Result<()> {
    println!("=== CameraBuilder API Demo ===\n");
    println!("This example requires a specific async runtime feature:");
    println!("- Run with: cargo run --example builder_api --features runtime-tokio");
    println!("- Or with:  cargo run --example builder_api --features runtime-async-std");
    println!("- Or with:  cargo run --example builder_api --features runtime-smol");
    println!("- Or for blocking: cargo run --example builder_api (no features)");
    Ok(())
}

#[cfg(all(
    feature = "runtime-tokio",
    not(any(feature = "runtime-async-std", feature = "runtime-smol"))
))]
#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    use grafton_visca::camera::profiles::{PtzOpticsG2, SonyBRC300, SonyFR7};
    use grafton_visca::runtime::TokioRuntime;
    use grafton_visca::runtime_adapters::tokio::{TcpTransport as Tcp, UdpTransport as Udp};
    use grafton_visca::CameraBuilder;

    tracing_subscriber::fmt::init();

    println!("=== CameraBuilder API Demo (Async) ===\n");

    println!("--- Example 1: Tokio TCP ---");
    let transport = Tcp::connect("192.168.0.110:5678").await?;
    let runtime = TokioRuntime::from_current()?;
    let _camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await?;
    println!("✓ Created async TCP camera with tokio");

    println!("\n--- Example 2: Tokio UDP ---");
    let transport = Udp::connect("192.168.0.110:1259").await?;
    let runtime = TokioRuntime::from_current()?;
    let _camera = CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await?;
    println!("✓ Created async UDP camera with tokio");

    println!("\n--- Example 3: Concurrent Creation ---");
    use tokio::join;

    async fn create_camera<P: grafton_visca::capabilities::Profile + Default>(
        addr: &str,
    ) -> grafton_visca::Result<()> {
        let transport = Tcp::connect(addr).await?;
        let runtime = TokioRuntime::from_current()?;
        let _camera = CameraBuilder::with_executor(runtime)
            .open_async::<P, _>(transport)
            .await?;
        Ok(())
    }

    let (cam1, cam2, cam3) = join!(
        create_camera::<PtzOpticsG2>("192.168.0.110:5678"),
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
        Ok(transport) => {
            let runtime = TokioRuntime::from_current()?;
            match CameraBuilder::with_executor(runtime)
                .open_async::<PtzOpticsG2, _>(transport)
                .await
            {
                Ok(_) => println!("Unexpected success"),
                Err(e) => println!("✓ Handled camera build error: {e}"),
            }
        }
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

#[cfg(all(feature = "runtime-async-std", not(feature = "runtime-smol")))]
fn main() -> grafton_visca::Result<()> {
    use async_std::task;
    use grafton_visca::camera::profiles::PtzOpticsG2;
    use grafton_visca::runtime_adapters::async_std::{TcpTransport as Tcp, UdpTransport as Udp};
    use grafton_visca::CameraBuilder;

    tracing_subscriber::fmt::init();

    println!("=== CameraBuilder API Demo (Async-std) ===\n");

    task::block_on(async {
        println!("--- Example 1: async-std TCP ---");
        let transport = Tcp::connect("192.168.0.110:5678").await?;
        let runtime = grafton_visca::runtime::AsyncStdRuntime::new();
        let _camera = CameraBuilder::with_executor(runtime)
            .open_async::<PtzOpticsG2, _>(transport)
            .await?;
        println!("✓ Created async TCP camera with async-std");

        println!("\n--- Example 2: async-std UDP ---");
        let transport = Udp::connect("192.168.0.110:1259").await?;
        let runtime = grafton_visca::runtime::AsyncStdRuntime::new();
        let _camera = CameraBuilder::with_executor(runtime)
            .open_async::<PtzOpticsG2, _>(transport)
            .await?;
        println!("✓ Created async UDP camera with async-std");

        println!("\n✓ All async-std builder examples completed!");

        Ok(())
    })
}

#[cfg(feature = "runtime-smol")]
fn main() -> grafton_visca::Result<()> {
    use grafton_visca::camera::profiles::PtzOpticsG2;
    use grafton_visca::runtime_adapters::smol::{TcpTransport as Tcp, UdpTransport as Udp};
    use grafton_visca::CameraBuilder;

    tracing_subscriber::fmt::init();

    println!("=== CameraBuilder API Demo (Smol) ===\n");

    smol::block_on(async {
        println!("--- Example 1: smol TCP ---");
        let transport = Tcp::connect("192.168.0.110:5678").await?;
        let runtime = grafton_visca::runtime::SmolRuntime::new();
        let _camera = CameraBuilder::with_executor(runtime)
            .open_async::<PtzOpticsG2, _>(transport)
            .await?;
        println!("✓ Created async TCP camera with smol");

        println!("\n--- Example 2: smol UDP ---");
        let transport = Udp::connect("192.168.0.110:1259").await?;
        let runtime = grafton_visca::runtime::SmolRuntime::new();
        let _camera = CameraBuilder::with_executor(runtime)
            .open_async::<PtzOpticsG2, _>(transport)
            .await?;
        println!("✓ Created async UDP camera with smol");

        println!("\n✓ All smol builder examples completed!");

        Ok(())
    })
}
