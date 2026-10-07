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
    profile::ProfileSpec,
    profiles::{PtzOpticsG2, SonyFR7},
    CameraId, Error, SessionConfig,
};

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
use grafton_visca::{
    camera::TransportKind,
    transport::{AddressingMode, SendSemantics, TransportConfig},
    OperationalTuning,
};

fn raw_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("raw profile")
}

fn sony_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony profile")
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
fn spy_config(addressing: AddressingMode) -> TransportConfig {
    let mut config = TransportConfig::default();
    config.addressing = addressing;
    config
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

    assert_eq!(config.target_count(), 0);
    assert!(config.sole_target().is_none());
    assert!(config.profile(CameraId::CAMERA_1).is_none());

    config
        .register_target(CameraId::CAMERA_1, profile.clone())
        .expect("camera 1");
    assert_eq!(config.target_count(), 1);
    assert_eq!(config.sole_target(), Some(CameraId::CAMERA_1));
    assert!(config.profile(CameraId::CAMERA_1).is_some());

    let duplicate = config.register_target(CameraId::CAMERA_1, profile.clone());
    assert!(matches!(duplicate, Err(Error::InvalidRequest(_))));
    let conflicting = config.register_target(CameraId::CAMERA_1, sony_profile());
    assert!(matches!(conflicting, Err(Error::InvalidRequest(_))));
    let broadcast = config.register_target(CameraId::BROADCAST, profile.clone());
    assert!(matches!(broadcast, Err(Error::InvalidRequest(_))));

    for target in [
        CameraId::CAMERA_2,
        CameraId::CAMERA_3,
        CameraId::CAMERA_4,
        CameraId::CAMERA_5,
        CameraId::CAMERA_6,
        CameraId::CAMERA_7,
    ] {
        config
            .register_target(target, profile.clone())
            .expect("bounded target");
    }
    assert_eq!(config.target_count(), 7);
    assert!(config.sole_target().is_none());
    assert_eq!(
        config.targets().into_iter().flatten().collect::<Vec<_>>(),
        vec![
            CameraId::CAMERA_1,
            CameraId::CAMERA_2,
            CameraId::CAMERA_3,
            CameraId::CAMERA_4,
            CameraId::CAMERA_5,
            CameraId::CAMERA_6,
            CameraId::CAMERA_7,
        ]
    );
}

