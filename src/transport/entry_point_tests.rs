//! Cross-entry-point agreement tests for the built-in transports.
//!
//! Every built-in way to open a TCP, UDP or serial transport must produce the
//! defaults of [`TransportConfig::for_tcp`], [`TransportConfig::for_udp`] or
//! [`TransportConfig::for_serial`] (#799), report an addressing hint that
//! matches the configuration the owner frames with, and share one error
//! contract for budgets and name resolution (#798, #800).

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
use crate::transport::TransportConfig;

#[cfg(feature = "blocking")]
fn invalid_bound_configurations() -> Vec<TransportConfig> {
    use crate::transport::{BufferConfig, TcpKeepaliveConfig};
    use std::time::Duration;

    let mut cases = Vec::new();
    for duration in [
        Duration::ZERO,
        Duration::from_millis(500),
        Duration::from_millis(1500),
        Duration::from_secs(32_768),
        Duration::MAX,
    ] {
        let mut idle = TransportConfig::for_tcp();
        idle.tcp_keepalive = Some(TcpKeepaliveConfig::new(duration));
        cases.push(idle);
        let mut interval = TransportConfig::for_tcp();
        interval.tcp_keepalive = Some(TcpKeepaliveConfig::default().with_interval(duration));
        cases.push(interval);
    }
    for ttl in [0, 256, u32::MAX] {
        let mut config = TransportConfig::for_tcp();
        config.ttl = Some(ttl);
        cases.push(config);
    }
    for (recv, max) in [
        (
            BufferConfig::MAX_RECV_BUFFER_SIZE + 1,
            BufferConfig::MAX_RETENTION_SIZE,
        ),
        (
            BufferConfig::MIN_RECV_BUFFER_SIZE,
            BufferConfig::MAX_RETENTION_SIZE + 1,
        ),
        (usize::MAX, usize::MAX),
    ] {
        let mut config = TransportConfig::for_tcp();
        config.buffer_config = BufferConfig {
            recv_buffer_size: recv,
            max_buffer_size: max,
        };
        cases.push(config);
    }
    cases
}

#[test]
fn maximum_supported_transport_bounds_validate() {
    use crate::transport::{BufferConfig, TcpKeepaliveConfig};
    use std::time::Duration;
    let mut config = TransportConfig::for_tcp();
    config.buffer_config = BufferConfig {
        recv_buffer_size: BufferConfig::MAX_RECV_BUFFER_SIZE,
        max_buffer_size: BufferConfig::MAX_RETENTION_SIZE,
    };
    config.tcp_keepalive = Some(
        TcpKeepaliveConfig::new(Duration::from_secs(32_767))
            .with_interval(Duration::from_secs(32_767)),
    );
    for ttl in [1, 255] {
        config.ttl = Some(ttl);
        config.validate().expect("supported boundary values");
    }
}

/// The two IP defaults differ only in their frame limit; both use IP
/// addressing. Serial uses serial addressing and no TCP socket options.
#[test]
fn per_transport_defaults_are_internally_consistent() {
    let tcp = TransportConfig::for_tcp();
    let udp = TransportConfig::for_udp();
    let serial = TransportConfig::for_serial();

    assert_eq!(tcp.buffer_config.recv_buffer_size, 256);
    assert_eq!(udp.buffer_config.recv_buffer_size, 1024);
    assert_eq!(serial.buffer_config.recv_buffer_size, 256);
    assert_eq!(
        (serial.tcp_nodelay, serial.ttl, serial.tcp_keepalive),
        (None, None, None)
    );
    // UDP carries no TCP socket options either.
    assert_eq!((udp.tcp_nodelay, udp.tcp_keepalive), (None, None));
    assert!(tcp.tcp_nodelay.is_some() && tcp.tcp_keepalive.is_some());
    for config in [tcp, udp, serial] {
        assert_eq!(
            config.connect_timeout,
            TransportConfig::default().connect_timeout
        );
        assert_eq!(config.read_timeout, TransportConfig::default().read_timeout);
        assert_eq!(
            config.write_timeout,
            TransportConfig::default().write_timeout
        );
        config.validate().expect("every default is valid");
    }
}

