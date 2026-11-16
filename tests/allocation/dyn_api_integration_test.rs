//! Runtime validation for the owner-backed dynamic API.
//!
//! These tests drive the final `Session`/`grafton_visca::r#async::Camera` boundary over the
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
    any(feature = "async", feature = "blocking"),
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use grafton_visca_test_support::{fake_camera, runtime_matrix};

use std::time::Duration;

use grafton_visca::{
    capabilities::TypedSupportSurface,
    config::{OperationalTuning, SessionConfig},
    operation::CancellationOutcome,
    profiles::{ProfileSpec, PtzOpticsG2},
    r#async::Session,
    runtime::Executor,
    state_cache::{StateEntry, StateKey},
    transport::{AddressingMode, SendSemantics, TransportConfig},
    types::CameraId,
    Error,
};

use fake_camera::{frames, AsyncWire, FakeCamera};

use crate::counting::allocations_during;

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
    let config = TransportConfig::default();

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
    Session::open(wire, &config, executor)
        .await
        .expect("owner session")
}

fn assert_unknown(camera: &grafton_visca::r#async::Camera) {
    assert_eq!(
        camera.state_cache().value(StateKey::StreamingMulticast),
        StateEntry::Unknown
    );
}

fn assert_state(camera: &grafton_visca::r#async::Camera, expected: bool) {
    match camera.state_cache().value(StateKey::StreamingMulticast) {
        StateEntry::Set(value) => assert_eq!(
            value,
            grafton_visca::state_cache::KnownState::StreamingMulticast(expected)
        ),
        other => panic!("expected multicast state {expected:?}, got {other:?}"),
    }
}

async fn dynamic_nouns_preserve_targeted_applied_and_custom_lifecycles<E: Executor>(executor: E) {
    let (fake, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile()), executor).await;
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("dynamic camera");
    let root = &camera;
    let nouns = &camera;

    assert_eq!(root.target(), CameraId::CAMERA_1);
    assert!(root.capabilities().zoom.is_some());
    assert!(!root
        .capabilities()
        .supports(TypedSupportSurface::DigitalZoomToggle));

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
    let targeted = &grafton_visca::command::pan_tilt::Home;
    root.submit(targeted)
        .await
        .expect("custom targeted admission")
        .applied()
        .await
        .expect("custom targeted applied lifecycle");
    let applied = &grafton_visca::command::zoom::Stop;
    root.submit(applied)
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
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("dynamic camera");

    let error = camera
        .zoom()
        .set_digital_zoom(true)
        .await
        .expect_err("PtzOptics G2 has no digital zoom toggle");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "set_digital_zoom",
            ..
        }
    ));
    assert_eq!(fake.write_count(), 0);

    session.shutdown().expect("shutdown");
}

/// Removing tally permissions denies the dynamic noun before I/O.
async fn dynamic_tally_metadata_does_not_grant_typed_permission<E: Executor>(executor: E) {
    use grafton_visca::profiles::SonyFr7;

    let source = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7");
    let mut capabilities = source.capabilities().clone();
    capabilities.profile_id = None;
    capabilities.typed_support = capabilities
        .typed_support
        .without(TypedSupportSurface::Tally)
        .without(TypedSupportSurface::TallyBrightness)
        .without(TypedSupportSurface::TallyMode);
    let profile = crate::profile_encodings::with_source_encodings(
        ProfileSpec::builder(capabilities)
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .position_inquiries(source.position_inquiries()),
        &source,
    )
    .build()
    .expect("profile without tally permission is representable");
    assert!(!profile.capabilities().supports(TypedSupportSurface::Tally));

    let (fake, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile), executor).await;
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("dynamic camera");
    let tally = camera.tally();
    for result in [
        tally.set_red(true).await,
        tally.set_red(false).await,
        tally.set_green(true).await,
        tally.set_green(false).await,
        tally
            .set_brightness(grafton_visca::types::TallyBrightness::Low)
            .await,
        tally
            .set_brightness(grafton_visca::types::TallyBrightness::High)
            .await,
        tally.set_mode(grafton_visca::types::TallyMode::Flash).await,
        tally.set_mode(grafton_visca::types::TallyMode::On).await,
        tally.set_mode(grafton_visca::types::TallyMode::Off).await,
        tally.red().await.map(|_| ()),
        tally.green().await.map(|_| ()),
    ] {
        assert!(
            matches!(result, Err(Error::FeatureNotSupported { .. })),
            "{result:?}"
        );
    }
    assert_eq!(
        fake.write_count(),
        0,
        "all tally rows reject before transport I/O"
    );
    session.shutdown().expect("shutdown");
}

