//! Blocking UDP transport implementation with IPv6 support.

use std::{net::UdpSocket, time::Duration};

use crate::{
    command::CommandKind,
    transport::{
        address::AddressResolver, buffer::BufferConfig, builder::TransportConfig, SyncTransport,
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
}

impl Udp {
    /// Connect to a UDP endpoint.
    ///
    /// This method resolves hostnames and supports both IPv4 and IPv6 addresses.
    /// The socket will bind to the appropriate unspecified address based on the
    /// target address family.
    pub fn connect(address: &str) -> Result<Self, Error> {
        let config = TransportConfig {
            read_timeout: Duration::from_secs(5),
            write_timeout: Duration::from_secs(5),
            buffer_config: BufferConfig::for_udp(),
            ..Default::default()
        };
        Self::connect_with_config(address, config)
    }

    /// Connect with a full configuration.
    ///
    /// This method provides full control over connection and socket parameters.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        // Use the common address resolver
        let resolver = AddressResolver::new();
        let target_addr = resolver.resolve_first(address)?;

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

        Ok(Self { socket })
    }
}

impl SyncTransport for Udp {
    fn send_with_kind(&mut self, data: &[u8], _kind: CommandKind) -> Result<(), Error> {
        // Send directly - retry logic is handled at the runtime/scheduler level for async
        // For blocking mode, the blocking runner will handle retries
        self.socket.send(data)?;
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        match self.socket.recv(dst) {
            Ok(n) => Ok(n),
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Err(Error::Timeout),
            Err(e) => Err(e.into()),
        }
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        duration: Duration,
    ) -> Result<usize, Error> {
        // Save the current timeout
        let original_timeout = self.socket.read_timeout()?;

        // Set the new timeout for this operation
        self.socket.set_read_timeout(Some(duration))?;

        // Perform the receive operation
        let result = self.socket.recv(dst);

        // Restore the original timeout
        self.socket.set_read_timeout(original_timeout)?;

        // Handle the result
        match result {
            Ok(n) => Ok(n),
            Err(e)
                if e.kind() == std::io::ErrorKind::TimedOut
                    || e.kind() == std::io::ErrorKind::WouldBlock =>
            {
                Err(Error::Timeout)
            }
            Err(e) => Err(e.into()),
        }
    }
}
