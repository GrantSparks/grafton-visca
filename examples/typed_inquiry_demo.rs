//! Typed inquiry responses using high-level API methods.
//!
//! This example shows how inquiry methods return typed responses and avoids
//! manual response parsing. It uses the conservative `GenericVisca` profile;
//! applications should select a model-specific profile when one matches their
//! hardware.
//!
//! Run with:
//! ```bash
//! cargo run --example typed_inquiry_demo -- 192.168.0.110
//! ```

#[cfg(not(feature = "mode-async"))]
mod support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io};

    use grafton_visca::{
        camera::Connect, inquiry_conversions::ZoomDomain, profiles::GenericVisca, Error,
        ZoomPositionExt,
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
                    "unexpected extra argument `{extra}`\nusage: cargo run --example typed_inquiry_demo -- [address]"
                ),
            ));
        }

        Ok(address)
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let _ = tracing_subscriber::fmt::try_init();

        let address = address()?;
        println!("Connecting to camera at {address}...");

        let camera = Connect::open_tcp_blocking::<GenericVisca>(&address)?;
        let query_result = (|| -> Result<(), Error> {
            let capabilities = camera.capabilities();
            println!("Profile metadata: {}", capabilities.model_name);
            println!("Inquiry support: {:?}", capabilities.inquiry_support);
            println!("\nHigh-level API inquiry demo\n");

            let power_on = camera.power().state()?;
            let status = if power_on { "on" } else { "off" };
            println!("Power: {status}");

            let zoom_pos = camera.zoom().position()?;
            let raw_value = zoom_pos.value();
            let optical_max = *capabilities.zoom_range_optical.end();
            let digital_max = capabilities
                .zoom_range_digital
                .as_ref()
                .map(|range| *range.end());
            let optical =
                zoom_pos.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max)?;

            println!("Zoom: 0x{raw_value:04X}");
            println!("  Optical range: {:.1}%", optical.value() * 100.0);

            if digital_max.is_some() {
                let combined = zoom_pos.normalize_with_max(
                    ZoomDomain::OpticalPlusDigital,
                    optical_max,
                    digital_max,
                )?;
                println!(
                    "  Optical + digital range: {:.1}%",
                    combined.value() * 100.0
                );
            } else {
                println!("  Digital zoom range: not declared by this profile");
            }

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
    eprintln!("Run with: cargo run --example typed_inquiry_demo -- 192.168.0.110");
}
