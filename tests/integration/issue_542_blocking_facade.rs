//! Blocking owner startup guards for issue #542.

#![cfg(feature = "blocking")]

use grafton_visca_test_support::{fake_camera, profile_fixtures};

use grafton_visca::{
    blocking::{Camera, Session},
    config::{SessionConfig, TransportKind},
    profiles::{ProfileSpec, SonyFr7},
    transport::{AddressingMode, IoBounds, SendSemantics, StandardTransport},
    Error,
};

use fake_camera::{BlockingWire, FakeCamera};
use profile_fixtures::NonDefaultCompileTimeProfile;

/// A fake camera and the wire onto it: IP addressing and datagram semantics
/// unless `kind` is serial, which selects serial addressing and stream
/// semantics.
fn wire(kind: Option<TransportKind>) -> (FakeCamera, BlockingWire) {
    let camera = FakeCamera::silent();
    let wire = camera.blocking_wire();
    let wire = match kind {
        Some(TransportKind::Serial) => {
            let config = IoBounds::default();

            wire.with_config(config)
                .with_addressing(AddressingMode::Serial)
                .with_semantics(SendSemantics::Stream)
        }
        _ => wire.with_addressing(AddressingMode::Ip),
    };
    let wire = match kind {
        Some(kind) => wire.with_transport_kind(kind),
        None => wire,
    };
    (camera, wire)
}

#[test]
fn incompatible_standard_transport_fails_before_blocking_startup_or_io() {
    let (fake, transport) = wire(Some(TransportKind::Tcp));
    let profile = ProfileSpec::from_compile_time::<SonyFr7>().expect("Sony FR7 profile");

    let error = Session::open(transport, &SessionConfig::new(profile))
        .expect_err("Sony FR7 must reject standard TCP before blocking startup");

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: Some(grafton_visca::profiles::ProfileId::SonyFr7),
            transport: StandardTransport::Tcp,
            ..
        }
    ));
    assert_eq!(fake.config_reads(), 0);
    assert_eq!(fake.write_count(), 0);
    assert_eq!(fake.receive_calls(), 0);
}

#[test]
fn custom_transport_remains_an_explicit_unchecked_escape_hatch() {
    let (_camera, transport) = wire(None);
    let profile = ProfileSpec::from_compile_time::<SonyFr7>().expect("Sony FR7 profile");

    Session::open(transport, &SessionConfig::new(profile))
        .expect("custom transports remain allowed for advanced integrations")
        .close()
        .expect("closing an idle session should succeed");
}

#[test]
fn non_default_downstream_profile_projects_as_a_typed_camera() {
    let (fake, transport) = wire(None);
    let session = Session::open(
        transport,
        &SessionConfig::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("non-default profile config"),
    )
    .expect("session");

    let camera: Camera<NonDefaultCompileTimeProfile> = session
        .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("exact non-default profile projection");
    assert_eq!(
        camera.target(),
        grafton_visca::types::IndividualCameraId::CAMERA_1
    );
    assert_eq!(fake.write_count(), 0, "a projection writes nothing");
    session.shutdown().expect("shutdown");
}

#[test]
fn explicit_target_selection_checks_registration_before_admission_or_io() {
    let (fake, transport) = wire(Some(TransportKind::Serial));
    let mut config =
        SessionConfig::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>().expect("config");
    config = config
        .with_target(
            grafton_visca::types::IndividualCameraId::CAMERA_2,
            grafton_visca::profiles::ProfileSpec::from_compile_time::<
                grafton_visca::profiles::PtzOpticsG2,
            >()
            .expect("G2 profile"),
        )
        .expect("second target");
    let session = Session::open(transport, &config).expect("session");

    assert_eq!(
        session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1
            )
            .expect("explicit registered target")
            .target(),
        grafton_visca::types::IndividualCameraId::CAMERA_1
    );
    assert!(matches!(
        session.camera::<grafton_visca::profiles::PtzOpticsG2>(
            grafton_visca::types::IndividualCameraId::CAMERA_7
        ),
        Err(Error::InvalidRequest { .. })
    ));
    assert!(matches!(
        session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_2),
        Err(Error::InvalidRequest { .. })
    ));
    assert_eq!(
        fake.write_count(),
        0,
        "a rejected projection admits and writes nothing"
    );
    session.shutdown().expect("shutdown");
}
