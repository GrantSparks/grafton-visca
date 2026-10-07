//! Runtime validation for the owner-backed dynamic API.
//!
//! These tests drive the final `Session`/`DynSessionCamera` boundary over the
//! suite's fake camera, which makes it possible to prove that profile
//! rejection and dynamic capability gates happen before a write.
//!
//! Every scenario is generic over the executor and runs under each enabled
//! runtime, as the cases `scenario::tokio` and `scenario::smol`. One fixture
//! serves both runtimes: a command is answered with ACK (and completion when
//! the fixture completes), an inquiry with a compact data reply, and a serial
//! fixture reports serial addressing and stream semantics while an IP fixture
//! reports datagram semantics.

#![cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    sync::Mutex,
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
    transport::{AddressingMode, SendSemantics, TransportConfig},
    CameraId, CancellationOutcome, Error, Executor, OperationalTuning, Session, SessionConfig,
    StateKey,
};

use fake_camera::{frames, AsyncWire, FakeCamera};

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

/// The fixture's fake camera. A command receives the normal ACK and, when
/// `complete`, completion; an inquiry receives a compact valid response. Each
/// reply comes from the addressed camera on a serial bus and from camera 1 on
/// IP.
fn probe_camera(addressing: AddressingMode, complete: bool) -> FakeCamera {
    FakeCamera::new(move |write, answer| {
        let target = write.first().copied().unwrap_or(0x81) & 0x0f;
        let source = match addressing {
            AddressingMode::Serial => 0x80 | (target.saturating_add(8) << 4),
            AddressingMode::Ip => 0x90,
        };
        let replies = if write.get(1) == Some(&0x09) {
            vec![frames::inquiry_reply(&[])]
        } else if complete {
            vec![frames::ack(1), frames::complete(1)]
        } else {
            vec![frames::ack(1)]
        };
        for mut reply in replies {
            reply[0] = source;
            answer.reply(reply);
        }
    })
}

/// A wire onto `camera` that reports `addressing` and the send semantics that
/// go with it: stream for serial, datagram for IP.
fn probe_wire(camera: &FakeCamera, addressing: AddressingMode) -> AsyncWire {
    let mut config = TransportConfig::default();
    config.addressing = addressing;
    let semantics = match addressing {
        AddressingMode::Serial => SendSemantics::Stream,
        AddressingMode::Ip => SendSemantics::Datagram,
    };
    camera
        .async_wire()
        .with_config(config)
        .with_addressing(addressing)
        .with_semantics(semantics)
}

/// A fake camera and the wire onto it. The camera records the writes.
fn probe(addressing: AddressingMode, complete: bool) -> (FakeCamera, AsyncWire) {
    let camera = probe_camera(addressing, complete);
    let wire = probe_wire(&camera, addressing);
    (camera, wire)
}

async fn open_session<E: Executor>(wire: AsyncWire, config: SessionConfig, executor: E) -> Session {
    Session::open(wire, config, executor)
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

async fn dynamic_nouns_preserve_targeted_applied_and_custom_lifecycles<E: Executor>(executor: E) {
    let (fake, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile()), executor).await;
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

    assert_eq!(fake.write_count(), 4);
    session.shutdown().expect("shutdown");
}

async fn dynamic_unsupported_gate_rejects_before_transport_io<E: Executor>(executor: E) {
    let (fake, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile()), executor).await;
    let camera = session.camera_dyn().expect("dynamic camera");

    let error = camera
        .zoom()
        .set_digital_zoom(true)
        .await
        .expect_err("PtzOptics G2 has no digital zoom toggle");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "digital zoom",
            ..
        }
    ));
    assert_eq!(fake.write_count(), 0);

    session.shutdown().expect("shutdown");
}

async fn dynamic_cache_views_share_state_and_isolate_targets<E: Executor>(executor: E) {
    let (_, transport) = probe(AddressingMode::Serial, true);
    let mut config = SessionConfig::new(profile());
    config
        .register_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config, executor.clone()).await;
    let first = session
        .camera_dyn_for(CameraId::CAMERA_1)
        .expect("camera 1 dynamic view");
    let first_view = session
        .camera_dyn_for(CameraId::CAMERA_1)
        .expect("same-target dynamic view");
    let second = session
        .camera_dyn_for(CameraId::CAMERA_2)
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
    session.shutdown().expect("shutdown");

    // A fresh owner receives a fresh fixed registry; state never leaks from
    // a previous session even when the profile is identical.
    let (_, fresh_transport) = probe(AddressingMode::Ip, true);
    let fresh = open_session(fresh_transport, SessionConfig::new(profile()), executor).await;
    let fresh_camera = fresh.camera_dyn().expect("fresh dynamic camera");
    assert_unknown(&fresh_camera);
    fresh.shutdown().expect("fresh shutdown");
}

async fn dynamic_target_selection_rejects_implicit_multi_target_view<E: Executor>(executor: E) {
    let (fake, transport) = probe(AddressingMode::Serial, true);
    let mut config = SessionConfig::new(profile());
    config
        .register_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config, executor).await;

    assert!(matches!(session.camera_dyn(), Err(Error::InvalidState(_))));
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
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");
}

