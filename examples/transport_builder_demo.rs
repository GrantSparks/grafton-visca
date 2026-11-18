//! Configured blocking camera connection.
//!
//! `CameraConfig` is pure reusable data. It applies the profile's default
//! port, validates transport compatibility, and then opens the owner-backed
//! blocking `CameraSession`.

use std::{env, time::Duration};

use grafton_visca::{
    blocking::CameraSession,
    config::CameraConfig,
    profiles::PtzOpticsG2,
    transport::{BufferConfig, IoBounds, TcpConnect, TcpKeepaliveConfig, UdpConnect},
};

#[derive(Clone, Copy)]
enum TransportKind {
    Tcp,
    Udp,
}

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
    let kind = if env::args().any(|arg| arg == "--udp") {
        TransportKind::Udp
    } else {
        TransportKind::Tcp
    };

    let budget = Duration::from_secs(3);
    let io_timeout = Duration::from_secs(2);
    let config = match kind {
        TransportKind::Tcp => CameraConfig::<PtzOpticsG2>::tcp_configured(
            &address,
            TcpConnect::default()
                .with_connect_timeout(budget)?
                .with_keepalive(Some(TcpKeepaliveConfig::new(Duration::from_secs(30))))?,
            IoBounds::new(io_timeout, io_timeout, BufferConfig::for_tcp())?,
        ),
        TransportKind::Udp => CameraConfig::<PtzOpticsG2>::udp_configured(
            &address,
            UdpConnect::default().with_connect_timeout(budget)?,
            IoBounds::new(io_timeout, io_timeout, BufferConfig::for_udp())?,
        ),
    };

    let session = CameraSession::connect(&config)?;
    let camera = session.camera();
    let state = camera.power().status()?;
    println!("Power: {state:?}");
    session.close()?;
    Ok(())
}