/// Refining grants require a valid base grant before a session can open.
#[test]
fn tally_refinements_without_the_base_grant_are_refused() {
    use grafton_visca::{capabilities::TypedSupportSet, profiles::SonyFr7};

    let source = ProfileSpec::from_compile_time::<SonyFr7>().expect("FR7");
    for refinement in [
        TypedSupportSurface::TallyBrightness,
        TypedSupportSurface::TallyMode,
    ] {
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.typed_support = capabilities
            .typed_support
            .without(TypedSupportSurface::Tally)
            .without(TypedSupportSurface::TallyBrightness)
            .without(TypedSupportSurface::TallyMode)
            .union(TypedSupportSet::from_surfaces(&[refinement]));
        let error = crate::profile_encodings::with_source_encodings(
            ProfileSpec::builder(capabilities)
                .transports(source.transports())
                .envelope(source.envelope())
                .timing(source.timing())
                .maximum_command_sockets(source.maximum_command_sockets())
                .supports_operation_complete(source.supports_operation_complete())
                .supports_command_cancel(source.supports_command_cancel())
                .position_inquiries(source.position_inquiries()),
            &source,
        )
        .build()
        .expect_err("orphan tally refinement must fail before session creation");
        assert!(
            matches!(&error, Error::InvalidRequest { reason: message }
                if message.contains(&format!("typed support {refinement:?}"))),
            "{refinement:?}: {error:?}"
        );
    }
}

async fn dynamic_cache_views_share_state_and_isolate_targets<E: Executor>(executor: E) {
    let (_, transport) = probe(AddressingMode::Serial, true);
    let mut config = SessionConfig::new(profile());
    config = config
        .with_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config, executor.clone()).await;
    let first = session
        .camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_1)
        .expect("camera 1 dynamic view");
    let first_view = session
        .camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_1)
        .expect("same-target dynamic view");
    let second = session
        .camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_2)
        .expect("camera 2 dynamic view");

    assert_unknown(&first);
    assert_unknown(&second);
    first
        .streaming()
        .set_multicast(true)
        .await
        .expect("exact applied multicast effect");
    assert_state(&first, true);
    assert_state(&first_view, true);
    assert_unknown(&second);

    first
        .streaming()
        .set_multicast(false)
        .await
        .expect("exact applied multicast clear effect");
    assert_state(&first, false);
    assert_unknown(&second);
    session.shutdown().expect("shutdown");

    // A fresh owner receives a fresh fixed registry; state never leaks from
    // a previous session even when the profile is identical.
    let (_, fresh_transport) = probe(AddressingMode::Ip, true);
    let fresh = open_session(fresh_transport, SessionConfig::new(profile()), executor).await;
    let fresh_camera = fresh
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("fresh dynamic camera");
    assert_unknown(&fresh_camera);
    fresh.shutdown().expect("fresh shutdown");
}

async fn runtime_profile_target_selection_is_explicit_and_checked<E: Executor>(executor: E) {
    let (fake, transport) = probe(AddressingMode::Serial, true);
    let mut config = SessionConfig::new(profile());
    config = config
        .with_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config, executor).await;

    assert_eq!(
        session
            .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
            .expect("explicit registered target")
            .target(),
        CameraId::CAMERA_1
    );
    let selected = session
        .camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_2)
        .expect("explicit dynamic target");
    assert_eq!(selected.target(), CameraId::CAMERA_2);
    let _ = selected
        .clone()
        .into_typed::<PtzOpticsG2>()
        .expect("matching static projection");
    assert_eq!(
        session
            .camera::<PtzOpticsG2>(grafton_visca::types::CameraId::CAMERA_1)
            .expect("explicit registered target")
            .target(),
        CameraId::CAMERA_1
    );
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");
}

async fn dynamic_queued_targeted_cancel_is_owner_local<E: Executor>(executor: E) {
    let (fake, transport) = probe(AddressingMode::Ip, false);
    let config = SessionConfig::new(profile())
        .with_tuning(OperationalTuning::new().with_maximum_command_sockets(1));
    let session = open_session(transport, config, executor.clone()).await;
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("dynamic camera");
    let nouns = &camera;

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

async fn runtime_profile_future_construction_matches_static_without_allocating<E: Executor>(
    executor: E,
) {
    let (_, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile()), executor).await;
    let static_camera = session
        .camera::<PtzOpticsG2>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("static camera");
    let dynamic_camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("dynamic camera");
    let nouns = &dynamic_camera;
    let static_zoom = static_camera.zoom();
    let dynamic_zoom = nouns.zoom();

    let static_allocations = allocations_during(|| {
        // Build and drop the native future without polling or transport I/O.
        let future = static_zoom.stop();
        std::mem::drop(std::hint::black_box(future));
    });
    let dynamic_allocations = allocations_during(|| {
        let future = dynamic_zoom.stop();
        std::mem::drop(std::hint::black_box(future));
    });
    assert_eq!(
        static_allocations, 0,
        "native future construction must not allocate"
    );
    assert_eq!(
        dynamic_allocations, static_allocations,
        "runtime-profile future allocation drift: runtime={dynamic_allocations}, static={static_allocations}"
    );

    session.shutdown().expect("shutdown");
}