async fn dynamic_queued_targeted_cancel_is_owner_local<E: Executor>(executor: E) {
    let (fake, transport) = probe(AddressingMode::Ip, false);
    let config = SessionConfig::new(profile())
        .with_tuning(OperationalTuning::new().maximum_command_sockets(1));
    let session = open_session(transport, config, executor.clone()).await;
    let camera = session.camera_dyn().expect("dynamic camera");
    let nouns: &dyn DynSessionCameraNouns = &camera;

    let first = nouns.zoom().stop().await.expect("first operation");
    fake.wait_for_writes_async(&executor, 1).await;
    assert_eq!(fake.write_count(), 1);

    let mut queued = nouns.pan_tilt().home().await.expect("queued operation");
    assert!(matches!(
        queued.cancel_with_timeout(Duration::from_secs(1)).await,
        Ok(CancellationOutcome::Cancelled)
    ));
    assert_eq!(fake.write_count(), 1);

    first.detach();
    session.shutdown().expect("shutdown");
}

async fn dynamic_future_construction_matches_one_explicit_static_box<E: Executor>(executor: E) {
    let (_, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile()), executor).await;
    let static_camera = session.camera::<PtzOpticsG2>().expect("static camera");
    let dynamic_camera = session.camera_dyn().expect("dynamic camera");
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

    session.shutdown().expect("shutdown");
}

async fn dynamic_noun_targeted_and_applied_handles_share_owner<E: Executor>(executor: E) {
    let (_, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile()), executor).await;
    let camera = session.camera_dyn().expect("dynamic camera");
    let nouns: &dyn DynSessionCameraNouns = &camera;

    nouns
        .zoom()
        .stop()
        .await
        .expect("applied operation admission")
        .applied()
        .await
        .expect("applied operation completion");
    nouns
        .pan_tilt()
        .home()
        .await
        .expect("targeted operation admission")
        .applied()
        .await
        .expect("targeted operation completion");

    session.shutdown().expect("shutdown");
}

async fn dynamic_motion_view_delegates_to_same_owner<E: Executor>(executor: E) {
    let (_, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile()), executor).await;
    let camera = session.camera_dyn().expect("dynamic camera");

    camera
        .motion()
        .stop_all_motion()
        .await
        .expect("motion safety delegation")
        .into_result()
        .expect("each supported STOP applied");
    session.shutdown().expect("shutdown");
}

async fn dynamic_unsupported_gate_and_target_selection_are_preflighted<E: Executor>(executor: E) {
    let (fake, first_transport) = probe(AddressingMode::Ip, true);
    let session = open_session(
        first_transport,
        SessionConfig::new(profile()),
        executor.clone(),
    )
    .await;
    let camera = session.camera_dyn().expect("dynamic camera");
    let error = camera
        .zoom()
        .set_digital_zoom(true)
        .await
        .expect_err("unsupported digital zoom must fail before I/O");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "digital zoom",
            ..
        }
    ));
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");

    let (fake, transport) = probe(AddressingMode::Serial, true);
    let mut config = SessionConfig::new(profile());
    config
        .register_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config, executor).await;
    assert!(matches!(session.camera_dyn(), Err(Error::InvalidState(_))));
    let selected = session
        .camera_dyn_for(CameraId::CAMERA_2)
        .expect("explicit target selection");
    assert_eq!(selected.target(), CameraId::CAMERA_2);
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");
}

async fn dynamic_cache_views_share_and_isolate_owner_state<E: Executor>(executor: E) {
    let (_, transport) = probe(AddressingMode::Serial, true);
    let mut config = SessionConfig::new(profile());
    config
        .register_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config, executor).await;
    let first = session
        .camera_dyn_for(CameraId::CAMERA_1)
        .expect("camera 1 view");
    let same = session
        .camera_dyn_for(CameraId::CAMERA_1)
        .expect("same-target view");
    let second = session
        .camera_dyn_for(CameraId::CAMERA_2)
        .expect("camera 2 view");
    assert_eq!(
        first.state_cache().value(StateKey::MulticastStreaming),
        StateEntry::Unknown
    );
    assert_eq!(
        second.state_cache().value(StateKey::MulticastStreaming),
        StateEntry::Unknown
    );
    first
        .advanced()
        .multicast_on()
        .await
        .expect("owner applied multicast state");
    assert!(matches!(
        first
            .state_cache()
            .value(StateKey::MulticastStreaming),
        StateEntry::Set(value) if value.get(0) == Some(1)
    ));
    assert!(matches!(
        same.state_cache()
            .value(StateKey::MulticastStreaming),
        StateEntry::Set(value) if value.get(0) == Some(1)
    ));
    assert_eq!(
        second.state_cache().value(StateKey::MulticastStreaming),
        StateEntry::Unknown
    );
    session.shutdown().expect("shutdown");
}

runtime_matrix!(
    dynamic_nouns_preserve_targeted_applied_and_custom_lifecycles,
    dynamic_unsupported_gate_rejects_before_transport_io,
    dynamic_cache_views_share_state_and_isolate_targets,
    dynamic_target_selection_rejects_implicit_multi_target_view,
    dynamic_queued_targeted_cancel_is_owner_local,
    dynamic_future_construction_matches_one_explicit_static_box,
    dynamic_noun_targeted_and_applied_handles_share_owner,
    dynamic_motion_view_delegates_to_same_owner,
    dynamic_unsupported_gate_and_target_selection_are_preflighted,
    dynamic_cache_views_share_and_isolate_owner_state,
);

// Keep the command imported in this binary so the cache test proves the
// canonical state effect through the same public request type used by static
// callers, even though the dynamic noun invokes it internally.
#[allow(dead_code)]
fn _multicast_streaming_type_is_public(_: MulticastStreaming) {}
