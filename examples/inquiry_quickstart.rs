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
mod blocking {
    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Connect},
        inquiry_conversions::{PanTiltPositionRaw, ZoomDomain},
        ZoomPositionExt,
    };

    pub fn main() -> grafton_visca::Result<()> {
        let _ = tracing_subscriber::fmt::try_init();

        let address = std::env::args()
            .nth(1)
            .or_else(|| std::env::var("VISCA_CAMERA_ADDR").ok())
            .unwrap_or_else(|| "192.168.0.110".to_string());

        println!("Inquiry quickstart");
        println!("Address: {address}");

        let camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&address)?;

        match camera.power().state() {
            Ok(is_on) => println!("Power: {}", if is_on { "on" } else { "off" }),
            Err(error) => println!("Power inquiry failed: {error}"),
        }

        match camera.pan_tilt().position() {
            Ok(position) => {
                let degrees = PanTiltPositionRaw::new(position.pan, position.tilt).as_degrees();
                println!(
                    "Pan/tilt: pan={:.1} deg, tilt={:.1} deg",
                    degrees.pan.0, degrees.tilt.0
                );
            }
            Err(error) => println!("Pan/tilt inquiry failed: {error}"),
        }

        match camera.zoom().position() {
            Ok(position) => {
                let optical = position.normalize_with_max(ZoomDomain::Optical, 0x4000, None);
                println!(
                    "Zoom: 0x{:04X} ({:.1}% optical)",
                    position.value(),
                    optical.0 * 100.0
                );
            }
            Err(error) => println!("Zoom inquiry failed: {error}"),
        }

        match camera.focus().mode() {
            Ok(mode) => println!("Focus mode: {mode:?}"),
            Err(error) => println!("Focus mode inquiry failed: {error}"),
        }

        match camera.exposure().mode() {
            Ok(mode) => println!("Exposure mode: {mode:?}"),
            Err(error) => println!("Exposure mode inquiry failed: {error}"),
        }

        match camera.white_balance().mode() {
            Ok(mode) => println!("White balance mode: {mode:?}"),
            Err(error) => println!("White balance inquiry failed: {error}"),
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
    eprintln!("Run with: cargo run --example inquiry_quickstart -- 192.168.0.110");
}
