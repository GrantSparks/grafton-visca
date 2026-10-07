//! Blocking TCP transport implementation with DNS resolution and IPv6 support.

use std::{
    io::BufReader,
    net::TcpStream,
    time::{Duration, Instant},
};

use crate::{
    command::CommandKind,
    timeout::Deadline,
    transport::{
        address::resolve_blocking,
        blocking::{read_once_bounded, write_all_bounded, TimedIo},
        builder::{AddressingMode, TransportConfig},
        connect::{connect_deadline, preflight, TcpAttempts},
        socket_options::{apply_tcp_socket_options, TcpConnectionConfig},
        stream_read, BlockingTransport, HasTransportConfig, ReceiveOutcome,
    },
    Error,
};

fn arm_write(stream: &mut TcpStream, timeout: Duration) -> std::io::Result<()> {
    stream.set_write_timeout(Some(timeout))
}

fn arm_read(reader: &mut BufReader<TcpStream>, timeout: Duration) -> std::io::Result<()> {
    reader.get_mut().set_read_timeout(Some(timeout))
}

/// TCP transport for blocking VISCA communication.
///
/// This transport supports DNS resolution and both IPv4 and IPv6 addresses.
/// It operates at the stream level, reading/writing raw bytes.
/// Framing is handled by the runtime layer.
pub struct Tcp {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    config: TransportConfig,
}

impl std::fmt::Debug for Tcp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tcp")
            .field("reader", &self.reader)
            .field("writer", &self.writer)
            .finish()
    }
}

impl Tcp {
    /// Connect with a full configuration.
    ///
    /// The address must include an explicit port; explicit IPv6 ports require
    /// brackets. Name resolution and connection setup share
    /// `config.connect_timeout`, and each resolved address is tried in order.
    /// See [`crate::transport::connect`] for the error contract shared with
    /// the async connectors.
    pub fn connect_with_config(address: &str, config: TransportConfig) -> Result<Self, Error> {
        let endpoint = preflight(address, &config)?;
        let options = TcpConnectionConfig::from(config);
        let deadline = connect_deadline(Instant::now(), options.connect_timeout)?;
        let mut attempts = TcpAttempts::new(resolve_blocking(&endpoint, deadline)?, deadline);

        while let Some((address, remaining)) = attempts.next_attempt(Instant::now()) {
            match TcpStream::connect_timeout(&address, remaining) {
                Ok(stream) => {
                    apply_tcp_socket_options(socket2::SockRef::from(&stream), options)?;
                    let reader_stream = stream.try_clone()?;
                    return Ok(Self {
                        reader: BufReader::new(reader_stream),
                        writer: stream,
                        config,
                    });
                }
                Err(error) => attempts.failed(Some(error)),
            }
        }
        Err(attempts.into_error(&endpoint, Instant::now()))
    }
}

impl HasTransportConfig for Tcp {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
        Some(crate::camera::TransportKind::Tcp)
    }
}

impl BlockingTransport for Tcp {
    fn send_with_timeout(
        &mut self,
        data: &[u8],
        _kind: CommandKind,
        timeout: Duration,
    ) -> Result<(), Error> {
        let deadline = Deadline::after(Instant::now(), timeout, "write_timeout")?;
        write_all_bounded(&mut self.writer, arm_write, data, deadline, timeout)
    }

    fn recv_into_with_timeout(
        &mut self,
        dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let read = read_once_bounded(
            &mut self.reader,
            arm_read,
            dst,
            timeout,
            TimedIo::StreamSocketRead,
        )?;
        stream_read(read, "peer closed connection").map(ReceiveOutcome::complete)
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        io::{self, ErrorKind},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    use super::*;
    use crate::transport::BufferConfig;

    fn invalid_buffer_config() -> TransportConfig {
        TransportConfig {
            buffer_config: BufferConfig {
                recv_buffer_size: 65,
                max_buffer_size: 64,
            },
            ..TransportConfig::default()
        }
    }

    #[test]
    fn invalid_config_returns_before_a_listener_can_accept() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        listener
            .set_nonblocking(true)
            .expect("make listener nonblocking");
        let address = listener.local_addr().expect("listener address");

        let accept_thread = thread::spawn(move || -> Result<bool, io::Error> {
            let deadline = Instant::now() + Duration::from_millis(100);
            loop {
                match listener.accept() {
                    Ok(_) => return Ok(true),
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return Ok(false);
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(error) => return Err(error),
                }
            }
        });

        let result = Tcp::connect_with_config(&address.to_string(), invalid_buffer_config());
        let accepted = accept_thread
            .join()
            .expect("join accept observer")
            .expect("listener failed while observing connector");

        assert!(matches!(
            result,
            Err(Error::InvalidRequest(actual))
                if actual.as_ref() == "transport receive buffer cannot exceed maximum buffer"
        ));
        assert!(
            !accepted,
            "invalid configuration must be rejected before TCP connect"
        );
    }
}
