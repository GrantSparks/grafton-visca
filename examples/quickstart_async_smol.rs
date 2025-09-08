//! Example of using grafton-visca with the smol runtime.
//!
//! This example demonstrates how to use the library with smol for async operations,
//! using the preferred high-level Camera API (not raw byte sends).
//!
//! # Usage
//! ```bash
//! cargo run --example quickstart_async_smol --features rt-smol
//! ```

#[cfg(not(feature = "rt-smol"))]
fn main() {
    eprintln!("This example requires the 'rt-smol' feature.");
    eprintln!("Run with: cargo run --example quickstart_async_smol --features rt-smol");
}

#[cfg(feature = "rt-smol")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    smol::block_on(async_main())
}

#[cfg(feature = "rt-smol")]
async fn async_main() -> Result<(), Box<dyn std::error::Error>> {
    use grafton_visca::{
        camera::{
            controls::{pan_tilt::PanTiltControl, power::PowerControl, zoom::ZoomControl},
            profiles::PtzOpticsG2,
            Camera,
        },
        runtime_trait::SmolRuntime,
    };

    // Preferred: build a camera and use high-level methods
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:5678".into());
    println!("Connecting to camera at {addr} with smol...");
    // Create the smol runtime and connect with type-safe pairing
    let runtime = SmolRuntime::new();
    let camera = Camera::open_tcp_async::<PtzOpticsG2, _>(addr, runtime).await?;

    println!("Powering on...\n");
    camera.power_on().await?;

    let is_on = camera.power().state().await?;
    println!("Power state: {}", if is_on { "ON" } else { "OFF" });

    camera.zoom_stop().await?;
    camera.pan_tilt_home().await?;

    println!("smol quickstart (high-level) completed successfully!");
    Ok(())
}
