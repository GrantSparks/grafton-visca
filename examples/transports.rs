//! Compare standard blocking TCP and UDP construction.
//!
//! Both paths use the profile's default port when the address has no explicit
//! port. Set `VISCA_CAMERA_ADDR` or pass an address on the command line.

use std::env;

use grafton_visca::{
    blocking::CameraSession, profiles::CompileTimeProfile, profiles::PtzOpticsG2, Error,
};

#[derive(Clone, Copy)]
enum Selection {
    Tcp,
    Udp,
    Both,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (address, selection) = parse_args()?;
    println!("Profile: {}", PtzOpticsG2::spec()?.name());

    let mut first_error = None;
    if matches!(selection, Selection::Tcp | Selection::Both) {
        report("TCP", check_tcp(&address), &mut first_error);
    }
    if matches!(selection, Selection::Udp | Selection::Both) {
        report("UDP", check_udp(&address), &mut first_error);
    }
    if let Some(error) = first_error {
        return Err(error.into());
    }
    Ok(())
}

fn parse_args() -> Result<(String, Selection), Box<dyn std::error::Error>> {
    let mut address = None;
    let mut selection = Selection::Both;
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--tcp-only" => selection = Selection::Tcp,
            "--udp-only" => selection = Selection::Udp,
            value if value.starts_with('-') => {
                return Err(format!("unknown option: {value}").into())
            }
            _ if address.is_none() => address = Some(arg),
            value => return Err(format!("unexpected argument: {value}").into()),
        }
    }
    Ok((
        address
            .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "pass a camera address or set VISCA_CAMERA_ADDR",
                )
            })?,
        selection,
    ))
}

fn check_tcp(address: &str) -> Result<(), Error> {
    let session = CameraSession::<PtzOpticsG2>::connect_tcp(address)?;
    let camera = session.camera();
    let state = camera.power().status()?;
    session.close()?;
    println!("  connected; power {state:?}");
    Ok(())
}

fn check_udp(address: &str) -> Result<(), Error> {
    let session = CameraSession::<PtzOpticsG2>::connect_udp(address)?;
    let camera = session.camera();
    let state = camera.power().status()?;
    session.close()?;
    println!("  connected; power {state:?}");
    Ok(())
}

fn report(label: &str, result: Result<(), Error>, first_error: &mut Option<Error>) {
    if let Err(error) = result {
        eprintln!("{label}: {error}");
        if first_error.is_none() {
            *first_error = Some(error);
        }
    }
}
