//! Typed inquiry responses using the blocking owner-backed API.

use std::env;

use grafton_visca::{blocking::CameraSession, profiles::PtzOpticsG2, Error};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "pass a camera address or set VISCA_CAMERA_ADDR",
            )
        })?;
    let session = CameraSession::<PtzOpticsG2>::connect_tcp(&address)?;
    let result = {
        let camera = session.camera();
        let power = camera.power().status()?;
        let zoom = camera.zoom().position()?;
        let mode = camera.exposure().mode()?;
        println!("Power: {power:?}");
        println!("Zoom: 0x{:04X}", zoom.value());
        println!("Exposure mode: {mode:?}");
        Ok::<(), Error>(())
    };
    finish(result, session.close())?;
    Ok(())
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
