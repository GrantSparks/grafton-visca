//! Advanced `CameraBuilder` demonstration.
//!
//! Normal applications should start with `Connect` or `CameraConfig`. Use
//! `CameraBuilder` when you already own a configured transport or custom
//! transport implementation and need to attach it to a typed camera.
//!
//! Run with:
//! - Blocking: cargo run --example builder_api
//! - Async: cargo run --example builder_api --features runtime-tokio

#[cfg(not(feature = "mode-async"))]
fn main() -> grafton_visca::Result<()> {
    use std::time::Duration;

    use grafton_visca::{
        camera::{
            profiles::{PtzOpticsG2, SonyFR7},
            CameraConfig, Connect,
        },
        transport::{TcpKeepaliveConfig, Transport, TransportConfig},
        CameraBuilder,
    };

    tracing_subscriber::fmt::init();

    println!("=== Advanced CameraBuilder Demo (Blocking) ===\n");

    println!("--- Preferred simple path: Connect ---");
    let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
    camera.power().on()?;
    camera.close()?;
    println!("✓ Opened PTZOptics G2 with Connect");

    println!("\n--- Preferred configured path: CameraConfig ---");
    let camera = CameraConfig::<SonyFR7>::new()
        .tcp()
        .address("192.168.0.108")
        .transport_config(TransportConfig {
            tcp_keepalive: Some(TcpKeepaliveConfig::new(Duration::from_secs(30))),
            ..TransportConfig::default()
        })
        .open_blocking()?;
    camera.power().state()?;
    camera.close()?;
    println!("✓ Opened Sony FR7 with CameraConfig");

    println!("\n--- Advanced BYO-transport path: CameraBuilder ---");
    let transport = Transport::udp()
        .address("192.168.0.110:1259")
        .max_retries(5)
        .retry_delay(Duration::from_millis(50))
        .build_blocking()?;
    let camera = CameraBuilder::from_transport_handle(transport)
        .profile::<PtzOpticsG2>()
        .open()?;
    camera.zoom().stop()?;
    camera.close()?;
    println!("✓ Attached a configured UDP transport with CameraBuilder");

    Ok(())
}

#[cfg(all(
    feature = "mode-async",
    not(any(feature = "runtime-tokio", feature = "runtime-smol"))
))]
fn main() -> grafton_visca::Result<()> {
    println!("=== Advanced CameraBuilder Demo ===\n");
    println!("This example requires a specific async runtime feature:");
    println!("- Run with: cargo run --example builder_api --features runtime-tokio");
    println!("- Or with:  cargo run --example builder_api --features runtime-smol");
    println!("- Or for blocking: cargo run --example builder_api (no features)");
    Ok(())
}

#[cfg(feature = "runtime-tokio")]
#[tokio::main]
async fn main() -> grafton_visca::Result<()> {
    use std::time::Duration;

    use grafton_visca::{
        camera::{
            profiles::{PtzOpticsG2, SonyFR7},
            CameraConfig, Connect,
        },
        runtime::TokioRuntime,
        runtime_adapters::tokio::UdpTransport,
        transport::{RetryConfig, TcpKeepaliveConfig, TransportConfig},
        CameraBuilder,
    };

    tracing_subscriber::fmt::init();

    println!("=== Advanced CameraBuilder Demo (Tokio) ===\n");

    println!("--- Preferred simple path: Connect ---");
    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", runtime).await?;
    camera.power().on().await?;
    camera.close().await?;
    println!("✓ Opened PTZOptics G2 with Connect");

    println!("\n--- Preferred configured path: CameraConfig ---");
    let runtime = TokioRuntime::from_current()?;
    let camera = CameraConfig::<SonyFR7>::new()
        .tcp()
        .address("192.168.0.108")
        .transport_config(TransportConfig {
            tcp_keepalive: Some(TcpKeepaliveConfig::new(Duration::from_secs(30))),
            ..TransportConfig::default()
        })
        .open_async(runtime)
        .await?;
    camera.power().state().await?;
    camera.close().await?;
    println!("✓ Opened Sony FR7 with CameraConfig");

    println!("\n--- Advanced BYO-transport path: CameraBuilder ---");
    let runtime = TokioRuntime::from_current()?;
    let transport = UdpTransport::connect_with_config(
        "192.168.0.110:1259",
        TransportConfig {
            retry_config: RetryConfig {
                max_retries: 5,
                base_retry_delay: Duration::from_millis(50),
                ..RetryConfig::default()
            },
            ..TransportConfig::default()
        },
    )
    .await?;
    let camera = CameraBuilder::with_executor(runtime)
        .from_transport(transport)
        .profile::<PtzOpticsG2>()
        .open_async()
        .await?;
    camera.zoom().stop().await?;
    camera.shutdown().await?;
    println!("✓ Attached a configured UDP transport with CameraBuilder");

    Ok(())
}

#[cfg(all(feature = "runtime-smol", not(feature = "runtime-tokio")))]
fn main() -> grafton_visca::Result<()> {
    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Connect},
        runtime::SmolRuntime,
        runtime_adapters::smol::UdpTransport,
        CameraBuilder,
    };

    tracing_subscriber::fmt::init();

    smol::block_on(async {
        println!("=== Advanced CameraBuilder Demo (smol) ===\n");

        println!("--- Preferred simple path: Connect ---");
        let camera =
            Connect::open_tcp_async::<PtzOpticsG2, _>("192.168.0.110", SmolRuntime::new()).await?;
        camera.power().on().await?;
        camera.close().await?;
        println!("✓ Opened PTZOptics G2 with Connect");

        println!("\n--- Advanced BYO-transport path: CameraBuilder ---");
        let transport = UdpTransport::connect("192.168.0.110:1259").await?;
        let camera = CameraBuilder::with_executor(SmolRuntime::new())
            .from_transport(transport)
            .profile::<PtzOpticsG2>()
            .open_async()
            .await?;
        camera.zoom().stop().await?;
        camera.shutdown().await?;
        println!("✓ Attached a UDP transport with CameraBuilder");

        Ok(())
    })
}
