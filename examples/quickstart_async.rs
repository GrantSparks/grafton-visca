//! Tokio quickstart for the high-level camera API.
//!
//! By default this example is read-only. Pass `--move` to run a short zoom
//! movement and then stop.
//!
//! Run with:
//! ```sh
//! cargo run --example quickstart_async --features runtime-tokio -- 192.168.0.110
//! cargo run --example quickstart_async --features runtime-tokio -- 192.168.0.110 --move
//! ```

mod support;

use std::{env, io};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, IdleWait},
    runtime::TokioRuntime,
    AffectedAxes, Connect, Error,
};
use tokio::time::{sleep, Duration};

use support::finish_session;

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
    "usage: cargo run --example quickstart_async --features runtime-tokio -- [address] [--move]"
}

fn power_label(is_on: bool) -> &'static str {
    if is_on {
        "on"
    } else {
        "off"
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt::try_init();

    let args = Args::parse()?;
    println!("Async quickstart (Tokio)");
    println!("Address: {}", args.address);

    let runtime = TokioRuntime::from_current()?;
    let session = Connect::open_tcp::<PtzOpticsG2, _>(&args.address, runtime).await?;
    let camera = session.camera::<PtzOpticsG2>()?;

    let run_result = async {
        let power = camera.power().state().await?;
        println!("Power: {}", power_label(power));

        let position = camera.zoom().position().await?;
        println!("Zoom position: 0x{:04X}", position.value());

        if args.move_camera {
            println!("Running short zoom movement.");
            let start_result = match camera.zoom().tele().await {
                Ok(operation) => operation.applied().await,
                Err(error) => Err(error),
            };
            if start_result.is_ok() {
                sleep(Duration::from_millis(250)).await;
            }

            // Once movement starts, always attempt STOP before propagating
            // its result or waiting for the axis to become idle.
            let stop_result = match camera.zoom().stop().await {
                Ok(operation) => operation.applied().await,
                Err(error) => Err(error),
            };
            if start_result.is_err() {
                if let Err(error) = &stop_result {
                    eprintln!("The safety STOP also failed: {error}");
                }
            }
            start_result?;
            stop_result?;
            camera
                .motion()
                .wait_until_idle(IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(2)))
                .await?;
            println!("Zoom stopped.");
        } else {
            println!("No movement requested. Pass --move to run a short zoom command.");
        }

        Ok::<(), Error>(())
    }
    .await;
    let close_result = session.close().await;

    finish_session(run_result, close_result)?;
    Ok(())
}
