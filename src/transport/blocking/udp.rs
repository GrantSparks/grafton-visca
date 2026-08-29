//! Blocking UDP transport implementation with IPv6 support.

use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

use crate::{
    command::CommandKind,
    transport::{
        address::{canonicalize_endpoint, AddressResolver},
        buffer::BufferConfig,
        builder::{AddressingMode, TransportConfig},
        BlockingTransport, HasTransportConfig, SendSemantics,
    },
    Error,
};

/// UDP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
/// It operates at the datagram level, treating each datagram as a frame.
#[derive(Debug)]
pub struct Udp {
    socket: UdpSocket,
    config: TransportConfig,
}

impl Udp {
    /// Connect to a UDP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// The socket will bind to the appropriate unspecified address based on the
    /// target address family. The address must include an explicit port; explicit
    /// IPv6 ports require brackets.
    pub fn connect(address: &str) -> Result<Self, Error> {
        let config = TransportConfig {
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            buffer_config: BufferConfig::for_udp(),
            addressing: AddressingMode::Ip, // UDP is always IP mode
            ..Default::default()
        };
        Self::connect_with_config(address, config)
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    /// The address must include an explicit port.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        let canonical_addr = canonicalize_endpoint(address, None)?;

        // Use the common address resolver
        let resolver = AddressResolver::new();
        let target_addr = resolver.resolve_first(&canonical_addr)?;

        // Bind to the appropriate unspecified address based on target family
        let bind_addr = resolver.bind_address_for(&target_addr);

        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(target_addr)?;

        // Apply socket options from config
        socket.set_read_timeout(Some(config.read_timeout))?;
        socket.set_write_timeout(Some(config.write_timeout))?;
        if let Some(ttl) = config.ttl {
            socket.set_ttl(ttl)?;
        }

        Ok(Self { socket, config })
    }
}

impl HasTransportConfig for Udp {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Udp)
    }
}

impl BlockingTransport for Udp {
    fn send_with_kind(&mut self, data: &[u8], _kind: CommandKind) -> Result<(), Error> {
        // Send directly - retry logic is handled at the runtime/scheduler level for async
        // For blocking mode, the blocking runner will handle retries
        self.socket.send(data)?;
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        loop {
            match self.socket.recv(dst) {
                Ok(0) => continue,
                Ok(n) => return Ok(n),
                Err(e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    return Err(Error::Timeout);
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        duration: Duration,
    ) -> Result<usize, Error> {
        // Save the current timeout
        let original_timeout = self.socket.read_timeout()?;
        let deadline = Instant::now()
            .checked_add(duration)
            .unwrap_or_else(Instant::now);

        // Keep one deadline for the whole operation. Empty datagrams must not
        // give the caller a fresh full timeout on every receive attempt.
        let result = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                break Err(Error::Timeout);
            }

            if let Err(error) = self.socket.set_read_timeout(Some(remaining)) {
                break Err(error.into());
            }

            match self.socket.recv(dst) {
                Ok(0) => continue,
                Ok(n) => break Ok(n),
                Err(e)
                    if e.kind() == std::io::ErrorKind::TimedOut
                        || e.kind() == std::io::ErrorKind::WouldBlock =>
                {
                    break Err(Error::Timeout);
                }
                Err(e) => break Err(e.into()),
            }
        };

        // Restore the original timeout
        self.socket.set_read_timeout(original_timeout)?;

        result
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }

    fn send_semantics(&self) -> SendSemantics {
        // UDP sends are atomic at the datagram boundary - a failed send
        // does not affect the state for subsequent sends
        SendSemantics::Datagram
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        net::UdpSocket,
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::transport::{BlockingTransport, TransportConfig};

    fn connected_socket_pair() -> (UdpSocket, UdpSocket) {
        let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind receiver");
        let sender = UdpSocket::bind("127.0.0.1:0").expect("bind sender");
        receiver
            .connect(sender.local_addr().expect("sender address"))
            .expect("connect receiver");
        sender
            .connect(receiver.local_addr().expect("receiver address"))
            .expect("connect sender");
        (receiver, sender)
    }

    #[test]
    fn recv_discards_empty_datagram_before_valid_datagram() {
        let (receiver, sender) = connected_socket_pair();
        sender.send(&[]).expect("send empty datagram");
        sender.send(b"valid").expect("send valid datagram");

        let mut transport = Udp {
            socket: receiver,
            config: TransportConfig::default(),
        };
        let mut dst = [0; 16];

        let received = transport.recv_into(&mut dst).expect("valid datagram");

        assert_eq!(received, 5);
        assert_eq!(&dst[..received], b"valid");
    }

    #[test]
    fn recv_into_with_timeout_expires_after_empty_datagram() {
        let (receiver, sender) = connected_socket_pair();
        sender.send(&[]).expect("send empty datagram");

        let mut transport = Udp {
            socket: receiver,
            config: TransportConfig::default(),
        };
        let mut dst = [0; 16];
        let timeout = Duration::from_millis(80);
        let started = Instant::now();

        let result = transport.recv_into_with_timeout(&mut dst, timeout);

        assert!(matches!(result, Err(Error::Timeout)));
        assert!(started.elapsed() >= timeout.saturating_sub(Duration::from_millis(20)));
    }

    #[test]
    fn recv_into_with_timeout_does_not_restart_after_empty_datagram() {
        let (receiver, sender) = connected_socket_pair();
        let receiver_address = receiver.local_addr().expect("receiver address");
        let sender = thread::spawn(move || {
            thread::sleep(Duration::from_millis(40));
            sender
                .send_to(&[], receiver_address)
                .expect("send empty datagram");
            thread::sleep(Duration::from_millis(60));
            sender
                .send_to(b"too late", receiver_address)
                .expect("send valid datagram");
        });

        let mut transport = Udp {
            socket: receiver,
            config: TransportConfig::default(),
        };
        let mut dst = [0; 16];
        let result = transport.recv_into_with_timeout(&mut dst, Duration::from_millis(70));

        sender.join().expect("sender thread");
        assert!(matches!(result, Err(Error::Timeout)));
    }
}
