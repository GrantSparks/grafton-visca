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
mod support;

#[cfg(not(feature = "mode-async"))]
mod blocking {
    use std::{env, io, thread::sleep, time::Duration};

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Connect},
        Error,
    };

    use super::support::finish_session;

    #[derive(Debug)]
    struct Args {
        address: String,
        move_camera: bool,
    }

    impl Args {
        fn parse() -> Result<Self, io::Error> {
            let mut address = None;
            let mut move_camera = false;

            for arg in env::args().skip(1) {
                match arg.as_str() {
                    "--move" => move_camera = true,
                    "-h" | "--help" => return Err(io::Error::other(usage())),
                    _ if arg.starts_with('-') => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("unknown option `{arg}`\n{}", usage()),
                        ));
                    }
                    _ if address.is_none() => address = Some(arg),
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!("unexpected extra argument `{arg}`\n{}", usage()),
                        ));
                    }
                }
            }

            Ok(Self {
                address: address
                    .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
                    .unwrap_or_else(|| "192.168.0.110".to_string()),
                move_camera,
            })
        }
    }

    fn usage() -> &'static str {
        "usage: cargo run --example quickstart -- [address] [--move]"
    }

    fn power_label(is_on: bool) -> &'static str {
        if is_on {
            "on"
        } else {
            "off"
        }
    }

    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        let _ = tracing_subscriber::fmt::try_init();

        let args = Args::parse()?;
        println!("Blocking quickstart");
        println!("Address: {}", args.address);

        let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>(&args.address)?;
        let run_result = (|| -> Result<(), Error> {
            let power = camera.power().state()?;
            println!("Power: {}", power_label(power));

            let position = camera.zoom().position()?;
            println!("Zoom position: 0x{:04X}", position.value());

            if args.move_camera {
                println!("Running short zoom movement.");
                let start_result = camera.zoom().tele();
                if start_result.is_ok() {
                    sleep(Duration::from_millis(250));
                }

                // Once movement starts, always attempt STOP before propagating
                // its result or waiting for the axis to become idle.
                let stop_result = camera.zoom().stop();
                if start_result.is_err() {
                    if let Err(error) = &stop_result {
                        eprintln!("The safety STOP also failed: {error}");
                    }
                }
                start_result?;
                stop_result?;
                camera.await_zoom_idle(Duration::from_secs(2))?;
                println!("Zoom stopped.");
            } else {
                println!("No movement requested. Pass --move to run a short zoom command.");
            }

            Ok(())
        })();
        let close_result = camera.close();

        finish_session(run_result, close_result)?;
        Ok(())
    }
}

#[cfg(not(feature = "mode-async"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    blocking::main()
}

#[cfg(feature = "mode-async")]
fn main() {
    println!("This example requires blocking mode. Run without async features:");
    println!("  cargo run --example quickstart -- 192.168.0.110");
}
