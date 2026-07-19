//! Tokio operation handles with exact per-command completion semantics.
//!
//! This example moves real hardware. It returns the camera home, briefly starts
//! zooming, and then sends a bounded stop command.
//!
//! Run with:
//! ```sh
//! cargo run --example operation_handles_async --features runtime-tokio -- 192.168.0.110
//! ```

mod support;

use std::{env, io, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    command::{PanTilt, Zoom},
    runtime::TokioRuntime,
    Error,
};

use support::finish_session;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = camera_address()?;

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&address, runtime).await?;

    let operation_result: Result<(), Error> = async {
        // A targeted command has a meaningful physical-settle state. One
        // deadline covers exact protocol completion and any profile fallback.
        camera
            .submit(&PanTilt::Home)
            .await?
            .await_settled(Duration::from_secs(20))
            .await?;

        // Async submission returns after scheduler acceptance. Detach leaves the
        // command scheduler-owned, so this example always follows it with a STOP.
        camera.submit(&Zoom::TeleStd).await?.detach();
        tokio::time::sleep(Duration::from_millis(250)).await;
        camera
            .submit(&Zoom::Stop)
            .await?
            .await_applied(Duration::from_secs(2))
            .await?;
        Ok(())
    }
    .await;

    let close_result = camera.close().await;
    finish_session(operation_result, close_result)?;
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
                "unexpected extra argument `{extra}`\nusage: cargo run --example operation_handles_async --features runtime-tokio -- [address]"
            ),
        ));
    }

    Ok(address)
}
