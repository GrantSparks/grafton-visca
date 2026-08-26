//! Narrow async facade coverage for the single-owner vertical slice.

#![cfg(all(feature = "async", feature = "runtime-tokio"))]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use grafton_visca::{
    camera::TransportKind,
    capabilities::Capabilities,
    completion::{AppliedOnly, Targeted},
    profile::{
        CompileTimeProfile, PositionInquirySupport, ProfileEnvelope, ProfileSpec,
        TransportCompatibility,
    },
    profiles::{ProfileId, PtzOpticsG2, SonyFR7},
    request,
    transport::{
        AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    Camera, CameraId, ControlClass, Error, Executor, Inquiry, InquiryRoute, Operation,
    OperationCommand, Request, ResponseDecoder, RetryClass, Session, SessionConfig, TimeoutClass,
    TokioExecutor, TokioRuntime,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

#[allow(dead_code)]
async fn generic_operation_submission<P, K, O>(
    camera: &Camera<P>,
    operation: &O,
) -> Result<Operation<K>, Error>
where
    P: CompileTimeProfile,
    K: grafton_visca::completion::Kind,
    O: OperationCommand<K> + ?Sized,
{
    camera.submit::<K, O>(operation).await
}

#[derive(Debug)]
struct PlainPing;

impl Request for PlainPing {
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

#[derive(Debug)]
struct RawInquiry;

impl Request for RawInquiry {
    type Class = request::Inquiry;
    const MAX_SIZE: usize = 3;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, out: &mut [u8]) -> Result<usize, Error> {
        out[..3].copy_from_slice(&[target.to_address_byte(), 0x09, 0xff]);
        Ok(3)
    }
}

impl Inquiry for RawInquiry {
    type Response = Vec<u8>;

    fn route(&self) -> InquiryRoute {
        InquiryRoute::RAW
    }

    fn decoder(&self) -> ResponseDecoder<Self::Response> {
        ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
    }
}

#[derive(Debug)]
struct ScriptedTransport {
    config: TransportConfig,
    standard_kind: Option<TransportKind>,
    responses: tokio::sync::mpsc::Receiver<Vec<u8>>,
    response_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    sent: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

impl HasTransportConfig for ScriptedTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn standard_transport_kind(&self) -> Option<TransportKind> {
        self.standard_kind
    }
}

impl AsyncTransport for ScriptedTransport {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.sent.lock().expect("sent lock").push(bytes.to_vec());
        let source = bytes
            .first()
            .map(|address| 0x80 | (address & 0x0f).saturating_add(8) << 4)
            .unwrap_or(0x90);
        if bytes.get(1) == Some(&0x09) {
            self.response_tx
                .try_send(vec![source, 0x50, 0x01, 0x02, 0xff])
                .expect("inquiry response queue");
        } else {
            self.response_tx
                .try_send(vec![source, 0x41, 0xff])
                .expect("ack queue");
            self.response_tx
                .try_send(vec![source, 0x51, 0xff])
                .expect("completion queue");
        }
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        let bytes = self
            .responses
            .recv()
            .await
            .ok_or(Error::ConnectionClosed { reason: None })?;
        dst[..bytes.len()].copy_from_slice(&bytes);
        Ok(bytes.len())
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        (self.standard_kind == Some(TransportKind::Serial)).then_some(self.config.addressing)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn generic_profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile")
}

fn unsupported_focus_profile() -> ProfileSpec {
    let capabilities = Capabilities::runtime_baseline("no-focus-camera", 1).expect("baseline");
    ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new(Some(5678), None, false))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(
            Duration::from_millis(100),
            Duration::from_secs(5),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
        )
        .maximum_command_sockets(1)
        .supports_operation_complete(false)
        .supports_command_cancel(false)
        .preset_recall_axes(None)
        .position_inquiries(PositionInquirySupport::new(false, false, false))
        .build()
        .expect("unsupported profile")
}