/// `CameraConfig` and the blocking builder start from the same defaults
/// (#799: the builder used to give TCP and UDP a 128-byte frame limit).
#[cfg(feature = "blocking")]
#[test]
fn camera_config_and_blocking_builder_agree_with_the_defaults() {
    use crate::{camera::CameraConfig, profiles::PtzOpticsG2};

    let camera_tcp = CameraConfig::<PtzOpticsG2>::tcp("camera.local").tcp_transport_config();
    let camera_udp = CameraConfig::<PtzOpticsG2>::udp("camera.local").udp_transport_config();
    assert_eq!(camera_tcp, TransportConfig::for_tcp());
    assert_eq!(camera_udp, TransportConfig::for_udp());
    assert_eq!(
        crate::transport::NetTransportBuilder::tcp().config(),
        &TransportConfig::for_tcp()
    );
    assert_eq!(
        crate::transport::NetTransportBuilder::udp().config(),
        &TransportConfig::for_udp()
    );
}

/// An explicitly chosen generic buffer configuration is honoured, not
/// mistaken for "unset" (#799: `CameraConfig` used to replace a caller's
/// `BufferConfig::default()` with the transport preset).
#[cfg(feature = "blocking")]
#[test]
fn camera_config_honours_an_explicit_generic_buffer_choice() {
    use crate::{camera::CameraConfig, profiles::PtzOpticsG2, transport::BufferConfig};

    let mut explicit = TransportConfig::for_tcp();
    explicit.buffer_config = BufferConfig::default();
    let tcp = CameraConfig::<PtzOpticsG2>::tcp("camera.local")
        .transport_config(explicit)
        .tcp_transport_config();
    assert_eq!(tcp.buffer_config, BufferConfig::default());

    // The transport kind still owns addressing.
    let _udp = CameraConfig::<PtzOpticsG2>::udp("camera.local")
        .transport_config(TransportConfig::for_serial())
        .udp_transport_config();
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
        transport::{BlockingTransport, Transport},
        Error,
    };

    #[test]
    fn invalid_configuration_fails_before_network_io() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let address = listener.local_addr().expect("address").to_string();
        for config in invalid_bound_configurations() {
            for error in [
                crate::transport::blocking::Tcp::connect_with_config(&address, config)
                    .expect_err("invalid TCP config"),
                crate::transport::blocking::Udp::connect_with_config(&address, config)
                    .expect_err("invalid UDP config"),
            ] {
                assert!(matches!(error, Error::InvalidRequest(_)), "{error:?}");
            }
            assert!(
                matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
            );
        }
    }

    #[test]
    fn ip_connectors_derive_identity_independently_of_config_preset() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let tcp = crate::transport::blocking::Tcp::connect_with_config(
            &address,
            TransportConfig::for_serial(),
        )
        .expect("TCP with caller-selected buffers");
        let udp = crate::transport::blocking::Udp::connect_with_config(
            &address,
            TransportConfig::for_serial(),
        )
        .expect("UDP with caller-selected buffers");
        assert_eq!(tcp.identity(), crate::transport::TransportIdentity::Tcp);
        assert_eq!(udp.identity(), crate::transport::TransportIdentity::Udp);
    }

    #[test]
    fn a_direct_ip_transport_accepts_a_nondefault_camera_reply_source() {
        use crate::{profiles::GenericVisca, SessionConfig};
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
            TransportConfig::for_tcp(),
        )
        .expect("connect");
        let session = crate::blocking::Session::open(
            transport,
            SessionConfig::from_compile_time::<GenericVisca>().expect("profile"),
        )
        .expect("session");
        session
            .camera::<GenericVisca>()
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

        let tcp = Transport::tcp()
            .address(listener.local_addr().expect("address").to_string())
            .build_blocking()
            .expect("TCP connects");
        let udp = Transport::udp()
            .address(peer.local_addr().expect("address").to_string())
            .build_blocking()
            .expect("UDP connects");

        for (transport, expected, identity) in [
            (&tcp, TransportConfig::for_tcp(), TransportIdentity::Tcp),
            (&udp, TransportConfig::for_udp(), TransportIdentity::Udp),
        ] {
            assert_eq!(transport.transport_config(), &expected);
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
        let mut tcp = Transport::tcp()
            .address(listener.local_addr().expect("address").to_string())
            .build_blocking()
            .expect("TCP connects");
        let mut udp = Transport::udp()
            .address(peer.local_addr().expect("address").to_string())
            .build_blocking()
            .expect("UDP connects");

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
            let mut serial = crate::transport::serial_blocking::SerialTransport::new(Config::new(
                slave.name().expect("pseudo-terminal path"),
            ))
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
            assert_eq!(serial.transport_config(), &TransportConfig::for_serial());
            assert_eq!(
                serial.identity().addressing(),
                AddressingMode::Serial,
                "a serial transport never reports IP addressing"
            );
        }
    }
}

