//! Tokio runtime setup with high-level camera accessors.
//!
//! This example demonstrates the public async runtime path without reaching into
//! runtime internals. It opens one camera session and performs several read-only
//! inquiries concurrently.
//!
//! Run with:
//! ```sh
//! cargo run --example runtime_demo --features runtime-tokio -- 192.168.0.110
//! ```

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    runtime::TokioRuntime,
    Error,
};

#[tokio::main]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt::try_init();

    let address = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110".to_string());

    println!("Tokio runtime example");
    println!("Address: {address}");

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&address, runtime).await?;

    let power = camera.power();
    let pan_tilt = camera.pan_tilt();
    let zoom = camera.zoom();
    let focus = camera.focus();

    let (power, pan_tilt, zoom, focus) = tokio::join!(
        power.state(),
        pan_tilt.position(),
        zoom.position(),
        focus.position()
    );

    match power {
        Ok(is_on) => println!("Power: {}", if is_on { "on" } else { "off" }),
        Err(error) => println!("Power inquiry failed: {error}"),
    }

    match pan_tilt {
        Ok(position) => println!("Pan/tilt: pan={}, tilt={}", position.pan, position.tilt),
        Err(error) => println!("Pan/tilt inquiry failed: {error}"),
    }

    match zoom {
        Ok(position) => println!("Zoom: 0x{:04X}", position.value()),
        Err(error) => println!("Zoom inquiry failed: {error}"),
    }

    match focus {
        Ok(position) => println!("Focus: {position:?}"),
        Err(error) => println!("Focus inquiry failed: {error}"),
    }

    camera.close().await?;
    Ok(())
}
