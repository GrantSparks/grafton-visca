//! Quickstart example using async-std runtime
//!
//! This example demonstrates basic async camera control using async-std runtime,
//! using the preferred high-level Camera API (not raw byte sends).
//!
//! Run with:
//! ```bash
//! cargo run --example quickstart_async_asyncstd --features rt-async-std
//! ```

#![cfg(feature = "rt-async-std")]

use grafton_visca::{
    camera::{
        methods::{pan_tilt::PanTiltControl, power::PowerControl, zoom::ZoomControl},
        profiles::PtzOpticsG2,
    },
    transport::Transport,
    CameraBuilder,
};
use std::error::Error;

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    // Preferred: build a camera and use high-level methods
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "192.168.0.110:5678".into());
    println!("Connecting to camera at {addr} with async-std...");
    // Use TransportBuilder for native async-std TCP transport
    let transport = Transport::tcp().address(&addr).connect().await?;

    let camera = CameraBuilder::async_std()
        .build_async::<PtzOpticsG2, _>(transport)
        .await?;

    // High-level control
    println!("Powering on...\n");
    camera.power_on().await?;

    // High-level inquiry
    let is_on = camera.power_inquiry().await?;
    println!("Power state: {}", if is_on { "ON" } else { "OFF" });

    // A couple more high-level calls
    camera.zoom_stop().await?;
    camera.pan_tilt_home().await?;

    println!("async-std quickstart (high-level) completed successfully!");
    Ok(())
}
