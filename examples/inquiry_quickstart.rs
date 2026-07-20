//! Blocking inquiry quickstart.
//!
//! This example reads camera state through high-level accessors and typed
//! inquiry responses. It does not change camera state.
//!
//! Run with:
//! ```sh
//! cargo run --example inquiry_quickstart -- 192.168.0.110
//! ```

#[cfg(not(feature = "mode-async"))]
mod support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Connect},
        inquiry_conversions::{PanTiltPositionRaw, ZoomDomain},
        Error, ZoomPositionExt,
    };

    use super::support::finish_session;

    fn address() -> Result<String, io::Error> {
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
                    "unexpected extra argument `{extra}`\nusage: cargo run --example inquiry_quickstart -- [address]"
                ),
            ));
        }

        Ok(address)
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let _ = tracing_subscriber::fmt::try_init();

        let address = address()?;

        println!("Inquiry quickstart");
        println!("Address: {address}");

        let camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&address)?;
        let query_result = (|| -> Result<(), Error> {
            let capabilities = camera.capabilities();
            println!("Profile: {}", capabilities.model_name);

            let is_on = camera.power().state()?;
            println!("Power: {}", if is_on { "on" } else { "off" });

            let position = camera.pan_tilt().position()?;
            let degrees = PanTiltPositionRaw::new(position.pan, position.tilt).as_degrees();
            println!(
                "Pan/tilt: pan={:.1} deg, tilt={:.1} deg",
                degrees.pan.0, degrees.tilt.0
            );

            let position = camera.zoom().position()?;
            let optical_max = *capabilities.zoom_range_optical.end();
            let digital_max = capabilities
                .zoom_range_digital
                .as_ref()
                .map(|range| *range.end());
            let optical =
                position.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)?;
            println!(
                "Zoom: 0x{:04X} ({:.1}% optical)",
                position.value(),
                optical.value() * 100.0
            );

            let mode = camera.focus().mode()?;
            println!("Focus mode: {mode:?}");

            let mode = camera.exposure().mode()?;
            println!("Exposure mode: {mode:?}");

            let mode = camera.white_balance().mode()?;
            println!("White balance mode: {mode:?}");

            Ok(())
        })();
        let close_result = camera.close();

        finish_session(query_result, close_result)?;
        Ok(())
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    eprintln!("This blocking example requires no async runtime feature.");
    eprintln!("Run with: cargo run --example inquiry_quickstart -- 192.168.0.110");
}
