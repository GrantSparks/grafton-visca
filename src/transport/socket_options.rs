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
    transport::builder::{TcpKeepaliveConfig, TransportConfig, DEFAULT_TCP_KEEPALIVE},
    Error,
};

/// Configuration for TCP connection behavior.
#[derive(Debug, Clone, Copy)]
pub struct TcpConnectionConfig {
    /// Whether to enable TCP_NODELAY (Nagle's algorithm disable).
    pub nodelay: Option<bool>,
    /// Time-to-live for packets.
    pub ttl: Option<u32>,
    /// Connection timeout duration.
    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    pub connect_timeout: Duration,
    /// TCP keepalive policy. When set, enables OS-level TCP keepalive probes
    /// to prevent camera-side idle timeout.
    pub tcp_keepalive: Option<TcpKeepaliveConfig>,
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
            #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
            connect_timeout: Duration::from_secs(5),
            tcp_keepalive: Some(DEFAULT_TCP_KEEPALIVE),
        }
    }
}

impl From<TransportConfig> for TcpConnectionConfig {
    fn from(config: TransportConfig) -> Self {
        Self {
            nodelay: config.tcp_nodelay,
            ttl: config.ttl,
            #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
            connect_timeout: config.connect_timeout,
            tcp_keepalive: config.tcp_keepalive,
        }
    }
}

/// Configuration for UDP socket behavior.
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
#[derive(Debug, Clone, Copy)]
pub struct UdpSocketConfig {
    /// Time-to-live for packets.
    pub ttl: Option<u32>,
    /// Connection timeout duration.
    pub connect_timeout: Duration,
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
impl Default for UdpSocketConfig {
    fn default() -> Self {
        Self {
            ttl: None,
            connect_timeout: Duration::from_secs(5),
        }
    }
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
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
    socket_ref.set_tcp_nodelay(config.nodelay_enabled())?;

    if let Some(ttl) = config.ttl {
        socket_ref.set_ttl_v4(ttl)?;
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
    socket_ref.set_tcp_nodelay(config.nodelay_enabled())?;

    if let Some(ttl) = config.ttl {
        socket_ref.set_ttl_v4(ttl)?;
    }

    apply_tcp_keepalive(socket, config.tcp_keepalive)
}

/// Apply TCP keepalive to any socket type that exposes an OS socket handle.
///
/// This returns an error if keepalive was requested but could not be applied.
#[cfg(unix)]
pub fn apply_tcp_keepalive<S>(
    socket: &S,
    tcp_keepalive: Option<TcpKeepaliveConfig>,
) -> Result<(), Error>
where
    S: AsFd,
{
    if let Some(config) = tcp_keepalive {
        let mut keepalive = socket2::TcpKeepalive::new().with_time(config.idle);

        if let Some(interval) = config.interval {
            keepalive = tcp_keepalive_with_interval(keepalive, interval);
        }

        socket2::SockRef::from(socket).set_tcp_keepalive(&keepalive)?;
    }

    Ok(())
}

/// Windows implementation of [`apply_tcp_keepalive`].
#[cfg(windows)]
pub fn apply_tcp_keepalive<S>(
    socket: &S,
    tcp_keepalive: Option<TcpKeepaliveConfig>,
) -> Result<(), Error>
where
    S: AsSocket,
{
    if let Some(config) = tcp_keepalive {
        let mut keepalive = socket2::TcpKeepalive::new().with_time(config.idle);

        if let Some(interval) = config.interval {
            keepalive = tcp_keepalive_with_interval(keepalive, interval);
        }

        socket2::SockRef::from(socket).set_tcp_keepalive(&keepalive)?;
    }

    Ok(())
}

#[cfg(any(
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos",
    target_os = "ios",
    target_os = "visionos",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "windows",
))]
fn tcp_keepalive_with_interval(
    keepalive: socket2::TcpKeepalive,
    interval: Duration,
) -> socket2::TcpKeepalive {
    keepalive.with_interval(interval)
}

#[cfg(not(any(
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos",
    target_os = "ios",
    target_os = "visionos",
    target_os = "linux",
    target_os = "macos",
    target_os = "netbsd",
    target_os = "tvos",
    target_os = "watchos",
    target_os = "windows",
)))]
fn tcp_keepalive_with_interval(
    keepalive: socket2::TcpKeepalive,
    _interval: Duration,
) -> socket2::TcpKeepalive {
    keepalive
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

        assert_eq!(transport.tcp_keepalive, Some(DEFAULT_TCP_KEEPALIVE));
        assert_eq!(tcp.tcp_keepalive, Some(DEFAULT_TCP_KEEPALIVE));
        assert_eq!(tcp.tcp_keepalive, transport.tcp_keepalive);
        assert!(tcp.nodelay_enabled());
        assert_eq!(transport.tcp_nodelay, Some(true));
    }

    #[test]
    #[cfg_attr(miri, ignore = "requires real TCP sockets")]
    fn apply_tcp_keepalive_enables_socket_keepalive() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener local addr");

        let accept_thread = thread::spawn(move || {
            let (_stream, _peer) = listener.accept().expect("accept connection");
        });

        let stream = TcpStream::connect(addr).expect("connect stream");
        apply_tcp_keepalive(&stream, Some(DEFAULT_TCP_KEEPALIVE)).expect("apply keepalive");

        let keepalive_enabled = socket2::SockRef::from(&stream)
            .keepalive()
            .expect("read keepalive state");
        assert!(keepalive_enabled, "TCP keepalive should be enabled");

        accept_thread.join().expect("join accept thread");
    }
}
