//! Cross-entry-point agreement tests for the built-in transports.
//!
//! Built-in connection plans, socket connectors, and caller-owned transports
//! share the checked bounds and preserve their physical transport identity.

#![allow(clippy::expect_used, clippy::panic)]

#[cfg(all(
    target_os = "linux",
    any(
        feature = "blocking",
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
mod tcp_queue;

#[cfg(feature = "blocking")]
use crate::transport::HasTransportConfig;
use crate::transport::{IoBounds, TcpConnect, UdpConnect};

const TCP_BOUNDS: IoBounds = IoBounds::TCP_DEFAULT;
const UDP_BOUNDS: IoBounds = IoBounds::UDP_DEFAULT;
#[cfg(feature = "blocking")]
const SERIAL_BOUNDS: IoBounds = IoBounds::SERIAL_DEFAULT;

#[test]
fn per_transport_defaults_are_internally_consistent() {
    assert_eq!(TCP_BOUNDS.buffer_config().recv_buffer_size(), 256);
    assert_eq!(UDP_BOUNDS.buffer_config().recv_buffer_size(), 1024);
    #[cfg(feature = "blocking")]
    assert_eq!(SERIAL_BOUNDS.buffer_config().recv_buffer_size(), 256);

    for bounds in [TCP_BOUNDS, UDP_BOUNDS] {
        assert_eq!(bounds.buffer_config().max_buffer_size(), 8192);
        assert_eq!(bounds.read_timeout(), std::time::Duration::from_secs(5));
        assert_eq!(bounds.write_timeout(), std::time::Duration::from_secs(5));
    }
    assert_eq!(
        TcpConnect::default().connect_timeout(),
        std::time::Duration::from_secs(5)
    );
    assert_eq!(
        UdpConnect::default().connect_timeout(),
        std::time::Duration::from_secs(5)
    );
    assert_eq!(TcpConnect::default().nodelay(), Some(true));
    assert_eq!(
        TcpConnect::default().keepalive(),
        Some(crate::transport::TcpKeepaliveConfig::default())
    );
    assert_eq!(TcpConnect::default().ttl(), None);
    assert_eq!(UdpConnect::default().ttl(), None);
}

/// Camera configuration lowers through the same TCP and UDP presets that the
/// connectors use.
#[cfg(feature = "blocking")]
#[test]
fn camera_config_agrees_with_the_transport_defaults() {
    use crate::{config::CameraConfig, profiles::PtzOpticsG2};
    let tcp = CameraConfig::<PtzOpticsG2>::tcp("camera.local");
    let udp = CameraConfig::<PtzOpticsG2>::udp("camera.local");
    assert_eq!(
        tcp.connection_plan()
            .map(|plan| plan.connection.transport.into_network())
            .expect("TCP plan")
            .io_bounds,
        TCP_BOUNDS
    );
    assert_eq!(
        udp.connection_plan()
            .map(|plan| plan.connection.transport.into_network())
            .expect("UDP plan")
            .io_bounds,
        UDP_BOUNDS
    );
}

/// An explicitly chosen generic buffer configuration is honoured instead of
/// being mistaken for an omitted override (#799).
#[cfg(feature = "blocking")]
#[test]
fn camera_config_honours_an_explicit_generic_buffer_choice() {
    use crate::{config::CameraConfig, profiles::PtzOpticsG2};
    let tcp = CameraConfig::<PtzOpticsG2>::tcp_configured(
        "camera.local",
        TcpConnect::default(),
        IoBounds::default(),
    )
    .connection_plan()
    .map(|plan| plan.connection.transport.into_network())
    .expect("explicit TCP plan");
    assert_eq!(
        tcp.io_bounds.buffer_config(),
        crate::transport::BufferConfig::default()
    );

    // A caller-selected preset does not change the connector's transport kind.
    let udp = CameraConfig::<PtzOpticsG2>::udp_configured(
        "camera.local",
        UdpConnect::default(),
        SERIAL_BOUNDS,
    )
    .connection_plan()
    .map(|plan| plan.connection.transport.into_network())
    .expect("explicit UDP plan");
    assert_eq!(
        udp.connector.kind(),
        crate::transport::NetworkTransport::Udp
    );
}

/// Live loopback checks of the blocking connectors.
#[cfg(feature = "blocking")]
mod blocking {
    use std::{
        net::{TcpListener, UdpSocket},
        time::Duration,
    };

    use super::*;
    use crate::{
        transport::{blocking_transport::BlockingTransportHandle, BlockingTransport},
        Error,
    };

    #[test]
    fn ip_connectors_derive_identity_independently_of_config_preset() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let tcp = crate::transport::blocking::Tcp::connect_with_config(
            &address,
            TcpConnect::default(),
            SERIAL_BOUNDS,
        )
        .expect("TCP with caller-selected buffers");
        let udp = crate::transport::blocking::Udp::connect_with_config(
            &address,
            UdpConnect::default(),
            SERIAL_BOUNDS,
        )
        .expect("UDP with caller-selected buffers");
        assert_eq!(tcp.identity(), crate::transport::TransportIdentity::Tcp);
        assert_eq!(udp.identity(), crate::transport::TransportIdentity::Udp);
    }

    #[test]
    fn a_direct_ip_transport_accepts_a_nondefault_camera_reply_source() {
        use crate::{config::SessionConfig, profiles::GenericVisca};
        use std::io::{Read, Write};
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let (done, finished) = std::sync::mpsc::channel();
        let peer = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().expect("accept");
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("read bound");
            let mut request = Vec::new();
            while request.last() != Some(&0xff) {
                let mut byte = [0];
                socket.read_exact(&mut byte).expect("command byte");
                request.push(byte[0]);
            }
            assert_eq!(request[0], 0x81);
            socket
                .write_all(&[0xa0, 0x41, 0xff, 0xa0, 0x51, 0xff])
                .expect("reply");
            finished
                .recv_timeout(Duration::from_secs(2))
                .expect("session finished");
        });
        let transport = crate::transport::blocking::Tcp::connect_with_config(
            &address,
            TcpConnect::default(),
            TCP_BOUNDS,
        )
        .expect("connect");
        let session = crate::blocking::Session::open(
            transport,
            &SessionConfig::from_compile_time::<GenericVisca>().expect("profile"),
        )
        .expect("session");
        session
            .camera::<GenericVisca>(crate::IndividualCameraId::CAMERA_1)
            .expect("camera")
            .power()
            .on()
            .expect("A0 reply settles command");
        session.shutdown().expect("shutdown");
        done.send(()).expect("done");
        peer.join().expect("peer");
    }

    #[test]
    fn built_transports_report_their_defaults_and_complete_identity() {
        use crate::transport::TransportIdentity;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let peer = UdpSocket::bind("127.0.0.1:0").expect("bind UDP peer");

        let tcp = BlockingTransportHandle::Tcp(
            crate::transport::blocking::Tcp::connect_with_config(
                &listener.local_addr().expect("address").to_string(),
                TcpConnect::default(),
                TCP_BOUNDS,
            )
            .expect("TCP connects"),
        );
        let udp = BlockingTransportHandle::Udp(
            crate::transport::blocking::Udp::connect_with_config(
                &peer.local_addr().expect("address").to_string(),
                UdpConnect::default(),
                UDP_BOUNDS,
            )
            .expect("UDP connects"),
        );

        for (transport, expected, identity) in [
            (&tcp, TCP_BOUNDS, TransportIdentity::Tcp),
            (&udp, UDP_BOUNDS, TransportIdentity::Udp),
        ] {
            assert_eq!(transport.io_bounds(), &expected);
            assert_eq!(transport.identity(), identity);
        }
    }

    /// #800: a budget beyond the monotonic clock used to give three different
    /// outcomes. Every bounded operation now rejects it the same way, naming
    /// the timeout it came from.
    #[test]
    fn unrepresentable_budgets_are_rejected_identically() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let peer = UdpSocket::bind("127.0.0.1:0").expect("bind UDP peer");
        let mut tcp = BlockingTransportHandle::Tcp(
            crate::transport::blocking::Tcp::connect_with_config(
                &listener.local_addr().expect("address").to_string(),
                TcpConnect::default(),
                TCP_BOUNDS,
            )
            .expect("TCP connects"),
        );
        let mut udp = BlockingTransportHandle::Udp(
            crate::transport::blocking::Udp::connect_with_config(
                &peer.local_addr().expect("address").to_string(),
                UdpConnect::default(),
                UDP_BOUNDS,
            )
            .expect("UDP connects"),
        );

        let rejected = |result: Result<(), Error>, expected: &str| {
            assert!(
                matches!(
                    result,
                    Err(Error::InvalidParameter { parameter, ref reason, .. })
                        if parameter == expected
                            && reason == "timeout is too large for the monotonic clock"
                ),
                "unexpected result for {expected}: {result:?}"
            );
        };

        rejected(
            tcp.send_with_timeout(&[0x81, 0x01, 0xFF], Duration::MAX),
            "write_timeout",
        );
        rejected(
            udp.send_with_timeout(&[0x81, 0x01, 0xFF], Duration::MAX),
            "write_timeout",
        );
        let mut dst = [0; 16];
        for transport in [&mut tcp, &mut udp] {
            rejected(
                transport
                    .recv_into_with_timeout(&mut dst, Duration::MAX)
                    .map(drop),
                "read_timeout",
            );
            assert!(matches!(
                transport.recv_into_with_timeout(&mut dst, Duration::ZERO),
                Err(Error::Timeout { .. })
            ));
        }

        #[cfg(all(unix, feature = "transport-serial"))]
        {
            use serialport::SerialPort;

            use crate::transport::{serial::Config, AddressingMode};

            let (_master, slave) = serialport::TTYPort::pair().expect("pseudo-terminal pair");
            let mut serial = crate::transport::serial_blocking::SerialTransport::open(
                Config::new(slave.name().expect("pseudo-terminal path")),
                SERIAL_BOUNDS,
            )
            .expect("serial opens");
            rejected(
                serial.send_with_timeout(&[0x81, 0x01, 0xFF], Duration::MAX),
                "write_timeout",
            );
            rejected(
                serial
                    .recv_into_with_timeout(&mut dst, Duration::MAX)
                    .map(drop),
                "read_timeout",
            );
            assert!(matches!(
                serial.recv_into_with_timeout(&mut dst, Duration::ZERO),
                Err(Error::Timeout { .. })
            ));
            assert_eq!(serial.io_bounds(), &SERIAL_BOUNDS);
            assert_eq!(
                serial.identity().addressing(),
                AddressingMode::Serial,
                "a serial transport never reports IP addressing"
            );
        }
    }
}