fn transport() -> (ScriptedTransport, Arc<std::sync::Mutex<Vec<Vec<u8>>>>) {
    let (response_tx, responses) = tokio::sync::mpsc::channel(16);
    let sent = Arc::new(std::sync::Mutex::new(Vec::new()));
    (
        ScriptedTransport {
            config: TransportConfig::default(),
            standard_kind: None,
            responses,
            response_tx,
            sent: Arc::clone(&sent),
        },
        sent,
    )
}

#[derive(Clone)]
struct CountingExecutor {
    inner: TokioExecutor,
    spawned: Arc<AtomicUsize>,
}

impl Executor for CountingExecutor {
    type Join<T>
        = <TokioExecutor as Executor>::Join<T>
    where
        T: Send + 'static;
    type Detach = <TokioExecutor as Executor>::Detach;

    fn spawn_with_detach<F>(&self, future: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        self.inner.spawn_with_detach(future)
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.inner.block_on(future)
    }

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        self.inner.sleep(duration)
    }

    fn timeout<'a, F, T>(
        &'a self,
        duration: Duration,
        future: F,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        self.inner.timeout(duration, future)
    }

    fn now(&self) -> std::time::Instant {
        self.inner.now()
    }
}

#[tokio::test]
async fn one_owner_admits_plain_inquiry_and_typed_operations() {
    let (transport, sent) = transport();
    let session = Session::open(
        transport,
        SessionConfig::new(generic_profile()),
        TokioRuntime::from_current().expect("runtime"),
    )
    .await
    .expect("session");
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    camera.execute(&PlainPing).await.expect("plain command");
    assert_eq!(
        camera.inquire(&RawInquiry).await.expect("inquiry"),
        vec![1, 2]
    );
    camera
        .submit::<Targeted, _>(&grafton_visca::request::builtin::PanTiltHome)
        .await
        .expect("targeted admission")
        .settled()
        .await
        .expect("targeted settlement");
    camera
        .submit::<AppliedOnly, _>(&grafton_visca::request::builtin::ZoomStop)
        .await
        .expect("applied-only admission")
        .applied()
        .await
        .expect("applied completion");

    session.shutdown().await.expect("shutdown");
    assert_eq!(sent.lock().expect("sent lock").len(), 4);
}

#[tokio::test]
async fn unsupported_axis_fails_before_owner_admission_or_io() {
    let (transport, sent) = transport();
    let session = Session::open(
        transport,
        SessionConfig::new(unsupported_focus_profile()),
        TokioRuntime::from_current().expect("runtime"),
    )
    .await
    .expect("session");
    let error = session
        .camera::<PtzOpticsG2>()
        .expect_err("runtime profile mismatch must fail before projection");
    assert!(matches!(error, Error::InvalidRequest(_)));
    assert!(sent.lock().expect("sent lock").is_empty());
    session.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn raw_serial_multi_target_registration_routes_each_camera() {
    let (mut transport, sent) = transport();
    transport.standard_kind = Some(TransportKind::Serial);
    transport.config.addressing = AddressingMode::Serial;
    let mut config = SessionConfig::new(generic_profile());
    config
        .register_target(CameraId::CAMERA_2, generic_profile())
        .expect("bounded registration");

    let session = Session::open(
        transport,
        config,
        TokioRuntime::from_current().expect("runtime"),
    )
    .await
    .expect("raw serial multi-target startup");
    assert!(matches!(
        session
            .camera::<PtzOpticsG2>()
            .expect_err("camera() requires one sole target"),
        Error::InvalidState(_)
    ));

    session
        .camera_for::<PtzOpticsG2>(CameraId::CAMERA_2)
        .expect("registered camera 2")
        .execute(&PlainPing)
        .await
        .expect("camera 2 command");
    session
        .camera_for::<PtzOpticsG2>(CameraId::CAMERA_1)
        .expect("registered camera 1")
        .execute(&PlainPing)
        .await
        .expect("camera 1 command");

    session.shutdown().await.expect("shutdown");
    let sent = sent.lock().expect("sent lock");
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0][0], CameraId::CAMERA_2.to_address_byte());
    assert_eq!(sent[1][0], CameraId::CAMERA_1.to_address_byte());
}

