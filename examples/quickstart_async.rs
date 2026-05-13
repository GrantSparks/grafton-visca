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

use std::env;

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    runtime::TokioRuntime,
    Error,
};
use tokio::time::{sleep, Duration};

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

#[tokio::main]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt::try_init();

    let args = Args::parse();
    println!("Async quickstart (Tokio)");
    println!("Address: {}", args.address);

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(&args.address, runtime).await?;

    let power = camera.power().state().await?;
    println!("Power: {}", power_label(power));

    match camera.zoom().position().await {
        Ok(position) => println!("Zoom position: 0x{:04X}", position.value()),
        Err(error) => println!("Zoom position inquiry failed: {error}"),
    }

    if args.move_camera {
        println!("Running short zoom movement.");
        camera.zoom().tele().await?;
        sleep(Duration::from_millis(250)).await;
        camera.zoom().stop().await?;
        camera.await_zoom_idle(Duration::from_secs(2)).await?;
        println!("Zoom stopped.");
    } else {
        println!("No movement requested. Pass --move to run a short zoom command.");
    }

    camera.close().await?;
    Ok(())
}
