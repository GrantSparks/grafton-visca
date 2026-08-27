//! Read-only Tokio serial VISCA example.
//!
//! The example builds a [`CameraConfig`](grafton_visca::camera::CameraConfig),
//! validates the requested VISCA camera ID before opening the device, performs
//! two inquiries, and explicitly closes the session on both inquiry success and
//! failure.
//!
//! Run with the example's exact required features:
//! ```sh
//! cargo run --example serial_async_demo \
//!   --features runtime-tokio,transport-serial-tokio -- [port] [camera_id]
//! ```
//!
//! The defaults are `/dev/ttyUSB0` at 9600 baud and camera ID 1.

use std::env;

use grafton_visca::{
    camera::{profiles::GenericVisca, CameraConfig},
    runtime::TokioRuntime,
    Error,
};

#[derive(Debug)]
struct Args {
    port: String,
    camera_id: u8,
}

impl Args {
    fn parse() -> Result<Self, Error> {
        let mut args = env::args().skip(1);
        let port = match args.next() {
            Some(value) if value.starts_with('-') => {
                return Err(Error::InvalidParameter {
                    parameter: "arguments",
                    value: value.into(),
                    reason: "unknown option; expected [port] [camera_id]".into(),
                });
            }
            Some(value) => value,
            None => "/dev/ttyUSB0".to_string(),
        };
        let camera_id = match args.next() {
            Some(raw) => raw.parse::<u8>().map_err(|_| Error::InvalidParameter {
                parameter: "camera_id",
                value: raw.into(),
                reason: "expected a numeric VISCA camera ID in 1..=8".into(),
            })?,
            None => 1,
        };

        if let Some(extra) = args.next() {
            return Err(Error::InvalidParameter {
                parameter: "arguments",
                value: extra.into(),
                reason: "unexpected extra argument; expected [port] [camera_id]".into(),
            });
        }

        Ok(Self { port, camera_id })
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let _ = tracing_subscriber::fmt::try_init();
    let args = Args::parse()?;

    println!("Tokio serial VISCA example");
    println!("Port: {}", args.port);
    println!("Baud rate: 9600");
    println!("Camera ID: {}", args.camera_id);

    let config = CameraConfig::<GenericVisca>::serial(args.port.clone(), 9600)
        .try_camera_id(args.camera_id)?;
    let runtime = TokioRuntime::from_current()?;
    // The single-camera serial constructor names `GenericVisca` once, on the
    // configuration: the camera view below is bound to it by construction.
    let session = config
        .open_serial_camera_async(runtime)
        .await
        .map_err(|error| {
            error.context(format!(
                "failed to open VISCA camera {} on serial port {}",
                args.camera_id, args.port
            ))
        })?;

    let camera = session.camera();
    println!("Serial camera session established; running read-only inquiries.");
    let inquiry_result = async {
        let version = camera
            .system()
            .version()
            .await
            .map_err(|error| error.with_context("serial version inquiry failed"))?;
        println!("Version: {version:?}");

        let power = camera
            .power()
            .state()
            .await
            .map_err(|error| error.with_context("serial power inquiry failed"))?;
        println!("Power: {}", if power { "on" } else { "standby" });

        Ok::<(), Error>(())
    }
    .await;

    // Close before propagating an inquiry error so the example demonstrates a
    // complete session lifecycle even when the camera rejects an inquiry.
    let close_result = session
        .close()
        .await
        .map(|_| ())
        .map_err(|error| error.with_context("failed to close serial camera session"));

    match (inquiry_result, close_result) {
        (Ok(()), Ok(())) => {
            println!("Read-only serial checks completed and the session was closed.");
            Ok(())
        }
        (Err(inquiry_error), Ok(())) => Err(inquiry_error),
        (Ok(()), Err(close_error)) => Err(close_error),
        (Err(inquiry_error), Err(close_error)) => {
            eprintln!("The session also failed to close cleanly: {close_error}");
            Err(inquiry_error)
        }
    }
}
