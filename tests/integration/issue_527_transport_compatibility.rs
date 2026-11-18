#![cfg(feature = "blocking")]

use grafton_visca::{
    config::{CameraConfig, TransportOptions},
    profiles::{ProfileId, SonyFr7},
    transport::StandardTransport,
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
            profile: Some(ProfileId::SonyFr7),
            transport: StandardTransport::Tcp,
            ..
        }
    ));
}

#[test]
fn dynamic_transport_validation_reports_profile_and_transport() {
    let config = CameraConfig::new(
        ProfileId::SonyFr7,
        TransportOptions::serial("/dev/issue-527-no-open", 9600),
    );
    let error = config
        .validate()
        .expect_err("SonyFr7 serial support is not source-backed");

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: Some(ProfileId::SonyFr7),
            transport: StandardTransport::Serial,
            ..
        }
    ));
}
