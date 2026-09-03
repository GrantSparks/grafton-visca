//! Runtime validation for the owner-backed dynamic API.
//!
//! These tests intentionally use a tiny in-memory transport instead of the
//! legacy test-kit camera.  That keeps the assertions at the final
//! `Session`/`DynSessionCamera` boundary and makes it possible to prove that
//! profile rejection and dynamic capability gates happen before a write.

#![cfg(all(feature = "dyn-api", feature = "runtime-tokio"))]

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use grafton_visca::{
    capabilities::TypedSupportSurface,
    command::MulticastStreaming,
    dynapi::{
        DynAppliedRequest, DynSessionCamera, DynSessionCameraControl, DynSessionCameraNouns,
        DynTargetedRequest,
    },
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    request::builtin::{PanTiltHome, ZoomStop},
    state_cache::StateEntry,
    transport::{
        AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    CameraId, CancellationOutcome, Error, OperationalTuning, Session, SessionConfig, StateKey,
    TokioRuntime,
};

/// The dynamic noun methods return one boxed future.  Count only construction
/// allocations and compare that with an explicitly boxed static future; this
/// catches an accidental second outer box without depending on allocator
/// internals during polling.
struct CountingAllocator;

static ALLOCATION_LOCK: Mutex<()> = Mutex::new(());

thread_local! {
    static COUNT_ALLOCATIONS: Cell<bool> = const { Cell::new(false) };
    static ALLOCATION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNT_ALLOCATIONS.try_with(Cell::get).unwrap_or(false) {
            let _ = ALLOCATION_COUNT.try_with(|count| {
                count.set(count.get().saturating_add(1));
            });
        }
        // SAFETY: this allocator forwards the original layout and pointer to
        // the platform allocator unchanged.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: `pointer` and `layout` came from `System::alloc` above.
        unsafe { System.dealloc(pointer, layout) }
    }
}

struct AllocationGuard;

impl Drop for AllocationGuard {
    fn drop(&mut self) {
        let _ = COUNT_ALLOCATIONS.try_with(|counting| counting.set(false));
    }
}

fn allocations_during(action: impl FnOnce()) -> usize {
    let _lock = ALLOCATION_LOCK.lock().expect("allocation lock");
    ALLOCATION_COUNT.with(|count| count.set(0));
    COUNT_ALLOCATIONS.with(|counting| counting.set(true));
    {
        let _guard = AllocationGuard;
        action();
    }
    ALLOCATION_COUNT.with(Cell::get)
}

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PtzOptics G2 profile")
}

/// In-memory owner transport.  A command receives the normal ACK and
/// completion pair; an inquiry receives a compact valid response.  The
/// transport records writes so preflight tests can assert that no I/O was
/// attempted.
#[derive(Debug)]
struct ProbeTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    complete: bool,
}

impl ProbeTransport {
    fn new(complete: bool) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let (response_tx, responses) = flume::unbounded();
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx,
                writes: Arc::clone(&writes),
                complete,
            },
            writes,
        )
    }

    fn serial(complete: bool) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let (mut transport, writes) = Self::new(complete);
        transport.config.addressing = AddressingMode::Serial;
        (transport, writes)
    }

    fn source(&self, bytes: &[u8]) -> u8 {
        let target = bytes.first().copied().unwrap_or(0x81) & 0x0f;
        match self.config.addressing {
            AddressingMode::Serial => 0x80 | target.saturating_add(8) << 4,
            AddressingMode::Ip => 0x90,
        }
    }
}

