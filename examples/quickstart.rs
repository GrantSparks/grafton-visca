//! Blocking quickstart for the owner-backed camera API.
//!
//! The default path only performs inquiries. Pass `--move` to run a short
//! zoom command followed by an applied STOP. The movement block holds a
//! stop-on-exit guard, so an early `?` still ends the motion: dropping an
//! operation handle is `detach` and never stops hardware.

use std::{env, io, thread::sleep, time::Duration};

use grafton_visca::{
    blocking::{Camera, Connect},
    camera::{profiles::PtzOpticsG2, IdleWait},
    AffectedAxes, Error,
};

/// Sends the typed zoom STOP when it leaves scope, on every exit path.
///
/// The library never stops hardware on drop, so the `?` inside the movement
/// block would otherwise leave the zoom driving. This is the same caller-owned
/// pattern `examples/operation_handles.rs` documents in full.
struct StopZoomOnExit<'a, 'session> {
    camera: &'a Camera<'session, PtzOpticsG2>,
}

impl Drop for StopZoomOnExit<'_, '_> {
    fn drop(&mut self) {
        // `Drop` cannot report a failure and may run while unwinding, so the
        // stop is best effort.
        if let Ok(stop) = self.camera.zoom().stop() {
            let _ = stop.applied();
        }
    }
}

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
            // The guard bounds the zoom to this block: an early `?` below
            // still stops the camera before the session closes.
            let _stop_on_exit = StopZoomOnExit { camera: &camera };
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
