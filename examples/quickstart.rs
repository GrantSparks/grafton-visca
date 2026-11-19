//! Read power and zoom from a PTZOptics G2 over TCP.
//! Pass `--move` to drive the zoom briefly and then stop it.

use std::{env, io, thread::sleep, time::Duration};

use grafton_visca::{
    blocking::Camera, blocking::CameraSession, profiles::PtzOpticsG2, types::AffectedAxes,
    types::IdleWait, Error,
};

/// Attempts a zoom STOP on scope exit. Dropping a drive handle only detaches it.
struct StopZoomOnExit<'a> {
    camera: &'a Camera<PtzOpticsG2>,
}

impl Drop for StopZoomOnExit<'_> {
    fn drop(&mut self) {
        // Drop cannot return a STOP error.
        if let Ok(mut stop) = self.camera.zoom().stop() {
            let _ = stop.applied();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (address, move_camera) = arguments()?;
    let session = CameraSession::<PtzOpticsG2>::connect_tcp(&address)?;

    let result = {
        let camera = session.camera();
        let power = camera.power().status()?;
        let zoom = camera.zoom().position()?;
        println!("Power: {power:?}");
        println!("Zoom position: 0x{:04X}", zoom.value());

        if move_camera {
            let _stop_on_exit = StopZoomOnExit { camera };
            let mut drive = camera.zoom().tele()?;
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
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "pass a camera address or set VISCA_CAMERA_ADDR",
                )
            })?,
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
