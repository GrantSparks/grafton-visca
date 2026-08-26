//! Compare standard blocking TCP and UDP construction.
//!
//! Both paths use the profile's default port when the address has no explicit
//! port. Set `VISCA_CAMERA_ADDR` or pass an address on the command line.

use std::{env, net::SocketAddr};

use grafton_visca::{
    blocking::Connect, camera::profiles::PtzOpticsG2, capabilities::Capabilities, Error,
};

#[derive(Clone, Copy)]
enum Selection {
    Tcp,
    Udp,
    Both,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (address, selection) = parse_args()?;
    println!(
        "Profile: {}",
        Capabilities::from_profile::<PtzOpticsG2>().model_name
    );

    if matches!(selection, Selection::Tcp | Selection::Both) {
        report("TCP", check_tcp(&address));
    }
    if matches!(selection, Selection::Udp | Selection::Both) {
        report("UDP", check_udp(&address));
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
            .unwrap_or_else(|| "192.168.0.110".to_owned()),
        selection,
    ))
}

fn check_tcp(address: &str) -> Result<(), Error> {
    let session = Connect::open_tcp::<PtzOpticsG2>(address)?;
    let camera = session.camera::<PtzOpticsG2>()?;
    let state = camera.power().state()?;
    session.close()?;
    println!("  connected; power {}", if state { "on" } else { "off" });
    Ok(())
}

fn check_udp(address: &str) -> Result<(), Error> {
    let session = Connect::open_udp::<PtzOpticsG2>(address)?;
    let camera = session.camera::<PtzOpticsG2>()?;
    let state = camera.power().state()?;
    session.close()?;
    println!("  connected; power {}", if state { "on" } else { "off" });
    Ok(())
}

fn report(label: &str, result: Result<(), Error>) {
    if let Err(error) = result {
        eprintln!("{label}: {error}");
    }
}

#[allow(dead_code)]
fn has_explicit_port(address: &str) -> bool {
    address.parse::<SocketAddr>().is_ok()
        || (address.matches(':').count() == 1
            && address
                .rsplit_once(':')
                .is_some_and(|(_, port)| port.parse::<u16>().is_ok()))
}
