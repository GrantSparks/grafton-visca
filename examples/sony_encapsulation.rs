//! Sony encapsulated VISCA protocol example.
//!
//! Sony professional profiles use an 8-byte encapsulation header with sequence
//! numbers. The profile selects that protocol automatically; application code
//! uses the same accessors as raw VISCA profiles.
//!
//! Run with:
//! ```sh
//! cargo run --example sony_encapsulation --features runtime-tokio -- 192.168.0.110
//! ```

use grafton_visca::{
    camera::{profiles::SonyFR7, Connect},
    capabilities::ProfileMetadata,
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

    println!("Sony encapsulation example");
    println!("Profile: {}", SonyFR7::MODEL_NAME);
    println!("Address: {address}");

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<SonyFR7, _>(&address, runtime).await?;

    match camera.power().state().await {
        Ok(is_on) => println!("Power: {}", if is_on { "on" } else { "off" }),
        Err(error) => println!("Power inquiry failed: {error}"),
    }

    match camera.pan_tilt().position().await {
        Ok(position) => println!("Pan/tilt: pan={}, tilt={}", position.pan, position.tilt),
        Err(error) => println!("Pan/tilt inquiry failed: {error}"),
    }

    match camera.zoom().position().await {
        Ok(position) => println!("Zoom: 0x{:04X}", position.value()),
        Err(error) => println!("Zoom inquiry failed: {error}"),
    }

    camera.close().await?;
    Ok(())
}