impl HasTransportConfig for ProbeTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for ProbeTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        let bytes = bytes.to_vec();
        let source = self.source(&bytes);
        let complete = self.complete;
        let response_tx = self.response_tx.clone();
        self.writes
            .lock()
            .expect("probe writes lock")
            .push(bytes.clone());
        async move {
            if bytes.get(1) == Some(&0x09) {
                response_tx
                    .send_async(vec![source, 0x50, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
            } else {
                response_tx
                    .send_async(vec![source, 0x41, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                if complete {
                    response_tx
                        .send_async(vec![source, 0x51, 0xff])
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                }
            }
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn recv_into<'a>(
        &'a mut self,
        destination: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send {
        async move {
            let response = self
                .responses
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            destination[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(self.config.addressing)
    }

    fn send_semantics(&self) -> SendSemantics {
        match self.config.addressing {
            AddressingMode::Serial => SendSemantics::Stream,
            AddressingMode::Ip => SendSemantics::Datagram,
        }
    }
}

async fn open_session(transport: ProbeTransport, config: SessionConfig) -> Session {
    Session::open(
        transport,
        config,
        TokioRuntime::from_current().expect("Tokio runtime"),
    )
    .await
    .expect("owner session")
}

fn assert_unknown(camera: &DynSessionCamera) {
    assert_eq!(
        camera.state_cache().value(StateKey::MulticastStreaming),
        StateEntry::Unknown
    );
}

fn assert_state(camera: &DynSessionCamera, expected: &[i64]) {
    match camera.state_cache().value(StateKey::MulticastStreaming) {
        StateEntry::Set(value) => assert_eq!(value.as_slice(), expected),
        other => panic!("expected multicast state {expected:?}, got {other:?}"),
    }
}

#[tokio::test]
async fn tokio_dynamic_nouns_preserve_targeted_applied_and_custom_lifecycles() {
    let (transport, writes) = ProbeTransport::new(true);
    let session = open_session(transport, SessionConfig::new(profile())).await;
    let camera = session.camera_dyn().expect("dynamic camera");
    let root: &dyn DynSessionCameraControl = &camera;
    let nouns: &dyn DynSessionCameraNouns = &camera;

    assert_eq!(root.target(), CameraId::CAMERA_1);
    assert!(root.capabilities().has_zoom);
    assert!(!root.supports_typed(TypedSupportSurface::DigitalZoomToggle));

    nouns
        .zoom()
        .stop()
        .await
        .expect("applied admission")
        .applied()
        .await
        .expect("applied lifecycle");
    nouns
        .pan_tilt()
        .home()
        .await
        .expect("targeted admission")
        .applied()
        .await
        .expect("targeted applied lifecycle");

    // Built-in requests can also be erased behind the object-safe custom
    // request markers.  They must still use the same owner and return the
    // corresponding closed operation handle.
    let targeted: &dyn DynTargetedRequest = &PanTiltHome;
    root.submit_targeted(targeted)
        .await
        .expect("custom targeted admission")
        .applied()
        .await
        .expect("custom targeted applied lifecycle");
    let applied: &dyn DynAppliedRequest = &ZoomStop;
    root.submit_applied(applied)
        .await
        .expect("custom applied admission")
        .applied()
        .await
        .expect("custom applied lifecycle");

    assert_eq!(writes.lock().expect("writes lock").len(), 4);
    session.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn tokio_dynamic_unsupported_gate_rejects_before_transport_io() {
    let (transport, writes) = ProbeTransport::new(true);
    let session = open_session(transport, SessionConfig::new(profile())).await;
    let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");

    let error = camera
        .zoom()
        .set_digital_zoom(true)
        .await
        .expect_err("PtzOptics G2 has no digital zoom toggle");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "digital zoom"
        }
    ));
    assert!(writes.lock().expect("writes lock").is_empty());

    session.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn tokio_dynamic_cache_views_share_state_and_isolate_targets() {
    let (transport, _) = ProbeTransport::serial(true);
    let mut config = SessionConfig::new(profile());
    config
        .register_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config).await;
    let first = session
        .camera_dyn_for(CameraId::CAMERA_1)
        .expect("camera 1 dynamic view");
    let first_view = DynSessionCamera::from_session_target(&session, CameraId::CAMERA_1)
        .expect("same-target dynamic view");
    let second = DynSessionCamera::from_session_target(&session, CameraId::CAMERA_2)
        .expect("camera 2 dynamic view");

    assert_unknown(&first);
    assert_unknown(&second);
    first
        .advanced()
        .multicast_on()
        .await
        .expect("exact applied multicast effect");
    assert_state(&first, &[1]);
    assert_state(&first_view, &[1]);
    assert_unknown(&second);

    first
        .advanced()
        .multicast_off()
        .await
        .expect("exact applied multicast clear effect");
    assert_state(&first, &[0]);
    assert_unknown(&second);
    session.shutdown().await.expect("shutdown");

    // A fresh owner receives a fresh fixed registry; state never leaks from
    // a previous session even when the profile is identical.
    let (fresh_transport, _) = ProbeTransport::new(true);
    let fresh = open_session(fresh_transport, SessionConfig::new(profile())).await;
    let fresh_camera = DynSessionCamera::from_session(&fresh).expect("fresh dynamic camera");
    assert_unknown(&fresh_camera);
    fresh.shutdown().await.expect("fresh shutdown");
}

#[tokio::test]
async fn tokio_dynamic_target_selection_rejects_implicit_multi_target_view() {
    let (transport, writes) = ProbeTransport::serial(true);
    let mut config = SessionConfig::new(profile());
    config
        .register_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config).await;

    assert!(matches!(
        DynSessionCamera::from_session(&session),
        Err(Error::InvalidState(_))
    ));
    let selected = session
        .camera_dyn_for(CameraId::CAMERA_2)
        .expect("explicit dynamic target");
    assert_eq!(selected.target(), CameraId::CAMERA_2);
    let _ = selected
        .camera::<PtzOpticsG2>()
        .expect("matching static projection");
    assert!(matches!(
        session.camera::<PtzOpticsG2>(),
        Err(Error::InvalidState(_))
    ));
    assert!(writes.lock().expect("writes lock").is_empty());
    session.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn tokio_dynamic_queued_targeted_cancel_is_owner_local() {
    let (transport, writes) = ProbeTransport::new(false);
    let config = SessionConfig::new(profile())
        .with_tuning(OperationalTuning::new().maximum_command_sockets(1))
        .expect("one socket tuning");
    let session = open_session(transport, config).await;
    let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
    let nouns: &dyn DynSessionCameraNouns = &camera;

    let first = nouns.zoom().stop().await.expect("first operation");
    for _ in 0..100 {
        if !writes.lock().expect("writes lock").is_empty() {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(writes.lock().expect("writes lock").len(), 1);

    let queued = nouns.pan_tilt().home().await.expect("queued operation");
    let cancellation = queued.cancel().await.expect("queued cancellation");
    assert!(matches!(
        cancellation.outcome(Duration::from_secs(1)).await,
        Ok(CancellationOutcome::Cancelled)
    ));
    assert_eq!(writes.lock().expect("writes lock").len(), 1);

    first.detach();
    session.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn tokio_dynamic_future_construction_matches_one_explicit_static_box() {
    let (transport, _) = ProbeTransport::new(true);
    let session = open_session(transport, SessionConfig::new(profile())).await;
    let static_camera = session.camera::<PtzOpticsG2>().expect("static camera");
    let dynamic_camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
    let nouns: &dyn DynSessionCameraNouns = &dynamic_camera;
    let static_zoom = static_camera.zoom();
    let dynamic_zoom = nouns.zoom();

    let static_allocations = allocations_during(|| {
        // This is the explicit baseline box required for a static async
        // future.  It represents one caller-owned allocation, not a library
        // allocation hidden by the test.
        let future = Box::pin(static_zoom.stop());
        std::mem::drop(std::hint::black_box(future));
    });
    let dynamic_allocations = allocations_during(|| {
        let future = dynamic_zoom.stop();
        std::mem::drop(std::hint::black_box(future));
    });
    assert_eq!(
        dynamic_allocations, static_allocations,
        "dynamic noun construction added an outer box: dynamic={dynamic_allocations}, static={static_allocations}"
    );

    session.shutdown().await.expect("shutdown");
}

// Keep the command imported in this binary so the cache test proves the
// canonical state effect through the same public request type used by static
// callers, even though the dynamic noun invokes it internally.
#[allow(dead_code)]
fn _multicast_streaming_type_is_public(_: MulticastStreaming) {}
