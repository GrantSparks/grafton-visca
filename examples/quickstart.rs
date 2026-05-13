//! Blocking quickstart for the high-level camera API.
//!
//! By default this example is read-only: it connects, queries a few pieces of
//! state, and exits. Pass `--move` to run a short zoom movement and then stop.
//!
//! Run with:
//! ```sh
//! cargo run --example quickstart -- 192.168.0.110
//! cargo run --example quickstart -- 192.168.0.110 --move
//! ```

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, thread::sleep, time::Duration};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Connect},
        Error,
    };

    #[derive(Debug)]
    struct Args {
        address: String,
        move_camera: bool,
    }

    impl Args {
        fn parse() -> Self {
            let mut address = None;
            let mut move_camera = false;

            for arg in env::args().skip(1) {
                match arg.as_str() {
                    "--move" => move_camera = true,
                    _ if address.is_none() => address = Some(arg),
                    _ => {}
                }
            }

            Self {
                address: address
                    .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
                    .unwrap_or_else(|| "192.168.0.110".to_string()),
                move_camera,
            }
        }
    }

    fn power_label(is_on: bool) -> &'static str {
        if is_on {
            "on"
        } else {
            "off"
        }
    }

    pub fn main() -> Result<(), Error> {
        let _ = tracing_subscriber::fmt::try_init();

        let args = Args::parse();
        println!("Blocking quickstart");
        println!("Address: {}", args.address);

        let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&args.address)?;

        let power = camera.power().state()?;
        println!("Power: {}", power_label(power));

        match camera.zoom().position() {
            Ok(position) => println!("Zoom position: 0x{:04X}", position.value()),
            Err(error) => println!("Zoom position inquiry failed: {error}"),
        }

        if args.move_camera {
            println!("Running short zoom movement.");
            camera.zoom().tele()?;
            sleep(Duration::from_millis(250));
            camera.zoom().stop()?;
            camera.await_zoom_idle(Duration::from_secs(2))?;
            println!("Zoom stopped.");
        } else {
            println!("No movement requested. Pass --move to run a short zoom command.");
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
    println!("This example requires blocking mode. Run without async features:");
    println!("  cargo run --example quickstart -- 192.168.0.110");
}
