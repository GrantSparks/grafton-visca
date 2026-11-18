//! Shared-session registry acceptance coverage.
//!
//! The registry is deliberately exercised through the public root and facade
//! APIs. The cameras below never perform socket or serial I/O; they only
//! provide deterministic VISCA frames when a request is admitted, and expose
//! the owner's write, receive and transport-configuration read counts.

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
use std::time::Duration;

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
use grafton_visca_test_support::fake_camera;

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
use fake_camera::FakeCamera;

use grafton_visca::{
    config::SessionConfig,
    profiles::{ProfileSpec, PtzOpticsG2, SonyFr7},
    types::IndividualCameraId,
    Error,
};

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
use grafton_visca::{
    config::{OperationalTuning, TransportKind},
    transport::{AddressingMode, IoBounds, SendSemantics},
};

fn raw_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("raw profile")
}

fn sony_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<SonyFr7>().expect("Sony profile")
}

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
/// A camera that answers a raw inquiry with data and any other write with an
/// ACK and a completion. A serial camera addresses its replies to the writing
/// target; otherwise they come from source `0x90`.
fn spy_camera(addressing: AddressingMode) -> FakeCamera {
    FakeCamera::new(move |write, answer| {
        let source = if addressing == AddressingMode::Serial {
            let id = write.first().copied().unwrap_or(0x81) & 0x0f;
            0x80 | (id.saturating_add(8) << 4)
        } else {
            0x90
        };
        if write.get(1) == Some(&0x09) {
            answer.reply(vec![source, 0x50, 0x01, 0x02, 0xff]);
        } else {
            answer
                .reply(vec![source, 0x41, 0xff])
                .reply(vec![source, 0x51, 0xff]);
        }
    })
}

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
/// The transport configuration of a spy with `addressing`.
fn spy_config(_addressing: AddressingMode) -> IoBounds {
    IoBounds::default()
}

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
/// A serial spy sends as a stream; otherwise as datagrams.
fn spy_semantics(addressing: AddressingMode) -> SendSemantics {
    if addressing == AddressingMode::Serial {
        SendSemantics::Stream
    } else {
        SendSemantics::Datagram
    }
}

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
/// The addressing hint a spy reports by default: only a standard serial
/// transport names its addressing.
fn spy_hint(
    addressing: AddressingMode,
    standard_kind: Option<TransportKind>,
) -> Option<AddressingMode> {
    (standard_kind == Some(TransportKind::Serial)).then_some(addressing)
}

