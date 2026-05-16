#![cfg(not(feature = "mode-async"))]

use grafton_visca::{
    camera::{
        profiles::{ProfileId, SonyFR7},
        CameraConfig, TransportKind, TransportOptions,
    },
    Error,
};

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
fn registry_default_ports_exist_only_for_supported_network_transports() {
    for profile in ProfileId::all() {
        assert_eq!(profile.default_tcp_port().is_some(), profile.supports_tcp());
        assert_eq!(profile.default_udp_port().is_some(), profile.supports_udp());
    }
}