async fn dynamic_noun_targeted_and_applied_handles_share_owner<E: Executor>(executor: E) {
    let (_, transport) = probe(AddressingMode::Ip, true);
    let session = open_session(transport, SessionConfig::new(profile()), executor).await;
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("dynamic camera");
    let nouns = &camera;

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
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("dynamic camera");

    camera
        .motion()
        .stop_all()
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
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
        .expect("dynamic camera");
    let error = camera
        .zoom()
        .set_digital_zoom(true)
        .await
        .expect_err("unsupported digital zoom must fail before I/O");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "set_digital_zoom",
            ..
        }
    ));
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");

    let (fake, transport) = probe(AddressingMode::Serial, true);
    let mut config = SessionConfig::new(profile());
    config = config
        .with_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config, executor).await;
    assert_eq!(
        session
            .camera::<grafton_visca::profiles::AnyProfile>(grafton_visca::types::CameraId::CAMERA_1)
            .expect("explicit registered target")
            .target(),
        CameraId::CAMERA_1
    );
    let selected = session
        .camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_2)
        .expect("explicit target selection");
    assert_eq!(selected.target(), CameraId::CAMERA_2);
    assert_eq!(fake.write_count(), 0);
    session.shutdown().expect("shutdown");
}

async fn dynamic_cache_views_share_and_isolate_owner_state<E: Executor>(executor: E) {
    let (_, transport) = probe(AddressingMode::Serial, true);
    let mut config = SessionConfig::new(profile());
    config = config
        .with_target(CameraId::CAMERA_2, profile())
        .expect("camera 2 registration");
    let session = open_session(transport, config, executor).await;
    let first = session
        .camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_1)
        .expect("camera 1 view");
    let same = session
        .camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_1)
        .expect("same-target view");
    let second = session
        .camera::<grafton_visca::profiles::AnyProfile>(CameraId::CAMERA_2)
        .expect("camera 2 view");
    assert_eq!(
        first.state_cache().value(StateKey::StreamingMulticast),
        StateEntry::Unknown
    );
    assert_eq!(
        second.state_cache().value(StateKey::StreamingMulticast),
        StateEntry::Unknown
    );
    first
        .streaming()
        .set_multicast(true)
        .await
        .expect("owner applied multicast state");
    assert!(matches!(
        first.state_cache().value(StateKey::StreamingMulticast),
        StateEntry::Set(grafton_visca::state_cache::KnownState::StreamingMulticast(
            true
        ))
    ));
    assert!(matches!(
        same.state_cache().value(StateKey::StreamingMulticast),
        StateEntry::Set(grafton_visca::state_cache::KnownState::StreamingMulticast(
            true
        ))
    ));
    assert_eq!(
        second.state_cache().value(StateKey::StreamingMulticast),
        StateEntry::Unknown
    );
    session.shutdown().expect("shutdown");
}

runtime_matrix!(
    dynamic_nouns_preserve_targeted_applied_and_custom_lifecycles,
    dynamic_unsupported_gate_rejects_before_transport_io,
    dynamic_tally_metadata_does_not_grant_typed_permission,
    dynamic_cache_views_share_state_and_isolate_targets,
    runtime_profile_target_selection_is_explicit_and_checked,
    dynamic_queued_targeted_cancel_is_owner_local,
    runtime_profile_future_construction_matches_static_without_allocating,
    dynamic_noun_targeted_and_applied_handles_share_owner,
    dynamic_motion_view_delegates_to_same_owner,
    dynamic_unsupported_gate_and_target_selection_are_preflighted,
    dynamic_cache_views_share_and_isolate_owner_state,
);

// Keep the command imported in this binary so the cache test proves the
// canonical state effect through the same public request type used by static
// callers, even though the dynamic noun invokes it internally.
fn _multicast_streaming_setting_is_boolean(_: bool) {}
