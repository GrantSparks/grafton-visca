//! Typed inquiry responses using high-level API methods.
//!
//! This example shows how inquiry methods return typed responses and avoids
//! manual response parsing.
//!
//! Run with:
//! ```bash
//! cargo run --example typed_inquiry_demo -- 192.168.0.110
//! ```

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use grafton_visca::{camera::Connect, profiles::GenericVisca, Error};

    pub fn main() -> Result<(), Error> {
        let _ = tracing_subscriber::fmt::try_init();

        let address = std::env::args()
            .nth(1)
            .or_else(|| std::env::var("VISCA_CAMERA_ADDR").ok())
            .unwrap_or_else(|| "192.168.0.110".to_string());
        println!("Connecting to camera at {address}...");

        let camera = Connect::open_tcp_blocking::<GenericVisca>(&address)?;

        println!("\nHigh-level API inquiry demo\n");

        match camera.power().state() {
            Ok(power_on) => {
                let status = if power_on { "on" } else { "off" };
                println!("Power: {status}");
            }
            Err(error) => println!("Power inquiry failed: {error}"),
        }

        match camera.zoom().position() {
            Ok(zoom_pos) => {
                let raw_value = zoom_pos.value();

                use grafton_visca::{inquiry_conversions::ZoomDomain, ZoomPositionExt};

                let optical_max = 0x4000u16;
                let digital_max = Some(0x7000u16);
                let optical =
                    zoom_pos.normalize_with_max(ZoomDomain::Optical, optical_max, digital_max);
                let full = zoom_pos.normalize_with_max(
                    ZoomDomain::OpticalPlusDigital,
                    optical_max,
                    digital_max,
                );

                println!("Zoom: 0x{raw_value:04X}");
                println!("  Optical zoom: {:.1}%", optical.0 * 100.0);
                println!("  Full range: {:.1}%", full.0 * 100.0);
            }
            Err(error) => println!("Zoom inquiry failed: {error}"),
        }

        camera.close()?;
        Ok(())
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() -> grafton_visca::Result<()> {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    eprintln!("This blocking example requires no async runtime feature.");
    eprintln!("Run with: cargo run --example typed_inquiry_demo -- 192.168.0.110");
}
