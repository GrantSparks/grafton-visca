//! Blocking quickstart for the owner-backed camera API.
//!
//! The default path only performs inquiries. Pass `--move` to run a short
//! zoom command followed by an applied STOP.

use std::{env, io, thread::sleep, time::Duration};

use grafton_visca::{
    blocking::Connect,
    camera::{profiles::PtzOpticsG2, IdleWait},
    AffectedAxes, Error,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (address, move_camera) = arguments()?;
    let session = Connect::open_tcp::<PtzOpticsG2>(&address)?;

    let result = {
        let camera = session.camera::<PtzOpticsG2>()?;
        let power = camera.power().state()?;
        let zoom = camera.zoom().position()?;
        println!("Power: {}", if power { "on" } else { "off" });
        println!("Zoom position: 0x{:04X}", zoom.value());

        if move_camera {
            let drive = camera.zoom().tele()?;
            sleep(Duration::from_millis(250));
            drive.applied()?;
            camera.zoom().stop()?.applied()?;
            camera
                .motion()
                .wait_until_idle(IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(2)))?;
        }
        Ok::<(), Error>(())
    };

    let close = session.close();
    finish(result, close)?;
    Ok(())
}

fn arguments() -> Result<(String, bool), io::Error> {
    let mut address = None;
    let mut move_camera = false;
    for argument in env::args().skip(1) {
        match argument.as_str() {
            "--move" => move_camera = true,
            "-h" | "--help" => {
                return Err(io::Error::other(
                    "usage: cargo run --example quickstart -- [address] [--move]",
                ));
            }
            _ if argument.starts_with('-') => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown option `{argument}`"),
                ));
            }
            _ if address.is_none() => address = Some(argument),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "only one address is accepted",
                ));
            }
        }
    }
    Ok((
        address
            .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
            .unwrap_or_else(|| "192.168.0.110".to_string()),
        move_camera,
    ))
}

fn finish<T>(operation: Result<T, Error>, close: Result<(), Error>) -> Result<T, Error> {
    match (operation, close) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
        (Err(operation), Err(close)) => {
            eprintln!("session close also failed: {close}");
            Err(operation)
        }
    }
}
