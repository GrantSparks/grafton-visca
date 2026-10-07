//! Cross-entry-point agreement tests for the built-in transports.
//!
//! Every built-in way to open a TCP, UDP or serial transport must produce the
//! defaults of [`TransportConfig::for_tcp`], [`TransportConfig::for_udp`] or
//! [`TransportConfig::for_serial`] (#799), report an addressing hint that
//! matches the configuration the owner frames with, and share one error
//! contract for budgets and name resolution (#798, #800).

#![allow(clippy::expect_used, clippy::panic)]

#[cfg(feature = "blocking")]
use crate::transport::HasTransportConfig;
use crate::transport::{AddressingMode, TransportConfig};

/// The two IP defaults differ only in their frame limit; both use IP
/// addressing. Serial uses serial addressing and no TCP socket options.
#[test]
fn per_transport_defaults_are_internally_consistent() {
    let tcp = TransportConfig::for_tcp();
    let udp = TransportConfig::for_udp();
    let serial = TransportConfig::for_serial();
    assert_eq!(tcp.addressing, AddressingMode::Ip);
    assert_eq!(udp.addressing, AddressingMode::Ip);
    assert_eq!(serial.addressing, AddressingMode::Serial);
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
    let udp = CameraConfig::<PtzOpticsG2>::udp("camera.local")
        .transport_config(TransportConfig::for_serial())
        .udp_transport_config();
    assert_eq!(udp.addressing, AddressingMode::Ip);
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
        command::CommandKind,
        transport::{BlockingTransport, Transport},
        Error,
    };

    #[test]
    fn built_transports_report_their_defaults_and_a_matching_hint() {
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

        for (transport, expected) in [
            (&tcp, TransportConfig::for_tcp()),
            (&udp, TransportConfig::for_udp()),
        ] {
            assert_eq!(transport.transport_config(), &expected);
            assert_eq!(
                transport.addressing_mode_hint(),
                Some(transport.transport_config().addressing)
            );
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
            tcp.send_with_timeout(&[0x81, 0x01, 0xFF], CommandKind::Command, Duration::MAX),
            "write_timeout",
        );
        rejected(
            udp.send_with_timeout(&[0x81, 0x01, 0xFF], CommandKind::Command, Duration::MAX),
            "write_timeout",
        );
        let mut dst = [0; 16];
        rejected(
            udp.recv_into_with_timeout(&mut dst, Duration::MAX)
                .map(drop),
            "read_timeout",
        );

        #[cfg(all(unix, feature = "transport-serial"))]
        {
            use serialport::SerialPort;

            use crate::transport::serial::Config;

            let (_master, slave) = serialport::TTYPort::pair().expect("pseudo-terminal pair");
            let mut serial = crate::transport::serial_blocking::SerialTransport::new(Config::new(
                slave.name().expect("pseudo-terminal path"),
            ))
            .expect("serial opens");
            rejected(
                serial.send_with_timeout(&[0x81, 0x01, 0xFF], CommandKind::Command, Duration::MAX),
                "write_timeout",
            );
            assert_eq!(serial.transport_config(), &TransportConfig::for_serial());
            assert_eq!(
                serial.addressing_mode_hint(),
                Some(AddressingMode::Serial),
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
        transport::{AsyncTransport, Transport},
        Error,
    };

    #[tokio::test]
    async fn async_direct_connectors_use_the_transport_defaults() {
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
        assert_eq!(tcp.addressing_mode_hint(), Some(AddressingMode::Ip));
        assert_eq!(udp.addressing_mode_hint(), Some(AddressingMode::Ip));

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
        assert_eq!(smol_tcp.addressing_mode_hint(), Some(AddressingMode::Ip));
        assert_eq!(smol_udp.addressing_mode_hint(), Some(AddressingMode::Ip));
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