#[test]
fn registry_bounds_duplicate_broadcast_and_sole_target_semantics() {
    let profile = raw_profile();
    let mut config = SessionConfig::default();
    assert_eq!(config.targets().count(), 0);
    assert!(config.sole_target().is_none());
    assert!(config
        .profile(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .is_none());

    config = config
        .with_target(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
            profile.clone(),
        )
        .expect("camera 1");
    assert_eq!(config.targets().count(), 1);
    assert_eq!(config.sole_target(), Some(IndividualCameraId::CAMERA_1));
    assert!(config
        .profile(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .is_some());

    let duplicate = config.clone().with_target(
        grafton_visca::types::IndividualCameraId::CAMERA_1,
        profile.clone(),
    );
    assert!(matches!(duplicate, Err(Error::InvalidRequest { .. })));
    let conflicting = config.clone().with_target(
        grafton_visca::types::IndividualCameraId::CAMERA_1,
        sony_profile(),
    );
    assert!(matches!(conflicting, Err(Error::InvalidRequest { .. })));

    for target in [
        IndividualCameraId::CAMERA_2,
        IndividualCameraId::CAMERA_3,
        IndividualCameraId::CAMERA_4,
        IndividualCameraId::CAMERA_5,
        IndividualCameraId::CAMERA_6,
        IndividualCameraId::CAMERA_7,
    ] {
        config = config
            .with_target(target, profile.clone())
            .expect("bounded target");
    }
    assert_eq!(config.targets().count(), 7);
    assert!(config.sole_target().is_none());
    assert_eq!(
        config.targets().collect::<Vec<_>>(),
        vec![
            IndividualCameraId::CAMERA_1,
            IndividualCameraId::CAMERA_2,
            IndividualCameraId::CAMERA_3,
            IndividualCameraId::CAMERA_4,
            IndividualCameraId::CAMERA_5,
            IndividualCameraId::CAMERA_6,
            IndividualCameraId::CAMERA_7,
        ]
    );
}

#[test]
fn root_and_blocking_session_config_are_the_same_type() {
    #[cfg(feature = "blocking")]
    fn assert_same_type(_: &SessionConfig, _: &grafton_visca::config::SessionConfig) {}

    #[cfg(feature = "blocking")]
    assert_same_type(
        &SessionConfig::new(raw_profile()),
        &SessionConfig::new(raw_profile()),
    );
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_registry {
    use super::*;
    use grafton_visca_test_support::runtime_matrix;

    #[cfg(feature = "runtime-tokio")]
    use std::num::NonZeroUsize;

    use grafton_visca::request::{self, Request};

    use fake_camera::AsyncWire;

    #[cfg(feature = "runtime-tokio")]
    use grafton_visca::request::{Inquiry, ResponseDecoder};

    /// A spy camera and its wire: the shared wire with the spy's addressing
    /// and, when `standard_kind` is a standard transport, its kind.
    fn spy(
        addressing: AddressingMode,
        standard_kind: Option<TransportKind>,
    ) -> (AsyncWire, FakeCamera) {
        spy_with_hint(
            addressing,
            standard_kind,
            spy_hint(addressing, standard_kind),
        )
    }

    fn spy_with_hint(
        addressing: AddressingMode,
        standard_kind: Option<TransportKind>,
        addressing_hint: Option<AddressingMode>,
    ) -> (AsyncWire, FakeCamera) {
        let camera = spy_camera(addressing);
        let mut wire = camera
            .async_wire()
            .with_config(spy_config(addressing))
            .with_semantics(spy_semantics(addressing));
        if let Some(hint) = addressing_hint {
            wire = wire.with_addressing(hint);
        }
        if let Some(kind) = standard_kind {
            wire = wire.with_transport_kind(kind);
        }
        (wire, camera)
    }

    #[derive(Debug)]
    struct Ping;

    impl Request for Ping {
        type Class = request::PlainClass;
        const MAX_SIZE: usize = 3;
        fn policy(&self) -> grafton_visca::request::CommandPolicy {
            grafton_visca::request::CommandPolicy::new(
                grafton_visca::profiles::CommandTimeoutClass::Quick,
                grafton_visca::request::CommandRetryClass::Never,
                grafton_visca::request::SubmissionClass::Normal,
            )
        }

        fn write_into(
            &self,
            target: grafton_visca::types::CameraId,
            out: &mut [u8],
        ) -> Result<usize, Error> {
            out[..3].copy_from_slice(&[(0x80 | target.value()), 0x01, 0xff]);
            Ok(3)
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct RawPowerInquiry;

    #[cfg(feature = "runtime-tokio")]
    impl Request for RawPowerInquiry {
        type Class = request::InquiryClass;
        const MAX_SIZE: usize = 4;
        fn policy(&self) -> grafton_visca::request::InquiryPolicy {
            grafton_visca::request::InquiryPolicy::new(
                grafton_visca::request::InquiryRetryClass::Never,
                grafton_visca::request::SubmissionClass::Normal,
            )
        }

        fn write_into(
            &self,
            target: grafton_visca::types::CameraId,
            out: &mut [u8],
        ) -> Result<usize, Error> {
            out[..4].copy_from_slice(&[(0x80 | target.value()), 0x09, 0x04, 0xff]);
            Ok(4)
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl Inquiry for RawPowerInquiry {
        type Response = Vec<u8>;

        fn decoder(&self) -> ResponseDecoder<Self::Response> {
            ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
        }
    }

    fn multi_config() -> SessionConfig {
        SessionConfig::new(raw_profile())
            .with_target(
                grafton_visca::types::IndividualCameraId::CAMERA_2,
                raw_profile(),
            )
            .expect("multi-target config")
    }

    // The Tokio and smol serial-routing tests are separate: the Tokio one
    // also drives raw inquiries and joins with `tokio::join!`, while the smol
    // one covers commands only, joined with `futures_lite`.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_serial_registry_routes_interleaved_commands_and_global_inquiries() {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let session = grafton_visca::r#async::Session::open(
            transport,
            &multi_config(),
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        )
        .await
        .expect("session");

        assert_eq!(
            session
                .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("explicit registered target")
                .target(),
            grafton_visca::types::IndividualCameraId::CAMERA_1
        );
        assert_eq!(
            session
                .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .unwrap()
                .target(),
            grafton_visca::types::IndividualCameraId::CAMERA_1
        );
        assert_eq!(
            session
                .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_2)
                .unwrap()
                .target(),
            grafton_visca::types::IndividualCameraId::CAMERA_2
        );
        assert!(matches!(
            session.camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_3),
            Err(Error::InvalidRequest { .. })
        ));

        let camera_one = session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera 1");
        let camera_two = session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_2)
            .expect("camera 2");
        let (one, two) = tokio::join!(camera_one.execute(&Ping), camera_two.execute(&Ping));
        one.expect("camera 1 command");
        two.expect("camera 2 command");

        let (first, second) = tokio::join!(
            camera_one.inquire(&RawPowerInquiry),
            camera_two.inquire(&RawPowerInquiry)
        );
        assert_eq!(first.expect("camera 1 inquiry"), vec![1, 2]);
        assert_eq!(second.expect("camera 2 inquiry"), vec![1, 2]);

        session.shutdown().expect("shutdown");
        assert_eq!(fake.write_count(), 4);
        let writes = fake.writes();
        assert_eq!(writes[0][0], (0x80 | IndividualCameraId::CAMERA_1.value()));
        assert_eq!(writes[1][0], (0x80 | IndividualCameraId::CAMERA_2.value()));
        assert_eq!(writes[2][0], (0x80 | IndividualCameraId::CAMERA_1.value()));
        assert_eq!(writes[3][0], (0x80 | IndividualCameraId::CAMERA_2.value()));
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_serial_registry_routes_interleaved_commands() {
        smol::block_on(async {
            let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
            let session = grafton_visca::r#async::Session::open(
                transport,
                &multi_config(),
                grafton_visca::runtime::SmolRuntime::new(),
            )
            .await
            .expect("session");
            let camera_one = session
                .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("camera 1");
            let camera_two = session
                .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_2)
                .expect("camera 2");
            let (one, two) =
                futures_lite::future::zip(camera_one.execute(&Ping), camera_two.execute(&Ping))
                    .await;
            one.expect("camera 1 command");
            two.expect("camera 2 command");
            session.shutdown().expect("shutdown");
            assert_eq!(fake.write_count(), 2);
        });
    }

    async fn empty_registry_fails_before_transport_config_or_io<
        E: grafton_visca::runtime::Executor,
    >(
        executor: E,
    ) {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let error =
            grafton_visca::r#async::Session::open(transport, &SessionConfig::default(), executor)
                .await
                .expect_err("empty registry");
        assert!(matches!(error, Error::InvalidRequest { .. }));
        assert_eq!(fake.config_reads(), 0);
        assert_eq!(fake.write_count(), 0);
        assert_eq!(fake.receive_calls(), 0);
    }

    // A single-runtime scenario: it runs under Tokio only.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_maximum_admission_capacity_fails_before_owner_construction() {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let config = SessionConfig::new(raw_profile()).with_admission_capacity(
            NonZeroUsize::new(usize::MAX).expect("usize::MAX is non-zero"),
        );

        let error = grafton_visca::r#async::Session::open(
            transport,
            &config,
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        )
        .await
        .expect_err("maximum admission capacity must fail validation");

        assert!(matches!(error, Error::InvalidRequest { .. }));
        // The rejection occurs before adapter construction, which would read
        // transport configuration and then create/spawn the owner actor.
        assert_eq!(fake.config_reads(), 0);
        assert_eq!(fake.write_count(), 0);
        assert_eq!(fake.receive_calls(), 0);
    }

    async fn multi_target_connect_rejects_before_dns_or_socket<
        E: grafton_visca::runtime::Runtime,
    >(
        executor: E,
    ) {
        use grafton_visca::config::TransportOptions;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("local listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let address = listener.local_addr().expect("listener address").to_string();
        for transport in [
            TransportOptions::tcp(&address),
            TransportOptions::udp(&address),
            TransportOptions::tcp("does-not-resolve.invalid"),
            TransportOptions::udp("does-not-resolve.invalid"),
        ] {
            let error = grafton_visca::r#async::Session::connect(
                &multi_config().with_transport(transport),
                executor.clone(),
            )
            .await
            .expect_err("multi-target IP preflight");
            assert!(matches!(
                error,
                Error::FeatureNotSupported {
                    feature: "multiple VISCA targets over TCP or UDP",
                    ..
                }
            ));
        }
        assert_eq!(
            listener.accept().expect_err("no connection opened").kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    async fn unsupported_multi_target_topologies_do_not_write_or_read<
        E: grafton_visca::runtime::Executor,
    >(
        executor: E,
    ) {
        let cases = [
            (TransportKind::Tcp, AddressingMode::Ip, raw_profile()),
            (TransportKind::Udp, AddressingMode::Ip, raw_profile()),
        ];
        for (kind, addressing, profile) in cases {
            let (transport, fake) = spy(addressing, Some(kind));
            let config = SessionConfig::new(profile)
                .with_target(
                    grafton_visca::types::IndividualCameraId::CAMERA_2,
                    raw_profile(),
                )
                .expect("config");
            let error =
                grafton_visca::r#async::Session::open(transport, &config, executor.clone()).await;
            assert!(matches!(
                error,
                Err(Error::FeatureNotSupported {
                    feature: "multiple VISCA targets over TCP or UDP",
                    ..
                })
            ));
            assert_eq!(fake.config_reads(), 0);
            assert_eq!(fake.write_count(), 0);
            assert_eq!(fake.receive_calls(), 0);
        }
    }

    async fn mixed_envelopes_and_custom_ip_fail_before_io<E: grafton_visca::runtime::Executor>(
        executor: E,
    ) {
        let cases = [
            (
                spy(AddressingMode::Serial, None),
                SessionConfig::new(raw_profile())
                    .with_target(
                        grafton_visca::types::IndividualCameraId::CAMERA_2,
                        sony_profile(),
                    )
                    .expect("mixed envelope config"),
            ),
            (
                spy(AddressingMode::Ip, None),
                SessionConfig::new(sony_profile())
                    .with_target(
                        grafton_visca::types::IndividualCameraId::CAMERA_2,
                        sony_profile(),
                    )
                    .expect("Sony multi-target config"),
            ),
            (
                spy(AddressingMode::Ip, Some(TransportKind::Custom)),
                SessionConfig::new(raw_profile())
                    .with_target(
                        grafton_visca::types::IndividualCameraId::CAMERA_2,
                        raw_profile(),
                    )
                    .expect("custom IP config"),
            ),
            (
                spy(AddressingMode::Ip, None),
                SessionConfig::new(raw_profile())
                    .with_target(
                        grafton_visca::types::IndividualCameraId::CAMERA_2,
                        raw_profile(),
                    )
                    .expect("unknown IP config"),
            ),
        ];
        for (((transport, fake), config), expected_feature) in cases.into_iter().zip([
            "multiple VISCA targets with Sony encapsulation",
            "multiple VISCA targets with Sony encapsulation",
            "multiple VISCA targets without serial addressing",
            "multiple VISCA targets without serial addressing",
        ]) {
            let error =
                grafton_visca::r#async::Session::open(transport, &config, executor.clone()).await;
            assert!(
                matches!(error, Err(Error::FeatureNotSupported { feature, .. }) if feature == expected_feature)
            );
            assert_eq!(fake.config_reads(), 0);
            assert_eq!(fake.write_count(), 0);
            assert_eq!(fake.receive_calls(), 0);
        }
    }

    async fn custom_serial_opt_in_reads_config_and_starts<E: grafton_visca::runtime::Executor>(
        executor: E,
    ) {
        let (transport, fake) =
            spy_with_hint(AddressingMode::Serial, None, Some(AddressingMode::Serial));
        let session = grafton_visca::r#async::Session::open(transport, &multi_config(), executor)
            .await
            .expect("explicit custom serial opt-in");
        assert!(fake.config_reads() > 0);
        session.shutdown().expect("shutdown");
    }

    async fn strictest_profile_pacing_is_shared_by_all_targets<
        E: grafton_visca::runtime::Executor,
    >(
        runtime: E,
    ) {
        let base = raw_profile();
        let strict = base.clone();
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let session = grafton_visca::r#async::Session::open(
            transport,
            &SessionConfig::new(base)
                .with_target(grafton_visca::types::IndividualCameraId::CAMERA_2, strict)
                .expect("strict multi-target config")
                .with_tuning(
                    OperationalTuning::new().with_command_spacing(Duration::from_millis(120)),
                ),
            runtime,
        )
        .await
        .expect("session");
        session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera 1")
            .execute(&Ping)
            .await
            .expect("first command");
        let started = std::time::Instant::now();
        session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_2)
            .expect("camera 2")
            .execute(&Ping)
            .await
            .expect("second command");
        assert!(started.elapsed() >= Duration::from_millis(110));
        assert_eq!(fake.write_count(), 2);
        session.shutdown().expect("shutdown");
    }

    runtime_matrix!(
        empty_registry_fails_before_transport_config_or_io,
        custom_serial_opt_in_reads_config_and_starts,
        unsupported_multi_target_topologies_do_not_write_or_read,
        multi_target_connect_rejects_before_dns_or_socket,
        mixed_envelopes_and_custom_ip_fail_before_io,
        strictest_profile_pacing_is_shared_by_all_targets,
    );
}

