//! Attach a caller-owned blocking transport with `Session::open`.
//!
//! Use `CameraSession::connect_udp` for the shortest standard path. `Session::open` is the
//! canonical escape hatch when application code constructs and owns a
//! configured transport itself.

use std::{env, io, net::UdpSocket, time::Duration};

use grafton_visca::{
    blocking::Session,
    config::SessionConfig,
    profiles::PtzOpticsG2,
    transport::{
        BlockingTransport, HasTransportConfig, ReceiveOutcome, TransportConfig, TransportIdentity,
    },
    Error,
};

/// A caller-owned datagram adapter using the standard library's socket timeouts.
#[derive(Debug)]
struct CallerUdp {
    socket: UdpSocket,
    config: grafton_visca::transport::ValidatedTransportConfig,
}

impl CallerUdp {
    fn connect(address: &str) -> Result<Self, Error> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(address)?;
        Ok(Self {
            socket,
            config: TransportConfig::for_udp().validated()?,
        })
    }
}

impl HasTransportConfig for CallerUdp {
    fn identity(&self) -> TransportIdentity<'_> {
        TransportIdentity::Udp
    }

    fn transport_config(&self) -> &grafton_visca::transport::ValidatedTransportConfig {
        &self.config
    }
}

impl BlockingTransport for CallerUdp {
    fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error> {
        self.socket.set_write_timeout(Some(timeout))?;
        let sent = self.socket.send(bytes)?;
        if sent != bytes.len() {
            return Err(
                io::Error::new(io::ErrorKind::WriteZero, "incomplete datagram write").into(),
            );
        }
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        destination: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        self.socket.set_read_timeout(Some(timeout))?;
        let received = self.socket.recv(destination)?;
        // The standard socket API does not report whether an exactly full
        // buffer lost a suffix, so preserve that uncertainty for the owner.
        Ok(if received == destination.len() {
            ReceiveOutcome::possibly_truncated(received)
        } else {
            ReceiveOutcome::complete(received)
        })
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let address = env::args()
        .nth(1)
        .or_else(|| env::var("VISCA_CAMERA_ADDR").ok())
        .unwrap_or_else(|| "192.168.0.110:1259".to_owned());

    let transport = CallerUdp::connect(&address)?;
    let config = SessionConfig::from_compile_time::<PtzOpticsG2>()?;
    let session = Session::open(transport, &config)?;
    let camera =
        session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)?;
    println!("Power: {:?}", camera.power().status()?);
    session.close()?;
    Ok(())
}
