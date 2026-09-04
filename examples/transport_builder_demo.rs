//! Configured blocking camera connection.
//!
//! `CameraConfig` is pure reusable data. It applies the profile's default
//! port, validates transport compatibility, and then opens the owner-backed
//! blocking `CameraSession`.

use std::{env, time::Duration};

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraConfig},
    transport::{TcpKeepaliveConfig, TransportConfig},
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
    .transport_config(transport_config(kind));

    let session = config.open()?;
    let camera = session.camera();
    let state = camera.power().state()?;
    println!("Power: {}", if state { "on" } else { "off" });
    session.close()?;
    Ok(())
}

fn transport_config(kind: TransportKind) -> TransportConfig {
    TransportConfig {
        connect_timeout: Duration::from_secs(3),
        read_timeout: Duration::from_secs(2),
        write_timeout: Duration::from_secs(2),
        tcp_keepalive: matches!(kind, TransportKind::Tcp)
            .then(|| TcpKeepaliveConfig::new(Duration::from_secs(30))),
        ..TransportConfig::default()
    }
}
