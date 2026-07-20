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

mod support;

use std::{env, io};

use grafton_visca::{
    camera::{profiles::SonyFR7, Connect},
    capabilities::ProfileMetadata,
    runtime::TokioRuntime,
    Error,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt::try_init();

    let address = camera_address()?;

    println!("Sony encapsulation example");
    println!("Profile: {}", SonyFR7::MODEL_NAME);
    println!("Address: {address}");

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_udp_async::<SonyFR7, _>(&address, runtime).await?;

    let inquiry_result: Result<(), Error> = async {
        let is_on = camera.power().state().await?;
        let pan_tilt = camera.pan_tilt().position().await?;
        let zoom = camera.zoom().position().await?;

        println!("Power: {}", if is_on { "on" } else { "off" });
        println!("Pan/tilt: pan={}, tilt={}", pan_tilt.pan, pan_tilt.tilt);
        println!("Zoom: 0x{:04X}", zoom.value());
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
                "unexpected extra argument `{extra}`\nusage: cargo run --example sony_encapsulation --features runtime-tokio -- [address]"
            ),
        ));
    }

    Ok(address)
}