/// The runtime connectors share name-resolution errors with blocking and
/// pass the caller's checked settings through to the transport.
#[cfg(all(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
mod facades {
    use std::net::TcpListener;

    use super::*;
    use crate::{
        camera::config::CameraConfig,
        profiles::PtzOpticsG2,
        runtime::{Runtime, SmolRuntime, TokioRuntime},
        transport::NetworkEndpoint,
        Error,
    };

    #[tokio::test]
    async fn malformed_endpoint_is_refused_on_each_runtime() {
        let tokio = TokioRuntime::from_current().expect("runtime");
        let smol = SmolRuntime::new();
        let tcp = CameraConfig::<PtzOpticsG2>::tcp("127.1:1259");
        let udp = CameraConfig::<PtzOpticsG2>::udp("127.1:1259");
        for error in [
            crate::r#async::CameraSession::connect(&tcp, tokio.clone())
                .await
                .expect_err("invalid endpoint"),
            crate::r#async::CameraSession::connect(&udp, tokio)
                .await
                .expect_err("invalid endpoint"),
            ::smol::block_on(crate::r#async::CameraSession::connect(&tcp, smol))
                .expect_err("invalid endpoint"),
            ::smol::block_on(crate::r#async::CameraSession::connect(&udp, smol))
                .expect_err("invalid endpoint"),
        ] {
            assert!(matches!(error, Error::InvalidAddress { .. }), "{error:?}");
        }
    }

    #[tokio::test]
    async fn every_async_ip_connector_derives_identity_from_its_transport() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let endpoint = NetworkEndpoint::parse(&address, None).expect("endpoint");
        let config = SERIAL_BOUNDS;
        let tokio = TokioRuntime::from_current().expect("Tokio runtime");
        let smol = SmolRuntime::new();
        macro_rules! check {
            ($transport:expr, $identity:expr) => {{
                let transport = $transport.expect("caller-selected I/O configuration");
                assert_eq!(transport.identity(), $identity);
                assert_eq!(transport.io_bounds(), &config);
            }};
        }
        check!(
            tokio
                .connect_tcp(endpoint.clone(), TcpConnect::default(), config)
                .await,
            crate::transport::TransportIdentity::Tcp
        );
        check!(
            tokio
                .connect_udp(endpoint.clone(), UdpConnect::default(), config)
                .await,
            crate::transport::TransportIdentity::Udp
        );
        check!(
            ::smol::block_on(smol.connect_tcp(endpoint.clone(), TcpConnect::default(), config)),
            crate::transport::TransportIdentity::Tcp
        );
        check!(
            ::smol::block_on(smol.connect_udp(endpoint, UdpConnect::default(), config)),
            crate::transport::TransportIdentity::Udp
        );
    }

    /// #798: an unresolvable host was `InvalidAddress` on the blocking
    /// facade but `Io` on Tokio and smol. Every facade now preserves the same
    /// resolver I/O cause in `ConnectionFailed` for both IP transports.
    #[tokio::test]
    async fn an_unresolvable_host_fails_identically_on_every_facade() {
        // RFC 6761 reserves `.invalid`; it never resolves.
        const HOST: &str = "grafton-visca.invalid:5678";

        let endpoint = NetworkEndpoint::parse(HOST, None).expect("endpoint");
        let tcp = TcpConnect::default();
        let udp = UdpConnect::default();
        let tokio_runtime = TokioRuntime::from_current().expect("Tokio runtime");
        let smol_runtime = SmolRuntime::new();
        let errors: Vec<(&str, Error)> = vec![
            (
                "blocking TCP",
                crate::transport::blocking::Tcp::connect_with_config(
                    HOST,
                    TcpConnect::default(),
                    TCP_BOUNDS,
                )
                .expect_err("unresolvable"),
            ),
            (
                "blocking UDP",
                crate::transport::blocking::Udp::connect_with_config(
                    HOST,
                    UdpConnect::default(),
                    UDP_BOUNDS,
                )
                .expect_err("unresolvable"),
            ),
            (
                "Tokio TCP",
                tokio_runtime
                    .connect_tcp(endpoint.clone(), tcp, TCP_BOUNDS)
                    .await
                    .expect_err("unresolvable"),
            ),
            (
                "Tokio UDP",
                tokio_runtime
                    .connect_udp(endpoint.clone(), udp, UDP_BOUNDS)
                    .await
                    .expect_err("unresolvable"),
            ),
            (
                "smol TCP",
                ::smol::block_on(smol_runtime.connect_tcp(endpoint.clone(), tcp, TCP_BOUNDS))
                    .expect_err("unresolvable"),
            ),
            (
                "smol UDP",
                ::smol::block_on(smol_runtime.connect_udp(endpoint.clone(), udp, UDP_BOUNDS))
                    .expect_err("unresolvable"),
            ),
        ];

        let (_, first) = &errors[0];
        let first_source = std::error::Error::source(first)
            .expect("resolver source")
            .downcast_ref::<std::io::Error>()
            .expect("direct I/O source");
        let source_kind = first_source.kind();
        let source_os_error = first_source.raw_os_error();
        for (facade, error) in &errors {
            assert!(
                matches!(error, Error::ConnectionFailed { addr, .. } if addr == HOST),
                "{facade}: {error:?}"
            );
            let source = std::error::Error::source(error)
                .expect("resolver source")
                .downcast_ref::<std::io::Error>()
                .expect("direct I/O source");
            assert_eq!(source.kind(), source_kind, "{facade}");
            assert_eq!(source.raw_os_error(), source_os_error, "{facade}");
            let dynamic: &(dyn std::error::Error + 'static) = error;
            let dynamic_source = dynamic
                .source()
                .expect("dynamic resolver source")
                .downcast_ref::<std::io::Error>()
                .expect("dynamic direct I/O source");
            assert!(std::ptr::eq(source, dynamic_source), "{facade}");
            if let Error::ConnectionFailed { source: stored, .. } = error {
                assert!(std::ptr::eq(source, stored.as_ref()), "{facade}");
            }
        }
    }
}

/// Real UDP writes through a hostname must reach the IPv4 camera even when the
/// system resolver places IPv6 first. The pure selection test supplies both
/// families deterministically; these cover the actual connector entry points.
#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
mod udp_address_selection {
    use super::{UdpConnect, UDP_BOUNDS};
    use std::{net::UdpSocket, time::Duration};

    fn peer() -> (UdpSocket, String) {
        let peer = UdpSocket::bind("127.0.0.1:0").expect("IPv4 camera");
        peer.set_read_timeout(Some(Duration::from_secs(1)))
            .expect("bounded receive");
        let endpoint = format!(
            "localhost:{}",
            peer.local_addr().expect("camera address").port()
        );
        (peer, endpoint)
    }

    fn received(peer: UdpSocket) {
        let mut bytes = [0; 16];
        let (length, source) = peer
            .recv_from(&mut bytes)
            .expect("camera received UDP frame");
        assert!(source.is_ipv4());
        assert_eq!(&bytes[..length], &[0x81, 0xff]);
    }

    #[cfg(feature = "blocking")]
    #[test]
    fn blocking_udp_localhost_reaches_the_ipv4_camera() {
        use crate::transport::BlockingTransport;
        let (peer, endpoint) = peer();
        let mut socket = crate::transport::blocking::Udp::connect_with_config(
            &endpoint,
            UdpConnect::default(),
            UDP_BOUNDS,
        )
        .expect("UDP setup");
        socket
            .send_with_timeout(&[0x81, 0xff], Duration::from_secs(1))
            .expect("send");
        received(peer);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_udp_localhost_reaches_the_ipv4_camera() {
        use crate::{
            runtime::{Runtime, TokioRuntime},
            transport::{AsyncTransport, NetworkEndpoint},
        };
        let (peer, endpoint) = peer();
        let mut socket = TokioRuntime::from_current()
            .expect("Tokio runtime")
            .connect_udp(
                NetworkEndpoint::parse(&endpoint, None).expect("endpoint"),
                UdpConnect::default(),
                UDP_BOUNDS,
            )
            .await
            .expect("UDP setup");
        socket.send(&[0x81, 0xff]).await.expect("send");
        received(peer);
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_udp_localhost_reaches_the_ipv4_camera() {
        use crate::{
            runtime::{Runtime, SmolRuntime},
            transport::{AsyncTransport, NetworkEndpoint},
        };
        ::smol::block_on(async {
            let (peer, endpoint) = peer();
            let mut socket = SmolRuntime::new()
                .connect_udp(
                    NetworkEndpoint::parse(&endpoint, None).expect("endpoint"),
                    UdpConnect::default(),
                    UDP_BOUNDS,
                )
                .await
                .expect("UDP setup");
            socket.send(&[0x81, 0xff]).await.expect("send");
            received(peer);
        });
    }
}