#[cfg(feature = "blocking")]
mod blocking_registry {
    use super::*;

    use fake_camera::BlockingWire;

    /// A spy camera and its wire: the shared wire with the spy's addressing
    /// and, when `standard_kind` is a standard transport, its kind.
    fn spy(
        addressing: AddressingMode,
        standard_kind: Option<TransportKind>,
    ) -> (BlockingWire, FakeCamera) {
        spy_with_hint(
            addressing,
            standard_kind,
            spy_hint(addressing, standard_kind),
        )
    }

    fn spy_with_hint(
        addressing: AddressingMode,
        standard_kind: Option<TransportKind>,
        addressing_hint: Option<AddressingMode>,
    ) -> (BlockingWire, FakeCamera) {
        let camera = spy_camera(addressing);
        let mut wire = camera
            .blocking_wire()
            .with_config(spy_config(addressing))
            .with_semantics(spy_semantics(addressing));
        if let Some(hint) = addressing_hint {
            wire = wire.with_addressing(hint);
        }
        if let Some(kind) = standard_kind {
            wire = wire.with_transport_kind(kind);
        }
        (wire, camera)
    }

    #[derive(Debug)]
    struct Ping;

    impl grafton_visca::request::Request for Ping {
        type Class = grafton_visca::request::PlainClass;
        const MAX_SIZE: usize = 3;
        fn policy(&self) -> grafton_visca::request::CommandPolicy {
            grafton_visca::request::CommandPolicy::new(
                grafton_visca::profiles::CommandTimeoutClass::Quick,
                grafton_visca::request::CommandRetryClass::Never,
                grafton_visca::request::SubmissionClass::Normal,
            )
        }

