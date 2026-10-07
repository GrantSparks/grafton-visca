//! Socket options shared by the blocking and async IP transports.
//!
//! Options are applied through [`socket2::SockRef`], which every std, Tokio
//! and smol socket converts into on Unix and Windows alike, so each option is
//! written once.

use crate::{
    transport::builder::{TcpKeepaliveConfig, TransportConfig},
    Error,
};

/// The TCP projection of a [`TransportConfig`].
#[derive(Debug, Clone, Copy)]
pub struct TcpConnectionConfig {
    /// `TCP_NODELAY`; `None` keeps the operating-system default.
    pub nodelay: Option<bool>,
    /// IPv4 TTL; `None` keeps the operating-system default.
    pub ttl: Option<u32>,
    /// Budget for resolving and connecting.
    pub connect_timeout: std::time::Duration,
    /// TCP keepalive policy. When set, enables OS-level probes that detect
    /// broken peers and may preserve idle network-path state. These probes are
    /// not application-level VISCA traffic.
    pub tcp_keepalive: Option<TcpKeepaliveConfig>,
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

/// The UDP projection of a [`TransportConfig`].
#[derive(Debug, Clone, Copy)]
pub struct UdpSocketConfig {
    /// IPv4 TTL; `None` keeps the operating-system default.
    pub ttl: Option<u32>,
    /// Budget for resolving and connecting.
    pub connect_timeout: std::time::Duration,
}

impl From<TransportConfig> for UdpSocketConfig {
    fn from(config: TransportConfig) -> Self {
        Self {
            ttl: config.ttl,
            connect_timeout: config.connect_timeout,
        }
    }
}

/// Apply the configured TCP options to a connected stream.
pub(crate) fn apply_tcp_socket_options(
    socket: socket2::SockRef<'_>,
    config: TcpConnectionConfig,
) -> Result<(), Error> {
    if let Some(nodelay) = config.nodelay {
        socket.set_tcp_nodelay(nodelay)?;
    }
    if let Some(ttl) = config.ttl {
        socket.set_ttl_v4(ttl)?;
    }
    if let Some(keepalive) = config.tcp_keepalive {
        socket.set_tcp_keepalive(&tcp_keepalive(keepalive))?;
    }
    Ok(())
}

/// Apply the configured UDP options to a bound socket.
pub(crate) fn apply_udp_socket_options(
    socket: socket2::SockRef<'_>,
    config: UdpSocketConfig,
) -> Result<(), Error> {
    if let Some(ttl) = config.ttl {
        socket.set_ttl_v4(ttl)?;
    }
    Ok(())
}

/// Lower a keepalive policy onto `socket2`, setting the probe interval on the
/// platforms that support configuring it and keeping the OS default elsewhere.
fn tcp_keepalive(config: TcpKeepaliveConfig) -> socket2::TcpKeepalive {
    let keepalive = socket2::TcpKeepalive::new().with_time(config.idle);
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
    if let Some(interval) = config.interval {
        return keepalive.with_interval(interval);
    }
    keepalive
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::builder::DEFAULT_TCP_KEEPALIVE;
    use std::{
        net::{TcpListener, TcpStream},
        thread,
    };

    #[test]
    #[cfg_attr(miri, ignore = "requires real TCP sockets")]
    fn the_tcp_defaults_enable_nodelay_and_keepalive() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener local addr");

        let accept_thread = thread::spawn(move || {
            let (_stream, _peer) = listener.accept().expect("accept connection");
        });

        let stream = TcpStream::connect(addr).expect("connect stream");
        let config = TcpConnectionConfig::from(TransportConfig::for_tcp());
        assert_eq!(config.tcp_keepalive, Some(DEFAULT_TCP_KEEPALIVE));
        apply_tcp_socket_options(socket2::SockRef::from(&stream), config)
            .expect("apply TCP options");

        let socket = socket2::SockRef::from(&stream);
        assert!(socket.keepalive().expect("read keepalive state"));
        assert!(socket.tcp_nodelay().expect("read nodelay state"));

        accept_thread.join().expect("join accept thread");
    }
}