#[tokio::test]
async fn incompatible_standard_transport_fails_before_send_or_owner_spawn() {
    let (mut transport, sent) = transport();
    transport.standard_kind = Some(TransportKind::Tcp);
    let spawned = Arc::new(AtomicUsize::new(0));
    let executor = CountingExecutor {
        inner: TokioExecutor::from_current().expect("runtime"),
        spawned: Arc::clone(&spawned),
    };

    let error = Session::open(
        transport,
        SessionConfig::new(ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile")),
        executor,
    )
    .await
    .expect_err("Sony FR7 must reject standard TCP before startup");

    assert!(matches!(
        error,
        Error::UnsupportedTransport {
            profile: ProfileId::SonyFr7,
            transport: TransportKind::Tcp,
        }
    ));
    assert!(sent.lock().expect("sent lock").is_empty());
    assert_eq!(spawned.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn dropping_or_detaching_operation_is_observation_only() {
    let (transport, sent) = transport();
    let session = Session::open(
        transport,
        SessionConfig::new(generic_profile()),
        TokioRuntime::from_current().expect("runtime"),
    )
    .await
    .expect("session");
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    let dropped = camera.zoom().stop().await.expect("operation admission");
    drop(dropped);
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(sent.lock().expect("sent lock").len(), 1);

    let detached = camera.zoom().stop().await.expect("operation admission");
    detached.detach();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!sent.lock().expect("sent lock").is_empty());
    session.shutdown().await.expect("shutdown request");
}

#[tokio::test]
async fn shutdown_requests_actor_and_rejects_later_admission_without_join_claim() {
    let (transport, sent) = transport();
    let session = Session::open(
        transport,
        SessionConfig::new(generic_profile()),
        TokioRuntime::from_current().expect("runtime"),
    )
    .await
    .expect("session");
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    session.shutdown().await.expect("shutdown request");
    let error = camera
        .execute(&PlainPing)
        .await
        .expect_err("shutdown must reject new submissions");
    assert!(matches!(error, Error::RuntimeShutdown));
    assert!(sent.lock().expect("sent lock").is_empty());
}

#[tokio::test]
async fn non_default_downstream_profile_projects_and_clones_without_new_owner() {
    let (transport, sent) = transport();
    let spawned = Arc::new(AtomicUsize::new(0));
    let executor = CountingExecutor {
        inner: TokioExecutor::from_current().expect("runtime"),
        spawned: Arc::clone(&spawned),
    };
    let session = Session::open(
        transport,
        SessionConfig::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("non-default profile config"),
        executor,
    )
    .await
    .expect("session");

    let camera: Camera<NonDefaultCompileTimeProfile> = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("exact non-default profile projection");
    let clone = camera.clone();
    assert_eq!(camera.target(), clone.target());
    assert_eq!(spawned.load(Ordering::SeqCst), 1);
    assert!(sent.lock().expect("sent lock").is_empty());
    session.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn wrong_unregistered_and_ambiguous_targets_fail_before_admission_or_io() {
    let (mut transport, sent) = transport();
    transport.standard_kind = Some(TransportKind::Serial);
    transport.config.addressing = AddressingMode::Serial;
    let mut config = SessionConfig::from_compile_time::<PtzOpticsG2>().expect("config");
    config
        .register_target(CameraId::CAMERA_2, generic_profile())
        .expect("second target");
    let session = Session::open(
        transport,
        config,
        TokioRuntime::from_current().expect("runtime"),
    )
    .await
    .expect("session");

    assert!(matches!(
        session.camera::<PtzOpticsG2>(),
        Err(Error::InvalidState(_))
    ));
    assert!(matches!(
        session.camera_for::<PtzOpticsG2>(CameraId::CAMERA_7),
        Err(Error::InvalidRequest(_))
    ));
    assert!(matches!(
        session.camera_for::<SonyFR7>(CameraId::CAMERA_2),
        Err(Error::InvalidRequest(_))
    ));
    assert!(sent.lock().expect("sent lock").is_empty());
    session.shutdown().await.expect("shutdown");
}