#[test]
fn root_and_blocking_session_config_are_the_same_type() {
    #[cfg(feature = "blocking")]
    fn assert_same_type(_: &SessionConfig, _: &grafton_visca::blocking::SessionConfig) {}

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

    use grafton_visca::{request, ControlClass, Request, RetryClass, TimeoutClass};

    use fake_camera::AsyncWire;

    #[cfg(feature = "runtime-tokio")]
    use grafton_visca::{Inquiry, InquiryRoute, ResponseDecoder};

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
        type Class = request::Plain;
        const MAX_SIZE: usize = 3;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Never;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
            out[..3].copy_from_slice(&[target.to_address_byte(), 0x01, 0xff]);
            Ok(3)
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[derive(Debug)]
    struct RawPowerInquiry;

    #[cfg(feature = "runtime-tokio")]
    impl Request for RawPowerInquiry {
        type Class = request::Inquiry;
        const MAX_SIZE: usize = 4;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
        const RETRY_CLASS: RetryClass = RetryClass::Never;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
            out[..4].copy_from_slice(&[target.to_address_byte(), 0x09, 0x04, 0xff]);
            Ok(4)
        }
    }

    #[cfg(feature = "runtime-tokio")]
    impl Inquiry for RawPowerInquiry {
        type Response = Vec<u8>;

        fn route(&self) -> InquiryRoute {
            InquiryRoute::RAW
        }

        fn decoder(&self) -> ResponseDecoder<Self::Response> {
            ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
        }
    }

    fn multi_config() -> SessionConfig {
        SessionConfig::new(raw_profile())
            .with_target(CameraId::CAMERA_2, raw_profile())
            .expect("multi-target config")
    }

    // The Tokio and smol serial-routing tests are separate: the Tokio one
    // also drives raw inquiries and joins with `tokio::join!`, while the smol
    // one covers commands only, joined with `futures_lite`.
    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_serial_registry_routes_interleaved_commands_and_global_inquiries() {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let session = grafton_visca::Session::open(
            transport,
            multi_config(),
            grafton_visca::TokioRuntime::from_current().expect("runtime"),
        )
        .await
        .expect("session");

        assert!(matches!(
            session.camera::<PtzOpticsG2>(),
            Err(Error::InvalidState(_))
        ));
        assert_eq!(
            session
                .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
                .unwrap()
                .target(),
            CameraId::CAMERA_1
        );
        assert_eq!(
            session
                .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
                .unwrap()
                .target(),
            CameraId::CAMERA_2
        );
        assert!(matches!(
            session.camera_for::<PtzOpticsG2>(CameraId::CAMERA_3),
            Err(Error::InvalidRequest(_))
        ));

        let camera_one = session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
            .expect("camera 1");
        let camera_two = session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
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
        assert_eq!(writes[0][0], CameraId::CAMERA_1.to_address_byte());
        assert_eq!(writes[1][0], CameraId::CAMERA_2.to_address_byte());
        assert_eq!(writes[2][0], CameraId::CAMERA_1.to_address_byte());
        assert_eq!(writes[3][0], CameraId::CAMERA_2.to_address_byte());
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_serial_registry_routes_interleaved_commands() {
        smol::block_on(async {
            let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
            let session = grafton_visca::Session::open(
                transport,
                multi_config(),
                grafton_visca::SmolRuntime::new(),
            )
            .await
            .expect("session");
            let camera_one = session
                .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
                .expect("camera 1");
            let camera_two = session
                .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
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

    async fn empty_registry_fails_before_transport_config_or_io<E: grafton_visca::Executor>(
        executor: E,
    ) {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let error = grafton_visca::Session::open(transport, SessionConfig::default(), executor)
            .await
            .expect_err("empty registry");
        assert!(matches!(error, Error::InvalidRequest(_)));
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

        let error = grafton_visca::Session::open(
            transport,
            config,
            grafton_visca::TokioRuntime::from_current().expect("runtime"),
        )
        .await
        .expect_err("maximum admission capacity must fail validation");

        assert!(matches!(error, Error::InvalidRequest(_)));
        // The rejection occurs before adapter construction, which would read
        // transport configuration and then create/spawn the owner actor.
        assert_eq!(fake.config_reads(), 0);
        assert_eq!(fake.write_count(), 0);
        assert_eq!(fake.receive_calls(), 0);
    }

    async fn unsupported_multi_target_topologies_do_not_write_or_read<
        E: grafton_visca::Executor,
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
                .with_target(CameraId::CAMERA_2, raw_profile())
                .expect("config");
            let error = grafton_visca::Session::open(transport, config, executor.clone()).await;
            assert!(matches!(error, Err(Error::NotSupported)));
            assert_eq!(fake.config_reads(), 0);
            assert_eq!(fake.write_count(), 0);
            assert_eq!(fake.receive_calls(), 0);
        }
    }

    async fn mixed_envelopes_and_custom_ip_fail_before_io<E: grafton_visca::Executor>(executor: E) {
        let cases = [
            (
                spy(AddressingMode::Serial, None),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, sony_profile())
                    .expect("mixed envelope config"),
            ),
            (
                spy(AddressingMode::Ip, None),
                SessionConfig::new(sony_profile())
                    .with_target(CameraId::CAMERA_2, sony_profile())
                    .expect("Sony multi-target config"),
            ),
            (
                spy(AddressingMode::Ip, Some(TransportKind::Custom)),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, raw_profile())
                    .expect("custom IP config"),
            ),
            (
                spy(AddressingMode::Ip, None),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, raw_profile())
                    .expect("unknown IP config"),
            ),
        ];
        for ((transport, fake), config) in cases {
            let error = grafton_visca::Session::open(transport, config, executor.clone()).await;
            assert!(matches!(error, Err(Error::NotSupported)));
            assert_eq!(fake.config_reads(), 0);
            assert_eq!(fake.write_count(), 0);
            assert_eq!(fake.receive_calls(), 0);
        }
    }

    async fn custom_serial_opt_in_reads_config_and_starts<E: grafton_visca::Executor>(executor: E) {
        let (transport, fake) =
            spy_with_hint(AddressingMode::Serial, None, Some(AddressingMode::Serial));
        let session = grafton_visca::Session::open(transport, multi_config(), executor)
            .await
            .expect("explicit custom serial opt-in");
        assert!(fake.config_reads() > 0);
        session.shutdown().expect("shutdown");
    }

    async fn strictest_profile_pacing_is_shared_by_all_targets<E: grafton_visca::Executor>(
        runtime: E,
    ) {
        let base = raw_profile();
        let strict = base.clone();
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let session = grafton_visca::Session::open(
            transport,
            SessionConfig::new(base)
                .with_target(CameraId::CAMERA_2, strict)
                .expect("strict multi-target config")
                .with_tuning(OperationalTuning::new().command_spacing(Duration::from_millis(120))),
            runtime,
        )
        .await
        .expect("session");
        session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
            .expect("camera 1")
            .execute(&Ping)
            .await
            .expect("first command");
        let started = std::time::Instant::now();
        session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
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

    impl grafton_visca::Request for Ping {
        type Class = grafton_visca::request::Plain;
        const MAX_SIZE: usize = 3;
        const TIMEOUT_CLASS: grafton_visca::TimeoutClass = grafton_visca::TimeoutClass::Quick;
        const RETRY_CLASS: grafton_visca::RetryClass = grafton_visca::RetryClass::Never;
        const CONTROL_CLASS: grafton_visca::ControlClass = grafton_visca::ControlClass::Normal;

        fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
            out[..3].copy_from_slice(&[target.to_address_byte(), 0x01, 0xff]);
            Ok(3)
        }
    }

    fn multi_config() -> SessionConfig {
        SessionConfig::new(raw_profile())
            .with_target(CameraId::CAMERA_2, raw_profile())
            .expect("multi-target config")
    }

    #[test]
    fn blocking_serial_registry_routes_each_target_and_keeps_shutdown_local() {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let session =
            grafton_visca::blocking::Session::open(transport, multi_config()).expect("session");
        assert!(matches!(
            session.camera::<PtzOpticsG2>(),
            Err(Error::InvalidState(_))
        ));
        session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
            .unwrap()
            .execute(&Ping)
            .unwrap();
        session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
            .unwrap()
            .execute(&Ping)
            .unwrap();
        session.shutdown().expect("shutdown");
        assert_eq!(fake.write_count(), 2);
        let writes = fake.writes();
        assert_eq!(writes[0][0], CameraId::CAMERA_2.to_address_byte());
        assert_eq!(writes[1][0], CameraId::CAMERA_1.to_address_byte());
    }

    #[test]
    fn blocking_topology_rejects_before_config_or_io() {
        for kind in [TransportKind::Tcp, TransportKind::Udp] {
            let (transport, fake) = spy(AddressingMode::Ip, Some(kind));
            let error = grafton_visca::blocking::Session::open(transport, multi_config())
                .expect_err("IP multi-target topology");
            assert!(matches!(error, Error::NotSupported));
            assert_eq!(fake.config_reads(), 0);
            assert_eq!(fake.write_count(), 0);
            assert_eq!(fake.receive_calls(), 0);
        }
    }

    #[test]
    fn blocking_empty_registry_fails_before_transport_config_or_io() {
        let (transport, fake) = spy(AddressingMode::Serial, Some(TransportKind::Serial));
        let error = grafton_visca::blocking::Session::open(transport, SessionConfig::default())
            .expect_err("empty registry");
        assert!(matches!(error, Error::InvalidRequest(_)));
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
                    .with_target(CameraId::CAMERA_2, sony_profile())
                    .expect("mixed envelope config"),
            ),
            (
                spy(AddressingMode::Ip, Some(TransportKind::Custom)),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, raw_profile())
                    .expect("custom IP config"),
            ),
            (
                spy(AddressingMode::Ip, None),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, raw_profile())
                    .expect("unknown IP config"),
            ),
        ];
        for ((transport, fake), config) in cases {
            let error = grafton_visca::blocking::Session::open(transport, config)
                .expect_err("unsupported topology");
            assert!(matches!(error, Error::NotSupported));
            assert_eq!(fake.config_reads(), 0);
            assert_eq!(fake.write_count(), 0);
            assert_eq!(fake.receive_calls(), 0);
        }
    }

    #[test]
    fn blocking_custom_serial_opt_in_reads_config_and_starts() {
        let (transport, fake) =
            spy_with_hint(AddressingMode::Serial, None, Some(AddressingMode::Serial));
        let session = grafton_visca::blocking::Session::open(transport, multi_config())
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
            SessionConfig::new(base)
                .with_target(CameraId::CAMERA_2, strict)
                .expect("strict multi-target config")
                .with_tuning(OperationalTuning::new().command_spacing(Duration::from_millis(120))),
        )
        .expect("session");
        session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
            .expect("camera 1")
            .execute(&Ping)
            .expect("first command");
        let started = std::time::Instant::now();
        session
            .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
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
            SessionConfig::new(raw_profile()),
        )
        .expect("blocking session");
        let async_session = grafton_visca::Session::open(
            FakeCamera::acking(1).async_wire(),
            SessionConfig::new(raw_profile()),
            grafton_visca::TokioRuntime::from_current().expect("runtime"),
        )
        .await
        .expect("async session");

        blocking
            .camera::<PtzOpticsG2>()
            .unwrap()
            .zoom()
            .stop()
            .unwrap()
            .applied()
            .unwrap();
        async_session
            .camera::<PtzOpticsG2>()
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
            .camera::<PtzOpticsG2>()
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
