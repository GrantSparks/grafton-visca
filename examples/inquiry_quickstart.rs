//! Blocking typed-inquiry quickstart.
//!
//! Every query goes through a borrowed camera view of one owner-backed
//! blocking session; the session owns transport and request routing.

use std::{env, io};

use grafton_visca::{blocking::Connect, profiles::PtzOpticsG2, Error};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = address()?;
    let session = Connect::open_tcp::<PtzOpticsG2>(&address)?;
    let result = {
        let camera = session.camera::<PtzOpticsG2>()?;
        let power = camera.power().state()?;
        let pan_tilt = camera.pan_tilt().position()?;
        let zoom = camera.zoom().position()?;
        let focus = camera.focus().position()?;
        println!("Power: {}", if power { "on" } else { "off" });
        println!("Pan/tilt: pan={}, tilt={}", pan_tilt.pan, pan_tilt.tilt);
        println!("Zoom: 0x{:04X}", zoom.value());
        println!("Focus: {focus:?}");
        Ok::<(), Error>(())
    };
    finish(result, session.close())?;
    Ok(())
}

fn address() -> Result<String, io::Error> {
    let mut values = env::args().skip(1);
    let address = values
        .next()
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110".to_string());
    if let Some(extra) = values.next() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unexpected extra argument `{extra}`"),
        ));
    }
    Ok(address)
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
