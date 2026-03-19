//! Shared socket configuration helpers for TCP and UDP transports.
//!
//! This module centralizes transport-level socket option handling so the
//! blocking and async runtimes apply the same semantics.

use std::time::Duration;

#[cfg(unix)]
use std::os::unix::io::AsFd;
#[cfg(windows)]
use std::os::windows::io::AsSocket;

use crate::{
    transport::builder::{TransportConfig, DEFAULT_TCP_KEEPALIVE_INTERVAL},
    Error,
};

/// Configuration for TCP connection behavior.
#[cfg_attr(
    not(any(
        feature = "runtime-tokio",
        feature = "runtime-async-std",
        feature = "runtime-smol"
    )),
    allow(dead_code)
)]
#[derive(Debug, Clone, Copy)]
pub struct TcpConnectionConfig {
    /// Whether to enable TCP_NODELAY (Nagle's algorithm disable).
    pub nodelay: Option<bool>,
    /// Time-to-live for packets.
    pub ttl: Option<u32>,
    /// Connection timeout duration.
    pub connect_timeout: Duration,
    /// TCP keepalive interval. When set, enables OS-level TCP keepalive probes
    /// at the specified interval to prevent camera-side idle timeout.
    pub tcp_keepalive: Option<Duration>,
}

impl TcpConnectionConfig {
    /// Whether TCP_NODELAY should be enabled for this connection.
    pub(crate) fn nodelay_enabled(self) -> bool {
        self.nodelay.unwrap_or(true)
    }
}

impl Default for TcpConnectionConfig {
    fn default() -> Self {
        Self {
            nodelay: Some(true),
            ttl: None,
            connect_timeout: Duration::from_secs(5),
            tcp_keepalive: Some(DEFAULT_TCP_KEEPALIVE_INTERVAL),
        }
    }
}

impl From<TransportConfig> for TcpConnectionConfig {
    fn from(config: TransportConfig) -> Self {
        Self {
            nodelay: config.tcp_nodelay,
            ttl: config.ttl,
            connect_timeout: config.connect_timeout,
            tcp_keepalive: config.tcp_keepalive,
        }
    }
}

/// Configuration for UDP socket behavior.
#[cfg_attr(
    not(any(
        feature = "runtime-tokio",
        feature = "runtime-async-std",
        feature = "runtime-smol"
    )),
    allow(dead_code)
)]
#[derive(Debug, Clone, Copy)]
pub struct UdpSocketConfig {
    /// Time-to-live for packets.
    pub ttl: Option<u32>,
    /// Connection timeout duration.
    pub connect_timeout: Duration,
}

impl Default for UdpSocketConfig {
    fn default() -> Self {
        Self {
            ttl: None,
            connect_timeout: Duration::from_secs(5),
        }
    }
}

impl From<TransportConfig> for UdpSocketConfig {
    fn from(config: TransportConfig) -> Self {
        Self {
            ttl: config.ttl,
            connect_timeout: config.connect_timeout,
        }
    }
}

/// Apply TCP socket options to any socket type that exposes an OS socket handle.
#[cfg(unix)]
pub fn apply_tcp_socket_options<S>(socket: &S, config: TcpConnectionConfig) -> Result<(), Error>
where
    S: AsFd,
{
    let socket_ref = socket2::SockRef::from(socket);
    socket_ref.set_nodelay(config.nodelay_enabled())?;

    if let Some(ttl) = config.ttl {
        socket_ref.set_ttl(ttl)?;
    }

    apply_tcp_keepalive(socket, config.tcp_keepalive)
}

/// Windows implementation of [`apply_tcp_socket_options`].
#[cfg(windows)]
pub fn apply_tcp_socket_options<S>(socket: &S, config: TcpConnectionConfig) -> Result<(), Error>
where
    S: AsSocket,
{
    let socket_ref = socket2::SockRef::from(socket);
    socket_ref.set_nodelay(config.nodelay_enabled())?;

    if let Some(ttl) = config.ttl {
        socket_ref.set_ttl(ttl)?;
    }

    apply_tcp_keepalive(socket, config.tcp_keepalive)
}

/// Apply TCP keepalive to any socket type that exposes an OS socket handle.
///
/// This returns an error if keepalive was requested but could not be applied.
#[cfg(unix)]
pub fn apply_tcp_keepalive<S>(socket: &S, tcp_keepalive: Option<Duration>) -> Result<(), Error>
where
    S: AsFd,
{
    if let Some(interval) = tcp_keepalive {
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(interval)
            .with_interval(interval);
        socket2::SockRef::from(socket).set_tcp_keepalive(&keepalive)?;
    }

    Ok(())
}

/// Windows implementation of [`apply_tcp_keepalive`].
#[cfg(windows)]
pub fn apply_tcp_keepalive<S>(socket: &S, tcp_keepalive: Option<Duration>) -> Result<(), Error>
where
    S: AsSocket,
{
    if let Some(interval) = tcp_keepalive {
        let keepalive = socket2::TcpKeepalive::new()
            .with_time(interval)
            .with_interval(interval);
        socket2::SockRef::from(socket).set_tcp_keepalive(&keepalive)?;
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };

    #[test]
    fn tcp_connection_config_defaults_match_transport_defaults() {
        let transport = TransportConfig::default();
        let tcp = TcpConnectionConfig::default();

        assert_eq!(
            transport.tcp_keepalive,
            Some(DEFAULT_TCP_KEEPALIVE_INTERVAL)
        );
        assert_eq!(tcp.tcp_keepalive, Some(DEFAULT_TCP_KEEPALIVE_INTERVAL));
        assert_eq!(tcp.tcp_keepalive, transport.tcp_keepalive);
        assert!(tcp.nodelay_enabled());
        assert_eq!(transport.tcp_nodelay, Some(true));
    }

    #[test]
    fn apply_tcp_keepalive_enables_socket_keepalive() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener local addr");

        let accept_thread = thread::spawn(move || {
            let (_stream, _peer) = listener.accept().expect("accept connection");
        });

        let stream = TcpStream::connect(addr).expect("connect stream");
        apply_tcp_keepalive(&stream, Some(DEFAULT_TCP_KEEPALIVE_INTERVAL))
            .expect("apply keepalive");

        let keepalive_enabled = socket2::SockRef::from(&stream)
            .keepalive()
            .expect("read keepalive state");
        assert!(keepalive_enabled, "TCP keepalive should be enabled");

        accept_thread.join().expect("join accept thread");
    }
}
