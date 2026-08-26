//! One owner-backed blocking preset operation.

use std::{env, io};

use grafton_visca::{blocking::Connect, profiles::PtzOpticsG2, Error, PresetNumber};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (address, action, number) = arguments()?;
    let preset = PresetNumber::new(number)?;
    let session = Connect::open_tcp::<PtzOpticsG2>(&address)?;
    let result = {
        let camera = session.camera::<PtzOpticsG2>()?;
        match action.as_str() {
            "set" => camera.presets().set(preset),
            "clear" | "reset" => camera.presets().reset(preset),
            "recall" => camera.presets().recall(preset)?.settled(),
            _ => Err(Error::InvalidParameter {
                parameter: "operation",
                value: action.into(),
                reason: "expected set, recall, or clear".into(),
            }),
        }
    };
    finish(result, session.close())?;
    Ok(())
}

fn arguments() -> Result<(String, String, u8), io::Error> {
    let mut values = env::args().skip(1);
    let address = values
        .next()
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110".to_string());
    let action = values.next().unwrap_or_else(|| "recall".to_string());
    let number = values
        .next()
        .unwrap_or_else(|| "1".to_string())
        .parse()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "preset must be 1..=255"))?;
    if values.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: preset_demo [address] [set|recall|clear] [number]",
        ));
    }
    Ok((address, action, number))
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
