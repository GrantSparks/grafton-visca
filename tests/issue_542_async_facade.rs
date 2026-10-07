//! Narrow async facade coverage for the single-owner vertical slice.
//!
//! Every scenario is generic over the executor and runs under each enabled
//! runtime, as the cases `scenario::tokio` and `scenario::smol`. The smol-only
//! leg therefore runs the same nine scenarios as the Tokio-only leg, among
//! them `owner_admits_uses_and_closes`, the plain open/use/close smoke case.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;
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
        CompileTimeProfile, PositionInquirySupport, ProfileEnvelope, ProfileSpec, ProfileTiming,
        TransportCompatibility,
    },
    profiles::{ProfileId, PtzOpticsG2, SonyFR7},
    request,
    transport::{AddressingMode, TransportConfig},
    Camera, CameraId, ControlClass, Error, Executor, Inquiry, InquiryRoute, Operation,
    OperationCommand, Request, ResponseDecoder, RetryClass, Session, SessionConfig, TimeoutClass,
};

use fake_camera::{frames, AsyncWire, FakeCamera};
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

/// A camera that answers each command with ACK and completion on socket 1 and
/// each inquiry with the data `[1, 2]`, from the address of the camera the
/// request was written to (camera `n` replies as `0x90 + 0x10 * (n - 1)`).
fn camera() -> FakeCamera {
    FakeCamera::new(|write, answer| {
        let source = write.first().map_or(0x90, |address| {
            0x80 | ((address & 0x0f).saturating_add(8) << 4)
        });
        let replies = if write.get(1) == Some(&0x09) {
            vec![frames::inquiry_reply(&[0x01, 0x02])]
        } else {
            vec![frames::ack(1), frames::complete(1)]
        };
        for mut reply in replies {
            reply[0] = source;
            answer.reply(reply);
        }
    })
}

/// A wire onto `camera` that reports a standard serial transport with serial
/// addressing.
fn serial_wire(camera: &FakeCamera) -> AsyncWire {
    let mut config = TransportConfig::default();
    config.addressing = AddressingMode::Serial;
    camera
        .async_wire()
        .with_config(config)
        .with_addressing(AddressingMode::Serial)
        .with_transport_kind(TransportKind::Serial)
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
            ProfileTiming::builder()
                .ack_timeout(Duration::from_millis(100))
                .command_timeouts(grafton_visca::CommandTimeouts::default())
                .inquiry_timeout(Duration::from_secs(1))
                .cancellation_timeout(Duration::from_secs(1))
                .ambiguity_timeout(Duration::from_secs(1))
                .busy_timeout(Duration::ZERO)
                .raw_inquiry_reply_skew(Duration::ZERO)
                .minimum_inquiry_spacing(Duration::ZERO)
                .minimum_command_spacing(Duration::ZERO)
                .build()
                .expect("valid timing"),
        )
        .maximum_command_sockets(1)
        .supports_operation_complete(false)
        .supports_command_cancel(false)
        .preset_recall_axes(None)
        .position_inquiries(PositionInquirySupport::new(false, false, false))
        .build()
        .expect("unsupported profile")
}

/// An executor that counts the tasks spawned through it and otherwise defers
/// to the wrapped runtime.
#[derive(Clone)]
struct CountingExecutor<E> {
    inner: E,
    spawned: Arc<AtomicUsize>,
}

