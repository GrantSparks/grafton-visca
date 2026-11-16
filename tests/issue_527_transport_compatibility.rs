#![cfg(feature = "blocking")]

use grafton_visca::{
    config::{TransportKind, TransportOptions},
    profiles::{ProfileId, SonyFr7},
    Error,
};

#[test]
fn dynamic_tcp_for_sony_fr7_fails_before_address_resolution() {
    let config = grafton_visca::config::CameraConfig::new(
        grafton_visca::profiles::ProfileSpec::from_compile_time::<SonyFr7>()
            .expect("source profile"),
        grafton_visca::config::TransportOptions::Custom,
    )
    .into_typed::<SonyFr7>()
    .expect("typed config")
    .with_transport(TransportOptions::tcp(
        "issue-527-should-not-resolve.invalid",
    ));

    let error = grafton_visca::blocking::CameraSession::connect(&config)
        .expect_err("SonyFr7 TCP should be rejected by registry validation");

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: ProfileId::SonyFr7,
            transport: TransportKind::Tcp,
            ..
        }
    ));
}

#[test]
fn dynamic_transport_validation_reports_profile_and_transport() {
    let transport = TransportOptions::serial("/dev/issue-527-no-open", 9600);
    let error = transport
        .validate_for_profile(ProfileId::SonyFr7)
        .expect_err("SonyFr7 serial support is not source-backed");

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: ProfileId::SonyFr7,
            transport: TransportKind::Serial,
            ..
        }
    ));
}

#[test]
fn registry_default_ports_exist_only_for_supported_network_transports() {
    for profile in ProfileId::all() {
        assert_eq!(
            profile.default_tcp_port().is_some(),
            profile
                .spec()
                .expect("registered profile")
                .transports()
                .tcp_port()
                .is_some()
        );
        assert_eq!(
            profile.default_udp_port().is_some(),
            profile
                .spec()
                .expect("registered profile")
                .transports()
                .udp_port()
                .is_some()
        );
    }
}