/// The async direct connectors and runtime connectors use the same defaults
/// and the same name-resolution error as the blocking facade.
#[cfg(all(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
mod facades {
    use std::net::{TcpListener, UdpSocket};

    use super::*;
    use crate::{
        runtime::{Runtime, SmolRuntime, TokioRuntime},
        runtime_adapters::{smol, tokio as tokio_adapters},
        transport::Transport,
        Error,
    };

    #[tokio::test]
    async fn invalid_configuration_fails_before_io_on_every_runtime() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let address = listener.local_addr().expect("address").to_string();
        let tokio = TokioRuntime::from_current().expect("runtime");
        let smol = SmolRuntime::new();
        for config in invalid_bound_configurations() {
            for error in [
                tokio
                    .connect_tcp(&address, config)
                    .await
                    .expect_err("invalid Tokio TCP config"),
                tokio
                    .connect_udp(&address, config)
                    .await
                    .expect_err("invalid Tokio UDP config"),
                ::smol::block_on(smol.connect_tcp(&address, config))
                    .expect_err("invalid smol TCP config"),
                ::smol::block_on(smol.connect_udp(&address, config))
                    .expect_err("invalid smol UDP config"),
            ] {
                assert!(matches!(error, Error::InvalidRequest(_)), "{error:?}");
            }
            assert!(
                matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock)
            );
        }
    }

    #[tokio::test]
    async fn every_async_ip_connector_derives_identity_from_its_transport() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address").to_string();
        let config = TransportConfig::for_serial();
        let tokio = TokioRuntime::from_current().expect("Tokio runtime");
        let smol = SmolRuntime::new();
        macro_rules! check {
            ($transport:expr, $identity:expr) => {{
                let transport = $transport.expect("caller-selected I/O configuration");
                assert_eq!(transport.identity(), $identity);
            }};
        }
        check!(
            tokio.connect_tcp(&address, config).await,
            crate::transport::TransportIdentity::Tcp
        );
        check!(
            tokio.connect_udp(&address, config).await,
            crate::transport::TransportIdentity::Udp
        );
        check!(
            ::smol::block_on(smol.connect_tcp(&address, config)),
            crate::transport::TransportIdentity::Tcp
        );
        check!(
            ::smol::block_on(smol.connect_udp(&address, config)),
            crate::transport::TransportIdentity::Udp
        );
        check!(
            tokio_adapters::TcpTransport::connect_with_config(&address, config).await,
            crate::transport::TransportIdentity::Tcp
        );
        check!(
            tokio_adapters::UdpTransport::connect_with_config(&address, config).await,
            crate::transport::TransportIdentity::Udp
        );
        check!(
            ::smol::block_on(smol::TcpTransport::connect_with_config(&address, config)),
            crate::transport::TransportIdentity::Tcp
        );
        check!(
            ::smol::block_on(smol::UdpTransport::connect_with_config(&address, config)),
            crate::transport::TransportIdentity::Udp
        );
    }

    #[tokio::test]
    async fn async_direct_connectors_use_the_transport_defaults() {
        use crate::transport::AddressingMode;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let peer = UdpSocket::bind("127.0.0.1:0").expect("bind UDP peer");
        let tcp_address = listener.local_addr().expect("address").to_string();
        let udp_address = peer.local_addr().expect("address").to_string();

        let tcp = tokio_adapters::TcpTransport::connect(&tcp_address)
            .await
            .expect("Tokio TCP connects");
        let udp = tokio_adapters::UdpTransport::connect(&udp_address)
            .await
            .expect("Tokio UDP connects");
        assert_eq!(tcp.transport_config(), &TransportConfig::for_tcp());
        assert_eq!(udp.transport_config(), &TransportConfig::for_udp());
        assert_eq!(tcp.identity().addressing(), AddressingMode::Ip);
        assert_eq!(udp.identity().addressing(), AddressingMode::Ip);

        let (smol_tcp, smol_udp) = ::smol::block_on(async {
            (
                smol::TcpTransport::connect(&tcp_address)
                    .await
                    .expect("smol TCP connects"),
                smol::UdpTransport::connect(&udp_address)
                    .await
                    .expect("smol UDP connects"),
            )
        });
        assert_eq!(smol_tcp.transport_config(), &TransportConfig::for_tcp());
        assert_eq!(smol_udp.transport_config(), &TransportConfig::for_udp());
        assert_eq!(smol_tcp.identity().addressing(), AddressingMode::Ip);
        assert_eq!(smol_udp.identity().addressing(), AddressingMode::Ip);
    }

    /// #798: an unresolvable host was `InvalidAddress` on the blocking
    /// facade but `Io` on Tokio and smol. It is now the same error, with the
    /// same message, on every facade and both IP transports.
    #[tokio::test]
    async fn an_unresolvable_host_fails_identically_on_every_facade() {
        // RFC 6761 reserves `.invalid`; it never resolves.
        const HOST: &str = "grafton-visca.invalid:5678";

        let tokio_runtime = TokioRuntime::from_current().expect("Tokio runtime");
        let smol_runtime = SmolRuntime::new();
        let errors: Vec<(&str, Error)> = vec![
            (
                "blocking TCP",
                Transport::tcp()
                    .address(HOST)
                    .build_blocking()
                    .expect_err("unresolvable"),
            ),
            (
                "blocking UDP",
                Transport::udp()
                    .address(HOST)
                    .build_blocking()
                    .expect_err("unresolvable"),
            ),
            (
                "Tokio TCP",
                tokio_runtime
                    .connect_tcp(HOST, TransportConfig::for_tcp())
                    .await
                    .expect_err("unresolvable"),
            ),
            (
                "Tokio UDP",
                tokio_runtime
                    .connect_udp(HOST, TransportConfig::for_udp())
                    .await
                    .expect_err("unresolvable"),
            ),
            (
                "smol TCP",
                ::smol::block_on(smol_runtime.connect_tcp(HOST, TransportConfig::for_tcp()))
                    .expect_err("unresolvable"),
            ),
            (
                "smol UDP",
                ::smol::block_on(smol_runtime.connect_udp(HOST, TransportConfig::for_udp()))
                    .expect_err("unresolvable"),
            ),
        ];

        let (_, first) = &errors[0];
        let Error::InvalidAddress { reason } = first else {
            panic!("blocking TCP reported {first:?}");
        };
        assert!(
            reason.starts_with("Failed to resolve 'grafton-visca.invalid:5678'"),
            "unexpected reason: {reason}"
        );
        for (facade, error) in &errors {
            assert!(
                matches!(error, Error::InvalidAddress { reason: actual } if actual == reason),
                "{facade} reported {error:?}, expected the blocking facade's {first:?}"
            );
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
    use crate::transport::TransportConfig;
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
            TransportConfig::for_udp(),
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
        use crate::transport::AsyncTransport;
        let (peer, endpoint) = peer();
        let mut socket = crate::runtime_adapters::tokio::UdpTransport::connect_with_config(
            &endpoint,
            TransportConfig::for_udp(),
        )
        .await
        .expect("UDP setup");
        socket.send(&[0x81, 0xff]).await.expect("send");
        received(peer);
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_udp_localhost_reaches_the_ipv4_camera() {
        use crate::transport::AsyncTransport;
        ::smol::block_on(async {
            let (peer, endpoint) = peer();
            let mut socket = crate::runtime_adapters::smol::UdpTransport::connect_with_config(
                &endpoint,
                TransportConfig::for_udp(),
            )
            .await
            .expect("UDP setup");
            socket.send(&[0x81, 0xff]).await.expect("send");
            received(peer);
        });
    }
}