impl<E: Executor> Executor for CountingExecutor<E> {
    type Join<T>
        = <E as Executor>::Join<T>
    where
        T: Send + 'static;
    type Detach = <E as Executor>::Detach;

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

async fn one_owner_admits_plain_inquiry_and_typed_operations<E: Executor>(executor: E) {
    let fake = camera();
    let session = Session::open(
        fake.async_wire(),
        SessionConfig::new(generic_profile()),
        executor,
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

    session.shutdown().expect("shutdown");
    assert_eq!(fake.write_count(), 4);
}

async fn unsupported_axis_fails_before_owner_admission_or_io<E: Executor>(executor: E) {
    let fake = camera();
    let session = Session::open(
        fake.async_wire(),
        SessionConfig::new(unsupported_focus_profile()),
        executor,
    )
    .await
    .expect("session");
    let error = session
        .camera::<PtzOpticsG2>()
        .expect_err("runtime profile mismatch must fail before projection");
    assert!(matches!(error, Error::InvalidRequest(_)));
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");
}

async fn raw_serial_multi_target_registration_routes_each_camera<E: Executor>(executor: E) {
    let fake = camera();
    let mut config = SessionConfig::new(generic_profile());
    config
        .register_target(CameraId::CAMERA_2, generic_profile())
        .expect("bounded registration");

    let session = Session::open(serial_wire(&fake), config, executor)
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

    session.shutdown().expect("shutdown");
    let sent = fake.writes();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[0][0], CameraId::CAMERA_2.to_address_byte());
    assert_eq!(sent[1][0], CameraId::CAMERA_1.to_address_byte());
}

async fn incompatible_standard_transport_fails_before_send_or_owner_spawn<E: Executor>(
    executor: E,
) {
    let fake = camera();
    let spawned = Arc::new(AtomicUsize::new(0));
    let executor = CountingExecutor {
        inner: executor,
        spawned: Arc::clone(&spawned),
    };

    let error = Session::open(
        fake.async_wire().with_transport_kind(TransportKind::Tcp),
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
            ..
        }
    ));
    assert_eq!(fake.write_count(), 0);
    assert_eq!(spawned.load(Ordering::SeqCst), 0);
}

async fn dropping_or_detaching_operation_is_observation_only<E: Executor>(executor: E) {
    let fake = camera();
    let session = Session::open(
        fake.async_wire(),
        SessionConfig::new(generic_profile()),
        executor.clone(),
    )
    .await
    .expect("session");
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    let dropped = camera.zoom().stop().await.expect("operation admission");
    drop(dropped);
    executor.sleep(Duration::from_millis(20)).await;
    assert_eq!(fake.write_count(), 1);

    let detached = camera.zoom().stop().await.expect("operation admission");
    detached.detach();
    executor.sleep(Duration::from_millis(20)).await;
    assert!(fake.write_count() > 0);
    session.shutdown().expect("shutdown request");
}

async fn shutdown_requests_actor_and_rejects_later_admission_without_join_claim<E: Executor>(
    executor: E,
) {
    let fake = camera();
    let session = Session::open(
        fake.async_wire(),
        SessionConfig::new(generic_profile()),
        executor,
    )
    .await
    .expect("session");
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    session.shutdown().expect("shutdown request");
    let error = camera
        .execute(&PlainPing)
        .await
        .expect_err("shutdown must reject new submissions");
    assert!(matches!(error, Error::RuntimeShutdown));
    assert_eq!(fake.write_count(), 0);
}

async fn non_default_downstream_profile_projects_and_clones_without_new_owner<E: Executor>(
    executor: E,
) {
    let fake = camera();
    let spawned = Arc::new(AtomicUsize::new(0));
    let executor = CountingExecutor {
        inner: executor,
        spawned: Arc::clone(&spawned),
    };
    let session = Session::open(
        fake.async_wire(),
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
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");
}

async fn wrong_unregistered_and_ambiguous_targets_fail_before_admission_or_io<E: Executor>(
    executor: E,
) {
    let fake = camera();
    let mut config = SessionConfig::from_compile_time::<PtzOpticsG2>().expect("config");
    config
        .register_target(CameraId::CAMERA_2, generic_profile())
        .expect("second target");
    let session = Session::open(serial_wire(&fake), config, executor)
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
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");
}

async fn owner_admits_uses_and_closes<E: Executor>(executor: E) {
    let session = Session::open(
        FakeCamera::acking(1).async_wire(),
        SessionConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
        executor,
    )
    .await
    .unwrap();

    session
        .camera::<PtzOpticsG2>()
        .unwrap()
        .zoom()
        .stop()
        .await
        .unwrap()
        .applied()
        .await
        .unwrap();

    // Exercise the explicit close path as well as the operation path.
    session.close().await.unwrap();
}

runtime_matrix!(
    one_owner_admits_plain_inquiry_and_typed_operations,
    unsupported_axis_fails_before_owner_admission_or_io,
    raw_serial_multi_target_registration_routes_each_camera,
    incompatible_standard_transport_fails_before_send_or_owner_spawn,
    dropping_or_detaching_operation_is_observation_only,
    shutdown_requests_actor_and_rejects_later_admission_without_join_claim,
    non_default_downstream_profile_projects_and_clones_without_new_owner,
    wrong_unregistered_and_ambiguous_targets_fail_before_admission_or_io,
    owner_admits_uses_and_closes,
);
