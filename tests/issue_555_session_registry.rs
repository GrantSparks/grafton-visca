//! Shared-session registry acceptance coverage.
//!
//! The registry is deliberately exercised through the public root and facade
//! APIs.  The transport spies below never perform socket or serial I/O; they
//! only provide deterministic VISCA frames when a request is admitted.

#[cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
use std::time::Duration;

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
use grafton_visca::OperationalTuning;

fn raw_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("raw profile")
}

fn sony_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony profile")
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
    use std::{
        future::Future,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    #[cfg(feature = "runtime-tokio")]
    use std::num::NonZeroUsize;

    use grafton_visca::{
        camera::TransportKind,
        request,
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        ControlClass, Request, RetryClass, TimeoutClass,
    };

    #[cfg(feature = "runtime-tokio")]
    use grafton_visca::{Inquiry, InquiryRoute, ResponseDecoder};

    #[derive(Debug, Clone)]
    struct SpyCounts {
        config_reads: Arc<AtomicUsize>,
        writes: Arc<AtomicUsize>,
        reads: Arc<AtomicUsize>,
    }

    impl SpyCounts {
        fn new() -> Self {
            Self {
                config_reads: Arc::new(AtomicUsize::new(0)),
                writes: Arc::new(AtomicUsize::new(0)),
                reads: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[derive(Debug)]
    struct AsyncSpy {
        config: TransportConfig,
        standard_kind: Option<TransportKind>,
        addressing_hint: Option<AddressingMode>,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
        counts: SpyCounts,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl AsyncSpy {
        fn new(
            addressing: AddressingMode,
            standard_kind: Option<TransportKind>,
        ) -> (Self, SpyCounts) {
            Self::new_with_hint(
                addressing,
                standard_kind,
                (standard_kind == Some(TransportKind::Serial)).then_some(addressing),
            )
        }

        fn new_with_hint(
            addressing: AddressingMode,
            standard_kind: Option<TransportKind>,
            addressing_hint: Option<AddressingMode>,
        ) -> (Self, SpyCounts) {
            let (response_tx, responses) = flume::unbounded();
            let counts = SpyCounts::new();
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig {
                        addressing,
                        ..TransportConfig::default()
                    },
                    standard_kind,
                    addressing_hint,
                    responses,
                    response_tx,
                    counts: counts.clone(),
                    writes,
                },
                counts,
            )
        }

        fn source_for(bytes: &[u8], addressing: AddressingMode) -> u8 {
            if addressing == AddressingMode::Serial {
                let id = bytes.first().copied().unwrap_or(0x81) & 0x0f;
                0x80 | (id.saturating_add(8) << 4)
            } else {
                0x90
            }
        }
    }

    impl HasTransportConfig for AsyncSpy {
        fn transport_config(&self) -> &TransportConfig {
            self.counts.config_reads.fetch_add(1, Ordering::SeqCst);
            &self.config
        }

        fn standard_transport_kind(&self) -> Option<TransportKind> {
            self.standard_kind
        }
    }

    impl AsyncTransport for AsyncSpy {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let bytes = bytes.to_vec();
            self.counts.writes.fetch_add(1, Ordering::SeqCst);
            self.writes.lock().expect("writes lock").push(bytes.clone());
            let tx = self.response_tx.clone();
            let source = Self::source_for(&bytes, self.config.addressing);
            let inquiry = bytes.get(1) == Some(&0x09);
            async move {
                if inquiry {
                    tx.send_async(vec![source, 0x50, 0x01, 0x02, 0xff])
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                } else {
                    tx.send_async(vec![source, 0x41, 0xff])
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                    tx.send_async(vec![source, 0x51, 0xff])
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                }
                Ok(())
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn recv_into<'a>(
            &'a mut self,
            dst: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            self.counts.reads.fetch_add(1, Ordering::SeqCst);
            async move {
                let response = self
                    .responses
                    .recv_async()
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                dst[..response.len()].copy_from_slice(&response);
                Ok(response.len())
            }
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            self.addressing_hint
        }

        fn send_semantics(&self) -> SendSemantics {
            if self.config.addressing == AddressingMode::Serial {
                SendSemantics::Stream
            } else {
                SendSemantics::Datagram
            }
        }
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

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_serial_registry_routes_interleaved_commands_and_global_inquiries() {
        let (transport, counts) =
            AsyncSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
        let writes = Arc::clone(&transport.writes);
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

        session.shutdown().await.expect("shutdown");
        assert_eq!(counts.writes.load(Ordering::SeqCst), 4);
        let writes = writes.lock().expect("writes lock");
        assert_eq!(writes[0][0], CameraId::CAMERA_1.to_address_byte());
        assert_eq!(writes[1][0], CameraId::CAMERA_2.to_address_byte());
        assert_eq!(writes[2][0], CameraId::CAMERA_1.to_address_byte());
        assert_eq!(writes[3][0], CameraId::CAMERA_2.to_address_byte());
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_empty_registry_fails_before_transport_config_or_io() {
        let (transport, counts) =
            AsyncSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
        let error = grafton_visca::Session::open(
            transport,
            SessionConfig::default(),
            grafton_visca::TokioRuntime::from_current().expect("runtime"),
        )
        .await
        .expect_err("empty registry");
        assert!(matches!(error, Error::InvalidRequest(_)));
        assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
        assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
        assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_maximum_admission_capacity_fails_before_owner_construction() {
        let (transport, counts) =
            AsyncSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
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
        assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
        assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
        assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_empty_registry_fails_before_transport_config_or_io() {
        smol::block_on(async {
            let (transport, counts) =
                AsyncSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
            let error = grafton_visca::Session::open(
                transport,
                SessionConfig::default(),
                grafton_visca::SmolRuntime::new(),
            )
            .await
            .expect_err("empty registry");
            assert!(matches!(error, Error::InvalidRequest(_)));
            assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
            assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
            assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
        });
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_serial_registry_routes_interleaved_commands() {
        smol::block_on(async {
            let (transport, counts) =
                AsyncSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
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
            session.shutdown().await.expect("shutdown");
            assert_eq!(counts.writes.load(Ordering::SeqCst), 2);
        });
    }

    async fn unsupported_multi_target_topologies_do_not_write_or_read<E, F>(runtime: F)
    where
        E: grafton_visca::Executor,
        F: Fn() -> E,
    {
        let cases = [
            (TransportKind::Tcp, AddressingMode::Ip, raw_profile()),
            (TransportKind::Udp, AddressingMode::Ip, raw_profile()),
        ];
        for (kind, addressing, profile) in cases {
            let (transport, counts) = AsyncSpy::new(addressing, Some(kind));
            let config = SessionConfig::new(profile)
                .with_target(CameraId::CAMERA_2, raw_profile())
                .expect("config");
            let error = grafton_visca::Session::open(transport, config, runtime()).await;
            assert!(matches!(error, Err(Error::NotSupported)));
            assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
            assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
            assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
        }
    }

    async fn unsupported_envelopes_and_custom_ip_do_not_write_or_read<E, F>(runtime: F)
    where
        E: grafton_visca::Executor,
        F: Fn() -> E,
    {
        let cases = [
            (
                AsyncSpy::new(AddressingMode::Serial, None),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, sony_profile())
                    .expect("mixed envelope config"),
            ),
            (
                AsyncSpy::new(AddressingMode::Ip, None),
                SessionConfig::new(sony_profile())
                    .with_target(CameraId::CAMERA_2, sony_profile())
                    .expect("Sony multi-target config"),
            ),
            (
                AsyncSpy::new(AddressingMode::Ip, Some(TransportKind::Custom)),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, raw_profile())
                    .expect("custom IP config"),
            ),
            (
                AsyncSpy::new(AddressingMode::Ip, None),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, raw_profile())
                    .expect("unknown IP config"),
            ),
        ];
        for ((transport, counts), config) in cases {
            let error = grafton_visca::Session::open(transport, config, runtime()).await;
            assert!(matches!(error, Err(Error::NotSupported)));
            assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
            assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
            assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
        }
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_custom_serial_opt_in_reads_config_and_starts() {
        let (transport, counts) =
            AsyncSpy::new_with_hint(AddressingMode::Serial, None, Some(AddressingMode::Serial));
        let session = grafton_visca::Session::open(
            transport,
            multi_config(),
            grafton_visca::TokioRuntime::from_current().expect("runtime"),
        )
        .await
        .expect("explicit custom serial opt-in");
        assert!(counts.config_reads.load(Ordering::SeqCst) > 0);
        session.shutdown().await.expect("shutdown");
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_custom_serial_opt_in_reads_config_and_starts() {
        smol::block_on(async {
            let (transport, counts) =
                AsyncSpy::new_with_hint(AddressingMode::Serial, None, Some(AddressingMode::Serial));
            let session = grafton_visca::Session::open(
                transport,
                multi_config(),
                grafton_visca::SmolRuntime::new(),
            )
            .await
            .expect("explicit custom serial opt-in");
            assert!(counts.config_reads.load(Ordering::SeqCst) > 0);
            session.shutdown().await.expect("shutdown");
        });
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_unsupported_multi_target_topologies_do_not_write_or_read() {
        unsupported_multi_target_topologies_do_not_write_or_read::<_, _>(|| {
            grafton_visca::TokioRuntime::from_current().expect("runtime")
        })
        .await;
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_unsupported_multi_target_topologies_do_not_write_or_read() {
        smol::block_on(unsupported_multi_target_topologies_do_not_write_or_read::<
            _,
            _,
        >(grafton_visca::SmolRuntime::new));
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_mixed_envelopes_and_custom_ip_fail_before_io() {
        unsupported_envelopes_and_custom_ip_do_not_write_or_read::<_, _>(|| {
            grafton_visca::TokioRuntime::from_current().expect("runtime")
        })
        .await;
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_mixed_envelopes_and_custom_ip_fail_before_io() {
        smol::block_on(unsupported_envelopes_and_custom_ip_do_not_write_or_read::<
            _,
            _,
        >(grafton_visca::SmolRuntime::new));
    }

    async fn strictest_pacing_applies_across_registered_targets<E>(runtime: E)
    where
        E: grafton_visca::Executor,
    {
        let base = raw_profile();
        let strict = base.clone();
        let (transport, counts) =
            AsyncSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
        let session = grafton_visca::Session::open(
            transport,
            SessionConfig::new(base)
                .with_target(CameraId::CAMERA_2, strict)
                .expect("strict multi-target config")
                .with_tuning(OperationalTuning::new().command_spacing(Duration::from_millis(120)))
                .expect("strict session pacing"),
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
        assert_eq!(counts.writes.load(Ordering::SeqCst), 2);
        session.shutdown().await.expect("shutdown");
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_strictest_profile_pacing_is_shared_by_all_targets() {
        strictest_pacing_applies_across_registered_targets(
            grafton_visca::TokioRuntime::from_current().expect("runtime"),
        )
        .await;
    }

    #[cfg(feature = "runtime-smol")]
    #[test]
    fn smol_strictest_profile_pacing_is_shared_by_all_targets() {
        smol::block_on(strictest_pacing_applies_across_registered_targets(
            grafton_visca::SmolRuntime::new(),
        ));
    }
}

#[cfg(feature = "blocking")]
mod blocking_registry {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
    };

    use grafton_visca::{
        camera::TransportKind,
        command::CommandKind,
        transport::{
            AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
    };

    #[derive(Debug, Clone)]
    struct Counts {
        config_reads: Arc<AtomicUsize>,
        writes: Arc<AtomicUsize>,
        reads: Arc<AtomicUsize>,
    }

    impl Counts {
        fn new() -> Self {
            Self {
                config_reads: Arc::new(AtomicUsize::new(0)),
                writes: Arc::new(AtomicUsize::new(0)),
                reads: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[derive(Debug)]
    struct BlockingSpy {
        config: TransportConfig,
        standard_kind: Option<TransportKind>,
        addressing_hint: Option<AddressingMode>,
        responses: VecDeque<Vec<u8>>,
        counts: Counts,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl BlockingSpy {
        fn new(addressing: AddressingMode, standard_kind: Option<TransportKind>) -> (Self, Counts) {
            Self::new_with_hint(
                addressing,
                standard_kind,
                (standard_kind == Some(TransportKind::Serial)).then_some(addressing),
            )
        }

        fn new_with_hint(
            addressing: AddressingMode,
            standard_kind: Option<TransportKind>,
            addressing_hint: Option<AddressingMode>,
        ) -> (Self, Counts) {
            let counts = Counts::new();
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig {
                        addressing,
                        ..TransportConfig::default()
                    },
                    standard_kind,
                    addressing_hint,
                    responses: VecDeque::new(),
                    counts: counts.clone(),
                    writes,
                },
                counts,
            )
        }

        fn receive(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            self.counts.reads.fetch_add(1, Ordering::SeqCst);
            let response = self.responses.pop_front().ok_or(Error::Timeout)?;
            dst[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }
    }

    impl HasTransportConfig for BlockingSpy {
        fn transport_config(&self) -> &TransportConfig {
            self.counts.config_reads.fetch_add(1, Ordering::SeqCst);
            &self.config
        }

        fn standard_transport_kind(&self) -> Option<TransportKind> {
            self.standard_kind
        }
    }

    impl BlockingTransport for BlockingSpy {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            self.counts.writes.fetch_add(1, Ordering::SeqCst);
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            let id = bytes.first().copied().unwrap_or(0x81) & 0x0f;
            let source = if self.config.addressing == AddressingMode::Serial {
                0x80 | (id.saturating_add(8) << 4)
            } else {
                0x90
            };
            self.responses.push_back(vec![source, 0x41, 0xff]);
            self.responses.push_back(vec![source, 0x51, 0xff]);
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            self.receive(dst)
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            self.addressing_hint
        }

        fn send_semantics(&self) -> SendSemantics {
            if self.config.addressing == AddressingMode::Serial {
                SendSemantics::Stream
            } else {
                SendSemantics::Datagram
            }
        }
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
        let (transport, counts) =
            BlockingSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
        let writes = Arc::clone(&transport.writes);
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
        assert_eq!(counts.writes.load(Ordering::SeqCst), 2);
        let writes = writes.lock().expect("writes lock");
        assert_eq!(writes[0][0], CameraId::CAMERA_2.to_address_byte());
        assert_eq!(writes[1][0], CameraId::CAMERA_1.to_address_byte());
    }

    #[test]
    fn blocking_topology_rejects_before_config_or_io() {
        for kind in [TransportKind::Tcp, TransportKind::Udp] {
            let (transport, counts) = BlockingSpy::new(AddressingMode::Ip, Some(kind));
            let error = grafton_visca::blocking::Session::open(transport, multi_config())
                .expect_err("IP multi-target topology");
            assert!(matches!(error, Error::NotSupported));
            assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
            assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
            assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn blocking_empty_registry_fails_before_transport_config_or_io() {
        let (transport, counts) =
            BlockingSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
        let error = grafton_visca::blocking::Session::open(transport, SessionConfig::default())
            .expect_err("empty registry");
        assert!(matches!(error, Error::InvalidRequest(_)));
        assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
        assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
        assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn blocking_mixed_envelope_and_custom_ip_fail_before_io() {
        let cases = [
            (
                BlockingSpy::new(AddressingMode::Serial, None),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, sony_profile())
                    .expect("mixed envelope config"),
            ),
            (
                BlockingSpy::new(AddressingMode::Ip, Some(TransportKind::Custom)),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, raw_profile())
                    .expect("custom IP config"),
            ),
            (
                BlockingSpy::new(AddressingMode::Ip, None),
                SessionConfig::new(raw_profile())
                    .with_target(CameraId::CAMERA_2, raw_profile())
                    .expect("unknown IP config"),
            ),
        ];
        for ((transport, counts), config) in cases {
            let error = grafton_visca::blocking::Session::open(transport, config)
                .expect_err("unsupported topology");
            assert!(matches!(error, Error::NotSupported));
            assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
            assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
            assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
        }
    }

    #[test]
    fn blocking_custom_serial_opt_in_reads_config_and_starts() {
        let (transport, counts) =
            BlockingSpy::new_with_hint(AddressingMode::Serial, None, Some(AddressingMode::Serial));
        let session = grafton_visca::blocking::Session::open(transport, multi_config())
            .expect("explicit custom serial opt-in");
        assert!(counts.config_reads.load(Ordering::SeqCst) > 0);
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_strictest_profile_pacing_is_shared_by_all_targets() {
        let base = raw_profile();
        let strict = base.clone();
        let (transport, counts) =
            BlockingSpy::new(AddressingMode::Serial, Some(TransportKind::Serial));
        let session = grafton_visca::blocking::Session::open(
            transport,
            SessionConfig::new(base)
                .with_target(CameraId::CAMERA_2, strict)
                .expect("strict multi-target config")
                .with_tuning(OperationalTuning::new().command_spacing(Duration::from_millis(120)))
                .expect("strict session pacing"),
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
        assert_eq!(counts.writes.load(Ordering::SeqCst), 2);
        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-tokio"))]
mod coexistence {
    use super::*;
    use std::{collections::VecDeque, future::Future, time::Duration};

    use grafton_visca::{
        blocking::Session as BlockingSession,
        command::CommandKind,
        transport::{
            AsyncTransport, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
    };

    #[derive(Debug)]
    struct BlockingProbe {
        config: TransportConfig,
        responses: VecDeque<Vec<u8>>,
    }

    impl HasTransportConfig for BlockingProbe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for BlockingProbe {
        fn send_with_timeout(
            &mut self,
            _bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            self.responses.push_back(vec![0x90, 0x41, 0xff]);
            self.responses.push_back(vec![0x90, 0x51, 0xff]);
            Ok(())
        }
        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let bytes = self.responses.pop_front().ok_or(Error::Timeout)?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[derive(Debug)]
    struct AsyncProbe {
        config: TransportConfig,
        replies: flume::Receiver<Vec<u8>>,
        reply_tx: flume::Sender<Vec<u8>>,
    }

    impl HasTransportConfig for AsyncProbe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for AsyncProbe {
        fn send(&mut self, _bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let tx = self.reply_tx.clone();
            async move {
                tx.send_async(vec![0x90, 0x41, 0xff])
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                tx.send_async(vec![0x90, 0x51, 0xff])
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                Ok(())
            }
        }
        #[allow(clippy::manual_async_fn)]
        fn recv_into<'a>(
            &'a mut self,
            dst: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            async move {
                let bytes = self
                    .replies
                    .recv_async()
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                dst[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
        }
        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[tokio::test]
    async fn blocking_and_async_sessions_have_one_owner_each_and_isolated_shutdown() {
        let blocking = BlockingSession::open(
            BlockingProbe {
                config: TransportConfig::default(),
                responses: VecDeque::new(),
            },
            SessionConfig::new(raw_profile()),
        )
        .expect("blocking session");
        let (reply_tx, replies) = flume::unbounded();
        let async_session = grafton_visca::Session::open(
            AsyncProbe {
                config: TransportConfig::default(),
                replies,
                reply_tx,
            },
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
        async_session.shutdown().await.expect("async shutdown");
    }
}
