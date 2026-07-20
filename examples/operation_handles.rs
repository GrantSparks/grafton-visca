//! Blocking operation handles with exact per-command completion semantics.
//!
//! This example moves real hardware. It returns the camera home, briefly starts
//! zooming, and then sends a bounded stop command.
//!
//! Run with:
//! ```sh
//! cargo run --example operation_handles -- 192.168.0.110
//! ```

#[cfg(not(feature = "mode-async"))]
mod support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io, thread::sleep, time::Duration};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Connect},
        command::{PanTilt, Zoom},
        Error,
    };

    use super::support::finish_session;

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let address = camera_address()?;

        let camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&address)?;

        let operation_result: Result<(), Error> = (|| {
            // A targeted command has a meaningful physical-settle state. One
            // deadline covers exact protocol completion and any profile fallback.
            camera
                .submit(&PanTilt::Home)?
                .await_settled(Duration::from_secs(20))?;

            // Detach is explicit fire-and-forget. It does not cancel or stop the
            // command, so this example always follows it with a bounded STOP.
            camera.submit(&Zoom::TeleStd)?.detach();
            sleep(Duration::from_millis(250));
            camera
                .submit(&Zoom::Stop)?
                .await_applied(Duration::from_secs(2))?;
            Ok(())
        })();

        let close_result = camera.close();
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
                    "unexpected extra argument `{extra}`\nusage: cargo run --example operation_handles -- [address]"
                ),
            ));
        }

        Ok(address)
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    eprintln!("This example uses the blocking API. Run without async features:");
    eprintln!("  cargo run --example operation_handles -- 192.168.0.110");
}
