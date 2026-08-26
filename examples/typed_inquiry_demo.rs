//! Typed inquiry responses using the blocking owner-backed API.

use std::env;

use grafton_visca::{blocking::Connect, profiles::GenericVisca, Error};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110".to_string());
    let session = Connect::open_tcp::<GenericVisca>(&address)?;
    let result = {
        let camera = session.camera::<GenericVisca>()?;
        let power = camera.power().state()?;
        let zoom = camera.zoom().position()?;
        let mode = camera.exposure().mode()?;
        println!("Power: {}", if power { "on" } else { "off" });
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
