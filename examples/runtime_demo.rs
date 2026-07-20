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

mod support;

use std::{env, io};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    runtime::TokioRuntime,
    Error,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt::try_init();

    let address = camera_address()?;

    println!("Tokio runtime example");
    println!("Address: {address}");

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&address, runtime).await?;

    let inquiry_result: Result<(), Error> = async {
        let power = camera.power();
        let pan_tilt = camera.pan_tilt();
        let zoom = camera.zoom();
        let focus = camera.focus();

        let (is_on, pan_tilt, zoom, focus) = tokio::try_join!(
            power.state(),
            pan_tilt.position(),
            zoom.position(),
            focus.position()
        )?;

        println!("Power: {}", if is_on { "on" } else { "off" });
        println!("Pan/tilt: pan={}, tilt={}", pan_tilt.pan, pan_tilt.tilt);
        println!("Zoom: 0x{:04X}", zoom.value());
        println!("Focus: {focus:?}");
        Ok(())
    }
    .await;

    let close_result = camera.close().await;
    finish_session(inquiry_result, close_result)?;
    Ok(())
}

fn camera_address() -> Result<String, io::Error> {
    let mut values = env::args().skip(1);
    let address = match values.next() {
        Some(value) if value.starts_with('-') => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown option `{value}`"),
            ));
        }
        Some(value) => value,
        None => env::var("VISCA_CAMERA_ADDR").unwrap_or_else(|_| "192.168.0.110".to_string()),
    };

    if let Some(extra) = values.next() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unexpected extra argument `{extra}`\nusage: cargo run --example runtime_demo --features runtime-tokio -- [address]"
            ),
        ));
    }

    Ok(address)
}
