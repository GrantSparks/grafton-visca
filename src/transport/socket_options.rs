//! Socket options shared by the blocking and async IP transports.
//!
//! Options are applied through [`socket2::SockRef`], which every std, Tokio
//! and smol socket converts into on Unix and Windows alike, so each option is
//! written once.

use crate::transport::{TcpConnect, TcpKeepaliveConfig, UdpConnect};

/// Create a TCP socket with every option applied before its first packet.
///
/// All connectors use this before initiating a handshake. Setup errors belong
/// to this address attempt, just like an error from `connect` itself.
pub(crate) fn configured_tcp_socket(
    address: std::net::SocketAddr,
    config: TcpConnect,
) -> std::io::Result<socket2::Socket> {
    let socket = socket2::Socket::new(
        socket2::Domain::for_address(address),
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )?;
    apply_tcp_socket_options(socket2::SockRef::from(&socket), address, config)?;
    Ok(socket)
}

/// Apply the configured TCP options before connecting a stream.
pub(crate) fn apply_tcp_socket_options(
    socket: socket2::SockRef<'_>,
    address: std::net::SocketAddr,
    config: TcpConnect,
) -> std::io::Result<()> {
    if let Some(nodelay) = config.nodelay() {
        socket.set_tcp_nodelay(nodelay)?;
    }
    if let Some(ttl) = config.ttl() {
        if address.is_ipv6() {
            socket.set_unicast_hops_v6(ttl)?;
        } else {
            socket.set_ttl_v4(ttl)?;
        }
    }
    if let Some(keepalive) = config.keepalive() {
        socket.set_tcp_keepalive(&tcp_keepalive(keepalive))?;
    }
    Ok(())
}

/// Apply the configured UDP options to a bound socket.
pub(crate) fn apply_udp_socket_options(
    socket: socket2::SockRef<'_>,
    address: std::net::SocketAddr,
    config: UdpConnect,
) -> std::io::Result<()> {
    if let Some(ttl) = config.ttl() {
        if address.is_ipv6() {
            socket.set_unicast_hops_v6(ttl)?;
        } else {
            socket.set_ttl_v4(ttl)?;
        }
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
    fn tcp_options_are_set_while_the_socket_is_still_unconnected() {
        for address in ["127.0.0.1:1259", "[::1]:1259"] {
            let address = address.parse::<std::net::SocketAddr>().expect("address");
            if address.is_ipv6() && TcpListener::bind("[::1]:0").is_err() {
                continue;
            }
            let socket = configured_tcp_socket(
                address,
                TcpConnect::default()
                    .with_ttl(Some(2))
                    .expect("valid connector test configuration"),
            )
            .expect("configure unconnected TCP socket");
            assert!(socket.peer_addr().is_err(), "no handshake has started");
            assert!(socket.tcp_nodelay().expect("nodelay"));
            assert!(socket.keepalive().expect("keepalive"));
            assert_eq!(
                if address.is_ipv6() {
                    socket.unicast_hops_v6().expect("hop limit")
                } else {
                    socket.ttl_v4().expect("TTL")
                },
                2,
                "the SYN will use the configured hop limit",
            );
        }
    }

    #[test]
    fn the_tcp_defaults_enable_nodelay_and_keepalive() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let addr = listener.local_addr().expect("listener local addr");

        let accept_thread = thread::spawn(move || {
            let (_stream, _peer) = listener.accept().expect("accept connection");
        });

        let stream = TcpStream::connect(addr).expect("connect stream");
        let config = TcpConnect::default();
        assert_eq!(config.keepalive(), Some(DEFAULT_TCP_KEEPALIVE));
        apply_tcp_socket_options(socket2::SockRef::from(&stream), addr, config)
            .expect("apply TCP options");

        let socket = socket2::SockRef::from(&stream);
        assert!(socket.keepalive().expect("read keepalive state"));
        assert!(socket.tcp_nodelay().expect("read nodelay state"));

        accept_thread.join().expect("join accept thread");
    }
    #[test]
    fn ipv4_ttl_remains_applied_to_tcp_and_udp() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("IPv4 listener");
        let port = listener.local_addr().expect("listener address").port();
        let endpoint =
            crate::transport::NetworkEndpoint::parse(&format!("[::ffff:127.0.0.1]:{port}"), None)
                .expect("mapped endpoint");
        let address = endpoint.socket_addr().expect("literal address");
        assert!(address.is_ipv4());
        let stream = TcpStream::connect(address).expect("IPv4 connect");
        apply_tcp_socket_options(
            socket2::SockRef::from(&stream),
            address,
            TcpConnect::default()
                .with_ttl(Some(7))
                .expect("valid connector test configuration"),
        )
        .expect("TCP options");
        assert_eq!(stream.ttl().expect("IPv4 TCP TTL"), 7);
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").expect("IPv4 UDP socket");
        apply_udp_socket_options(
            socket2::SockRef::from(&socket),
            address,
            UdpConnect::default()
                .with_ttl(Some(7))
                .expect("valid connector test configuration"),
        )
        .expect("UDP options");
        assert_eq!(socket.ttl().expect("IPv4 UDP TTL"), 7);
    }
}
