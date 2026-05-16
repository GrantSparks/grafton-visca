#![cfg(not(feature = "mode-async"))]

use grafton_visca::{
    camera::{
        profiles::{ProfileId, SonyFR7},
        CameraBuilder, CameraConfig, TransportKind, TransportOptions,
    },
    transport::Transport,
    Error,
};

use std::{net::TcpListener, thread};

#[test]
fn dynamic_tcp_for_sony_fr7_fails_before_address_resolution() {
    let config = CameraConfig::<SonyFR7>::new().transport(TransportOptions::tcp(
        "issue-527-should-not-resolve.invalid",
    ));

    let error = config
        .open_blocking()
        .expect_err("SonyFR7 TCP should be rejected by registry validation");

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: ProfileId::SonyFr7,
            transport: TransportKind::Tcp,
        }
    ));
}

#[test]
fn dynamic_transport_validation_reports_profile_and_transport() {
    let transport = TransportOptions::serial("/dev/issue-527-no-open", 9600);
    let error = transport
        .validate_for_profile(ProfileId::SonyFr7)
        .expect_err("SonyFR7 serial support is not source-backed");

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: ProfileId::SonyFr7,
            transport: TransportKind::Serial,
        }
    ));
}

#[test]
fn known_byo_tcp_handle_for_sony_fr7_is_validated_before_protocol_startup() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind local TCP listener");
    let addr = listener.local_addr().expect("read local listener address");
    let accept_thread = thread::spawn(move || {
        let _ = listener.accept();
    });

    let transport = Transport::tcp()
        .address(addr.to_string())
        .build_blocking()
        .expect("local TCP transport should connect");

    let result = CameraBuilder::from_transport_handle(transport)
        .profile::<SonyFR7>()
        .open();

    let error = match result {
        Err(error) => error,
        Ok(camera) => {
            let _ = camera.close();
            panic!("SonyFR7 over a known TCP BYO transport should be rejected");
        }
    };

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: ProfileId::SonyFr7,
            transport: TransportKind::Tcp,
        }
    ));

    accept_thread.join().expect("accept thread should finish");
}

#[test]
fn registry_default_ports_exist_only_for_supported_network_transports() {
    for profile in ProfileId::all() {
        assert_eq!(profile.default_tcp_port().is_some(), profile.supports_tcp());
        assert_eq!(profile.default_udp_port().is_some(), profile.supports_udp());
    }
}