        fn write_into(
            &self,
            target: grafton_visca::types::CameraId,
            out: &mut [u8],
        ) -> Result<usize, Error> {
            out[..3].copy_from_slice(&[(0x80 | target.value()), 0x01, 0xff]);
            Ok(3)
        }
    }

    fn multi_config() -> SessionConfig {
        SessionConfig::new(raw_profile())
            .with_target(
                grafton_visca::types::IndividualCameraId::CAMERA_2,
                raw_profile(),
            )
            .expect("multi-target config")
    }

    #[test]
    fn blocking_serial_registry_routes_each_target_and_keeps_shutdown_local() {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let session =
            grafton_visca::blocking::Session::open(transport, &multi_config()).expect("session");
        assert_eq!(
            session
                .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("explicit registered target")
                .target(),
            grafton_visca::types::IndividualCameraId::CAMERA_1
        );
        session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_2)
            .unwrap()
            .execute(&Ping)
            .unwrap();
        session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .unwrap()
            .execute(&Ping)
            .unwrap();
        session.shutdown().expect("shutdown");
        assert_eq!(fake.write_count(), 2);
        let writes = fake.writes();
        assert_eq!(writes[0][0], (0x80 | IndividualCameraId::CAMERA_2.value()));
        assert_eq!(writes[1][0], (0x80 | IndividualCameraId::CAMERA_1.value()));
    }

    #[test]
    fn blocking_multi_target_connect_rejects_before_dns_or_socket() {
        use grafton_visca::config::TransportOptions;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("local listener");
        listener
            .set_nonblocking(true)
            .expect("nonblocking listener");
        let address = listener.local_addr().expect("listener address").to_string();
        for transport in [
            TransportOptions::tcp(&address),
            TransportOptions::udp(&address),
            TransportOptions::tcp("does-not-resolve.invalid"),
            TransportOptions::udp("does-not-resolve.invalid"),
        ] {
            let error = grafton_visca::blocking::Session::connect(
                &multi_config().with_transport(transport),
            )
            .expect_err("multi-target IP preflight");
            assert!(matches!(
                error,
                Error::FeatureNotSupported {
                    feature: "multiple VISCA targets over TCP or UDP",
                    ..
                }
            ));
        }
        assert_eq!(
            listener.accept().expect_err("no connection opened").kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn blocking_topology_rejects_before_config_or_io() {
        for kind in [TransportKind::Tcp, TransportKind::Udp] {
            let (transport, fake) = spy(AddressingMode::Ip, Some(kind));
            let error = grafton_visca::blocking::Session::open(transport, &multi_config())
                .expect_err("IP multi-target topology");
            assert!(matches!(
                error,
                Error::FeatureNotSupported {
                    feature: "multiple VISCA targets over TCP or UDP",
                    ..
                }
            ));
            assert_eq!(fake.config_reads(), 0);
            assert_eq!(fake.write_count(), 0);
            assert_eq!(fake.receive_calls(), 0);
        }
    }

    #[test]
    fn blocking_empty_registry_fails_before_transport_config_or_io() {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let error = grafton_visca::blocking::Session::open(transport, &SessionConfig::default())
            .expect_err("empty registry");
        assert!(matches!(error, Error::InvalidRequest { .. }));
        assert_eq!(fake.config_reads(), 0);
        assert_eq!(fake.write_count(), 0);
        assert_eq!(fake.receive_calls(), 0);
    }

    #[test]
    fn blocking_mixed_envelope_and_custom_ip_fail_before_io() {
        let cases = [
            (
                spy(AddressingMode::Serial, None),
                SessionConfig::new(raw_profile())
                    .with_target(
                        grafton_visca::types::IndividualCameraId::CAMERA_2,
                        sony_profile(),
                    )
                    .expect("mixed envelope config"),
            ),
            (
                spy(AddressingMode::Ip, Some(TransportKind::Custom)),
                SessionConfig::new(raw_profile())
                    .with_target(
                        grafton_visca::types::IndividualCameraId::CAMERA_2,
                        raw_profile(),
                    )
                    .expect("custom IP config"),
            ),
            (
                spy(AddressingMode::Ip, None),
                SessionConfig::new(raw_profile())
                    .with_target(
                        grafton_visca::types::IndividualCameraId::CAMERA_2,
                        raw_profile(),
                    )
                    .expect("unknown IP config"),
            ),
        ];
        for (((transport, fake), config), expected_feature) in cases.into_iter().zip([
            "multiple VISCA targets with Sony encapsulation",
            "multiple VISCA targets without serial addressing",
            "multiple VISCA targets without serial addressing",
        ]) {
            let error = grafton_visca::blocking::Session::open(transport, &config)
                .expect_err("unsupported topology");
            assert!(
                matches!(error, Error::FeatureNotSupported { feature, .. } if feature == expected_feature)
            );
            assert_eq!(fake.config_reads(), 0);
            assert_eq!(fake.write_count(), 0);
            assert_eq!(fake.receive_calls(), 0);
        }
    }

    #[test]
    fn blocking_custom_serial_opt_in_reads_config_and_starts() {
        let (transport, fake) =
            spy_with_hint(AddressingMode::Serial, None, Some(AddressingMode::Serial));
        let session = grafton_visca::blocking::Session::open(transport, &multi_config())
            .expect("explicit custom serial opt-in");
        assert!(fake.config_reads() > 0);
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_strictest_profile_pacing_is_shared_by_all_targets() {
        let base = raw_profile();
        let strict = base.clone();
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let session = grafton_visca::blocking::Session::open(
            transport,
            &SessionConfig::new(base)
                .with_target(grafton_visca::types::IndividualCameraId::CAMERA_2, strict)
                .expect("strict multi-target config")
                .with_tuning(
                    OperationalTuning::new().with_command_spacing(Duration::from_millis(120)),
                ),
        )
        .expect("session");
        session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera 1")
            .execute(&Ping)
            .expect("first command");
        let started = std::time::Instant::now();
        session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_2)
            .expect("camera 2")
            .execute(&Ping)
            .expect("second command");
        assert!(started.elapsed() >= Duration::from_millis(110));
        assert_eq!(fake.write_count(), 2);
        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-tokio"))]
mod coexistence {
    use super::*;

    use grafton_visca::blocking::Session as BlockingSession;

    #[tokio::test]
    async fn blocking_and_async_sessions_have_one_owner_each_and_isolated_shutdown() {
        let blocking = BlockingSession::open(
            FakeCamera::acking(1).blocking_wire(),
            &SessionConfig::new(raw_profile()),
        )
        .expect("blocking session");
        let async_session = grafton_visca::r#async::Session::open(
            FakeCamera::acking(1).async_wire(),
            &SessionConfig::new(raw_profile()),
            grafton_visca::runtime::TokioRuntime::from_current().expect("runtime"),
        )
        .await
        .expect("async session");

        blocking
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .unwrap()
            .zoom()
            .stop()
            .unwrap()
            .applied()
            .unwrap();
        async_session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .unwrap()
            .zoom()
            .stop()
            .await
            .unwrap()
            .applied()
            .await
            .unwrap();
        blocking.shutdown().expect("blocking shutdown");
        async_session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .unwrap()
            .zoom()
            .stop()
            .await
            .unwrap()
            .applied()
            .await
            .unwrap();
        async_session.shutdown().expect("async shutdown");
    }
}
