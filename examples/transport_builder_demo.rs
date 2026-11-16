//! Configured blocking camera connection.
//!
//! `CameraConfig` is pure reusable data. It applies the profile's default
//! port, validates transport compatibility, and then opens the owner-backed
//! blocking `CameraSession`.

use std::{env, time::Duration};

use grafton_visca::{
    blocking::CameraSession, config::CameraConfig, profiles::PtzOpticsG2,
    transport::TcpKeepaliveConfig, transport::TransportConfig,
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
        .unwrap_or_else(|| "192.168.0.110".to_owned());
    let kind = if env::args().any(|arg| arg == "--udp") {
        TransportKind::Udp
    } else {
        TransportKind::Tcp
    };

    let config = match kind {
        TransportKind::Tcp => CameraConfig::<PtzOpticsG2>::tcp(&address),
        TransportKind::Udp => CameraConfig::<PtzOpticsG2>::udp(&address),
    }
    .with_transport_config(transport_config(kind));

    let session = CameraSession::connect(&config)?;
    let camera = session.camera();
    let state = camera.power().status()?;
    println!("Power: {state:?}");
    session.close()?;
    Ok(())
}

/// Start from the transport's own defaults and adjust only the timeouts and
/// keepalive.
fn transport_config(kind: TransportKind) -> TransportConfig {
    let config = match kind {
        TransportKind::Tcp => TransportConfig::for_tcp(),
        TransportKind::Udp => TransportConfig::for_udp(),
    };
    config
        .with_connect_timeout(Duration::from_secs(3))
        .with_read_timeout(Duration::from_secs(2))
        .with_write_timeout(Duration::from_secs(2))
        .with_tcp_keepalive(
            matches!(kind, TransportKind::Tcp)
                .then(|| TcpKeepaliveConfig::new(Duration::from_secs(30))),
        )
}
