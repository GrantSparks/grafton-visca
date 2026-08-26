//! Blocking owner startup guards for issue #542.

#![cfg(feature = "blocking")]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use grafton_visca::{
    blocking::{Camera, Session, SessionConfig},
    camera::{profiles::SonyFR7, TransportKind},
    command::CommandKind,
    profile::ProfileSpec,
    transport::{
        AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    Error,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

#[derive(Debug, Default)]
struct ProbeCounts {
    config_reads: AtomicUsize,
    writes: AtomicUsize,
    reads: AtomicUsize,
}

#[derive(Debug)]
struct ProbeTransport {
    config: TransportConfig,
    standard_kind: Option<TransportKind>,
    counts: Arc<ProbeCounts>,
}

impl ProbeTransport {
    fn new(standard_kind: Option<TransportKind>, counts: Arc<ProbeCounts>) -> Self {
        Self {
            config: TransportConfig::default(),
            standard_kind,
            counts,
        }
    }
}

impl HasTransportConfig for ProbeTransport {
    fn transport_config(&self) -> &TransportConfig {
        self.counts.config_reads.fetch_add(1, Ordering::SeqCst);
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<TransportKind> {
        self.standard_kind
    }
}

impl BlockingTransport for ProbeTransport {
    fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        self.counts.writes.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
        self.counts.reads.fetch_add(1, Ordering::SeqCst);
        Ok(0)
    }

    fn recv_into_with_timeout(
        &mut self,
        _dst: &mut [u8],
        _timeout: std::time::Duration,
    ) -> Result<usize, Error> {
        self.counts.reads.fetch_add(1, Ordering::SeqCst);
        Ok(0)
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
    }

    fn send_semantics(&self) -> SendSemantics {
        if self.config.addressing == AddressingMode::Serial {
            SendSemantics::Stream
        } else {
            SendSemantics::Datagram
        }
    }
}

#[test]
fn incompatible_standard_transport_fails_before_blocking_startup_or_io() {
    let counts = Arc::new(ProbeCounts::default());
    let transport = ProbeTransport::new(Some(TransportKind::Tcp), Arc::clone(&counts));
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile");

    let error = Session::open(transport, SessionConfig::new(profile))
        .expect_err("Sony FR7 must reject standard TCP before blocking startup");

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: grafton_visca::camera::profiles::ProfileId::SonyFr7,
            transport: TransportKind::Tcp,
        }
    ));
    assert_eq!(counts.config_reads.load(Ordering::SeqCst), 0);
    assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
    assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
}

#[test]
fn custom_transport_remains_an_explicit_unchecked_escape_hatch() {
    let counts = Arc::new(ProbeCounts::default());
    let transport = ProbeTransport::new(None, counts);
    let profile = ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile");

    Session::open(transport, SessionConfig::new(profile))
        .expect("custom transports remain allowed for advanced integrations")
        .close()
        .expect("closing an idle session should succeed");
}

#[test]
fn non_default_downstream_profile_projects_as_a_borrowed_camera() {
    let counts = Arc::new(ProbeCounts::default());
    let transport = ProbeTransport::new(None, Arc::clone(&counts));
    let session = Session::open(
        transport,
        SessionConfig::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("non-default profile config"),
    )
    .expect("session");

    let camera: Camera<'_, NonDefaultCompileTimeProfile> = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("exact non-default profile projection");
    let _borrowed: &Camera<'_, NonDefaultCompileTimeProfile> = &camera;
    assert_eq!(camera.target(), grafton_visca::CameraId::CAMERA_1);
    assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
    assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
    session.shutdown().expect("shutdown");
}

#[test]
fn wrong_unregistered_and_ambiguous_targets_fail_before_admission_or_io() {
    let counts = Arc::new(ProbeCounts::default());
    let mut transport = ProbeTransport::new(Some(TransportKind::Serial), Arc::clone(&counts));
    transport.config.addressing = AddressingMode::Serial;
    let mut config =
        SessionConfig::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>().expect("config");
    config
        .register_target(
            grafton_visca::CameraId::CAMERA_2,
            grafton_visca::ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>()
                .expect("G2 profile"),
        )
        .expect("second target");
    let session = Session::open(transport, config).expect("session");

    assert!(matches!(
        session.camera::<grafton_visca::profiles::PtzOpticsG2>(),
        Err(Error::InvalidState(_))
    ));
    assert!(matches!(
        session
            .camera_for::<grafton_visca::profiles::PtzOpticsG2>(grafton_visca::CameraId::CAMERA_7),
        Err(Error::InvalidRequest(_))
    ));
    assert!(matches!(
        session.camera_for::<SonyFR7>(grafton_visca::CameraId::CAMERA_2),
        Err(Error::InvalidRequest(_))
    ));
    assert_eq!(counts.writes.load(Ordering::SeqCst), 0);
    assert_eq!(counts.reads.load(Ordering::SeqCst), 0);
    session.shutdown().expect("shutdown");
}
