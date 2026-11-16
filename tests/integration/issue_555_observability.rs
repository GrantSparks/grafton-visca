//! Public observability and owner-backed state-cache acceptance coverage.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca_test_support::fake_camera;

use fake_camera::FakeCamera;
use grafton_visca::{config::SessionConfig, observability::MetricsSnapshot, profiles::ProfileSpec};

#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
use grafton_visca::{
    transport::{AddressingMode, SendSemantics, TransportConfig},
    Error,
};

#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
use grafton_visca::{
    state_cache::{StateCache, StateEntry, StateKey},
    types::CameraId,
};

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>().expect("profile")
}

/// A camera that answers every write with an ACK and a completion. A serial
/// camera addresses its replies to the writing target; otherwise they come
/// from source `0x90`. With `fail_send`, every write fails and nothing is
/// answered.
#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
fn probe_camera(serial: bool, fail_send: bool) -> FakeCamera {
    FakeCamera::new(move |write, answer| {
        if fail_send {
            answer.fail_send(Error::connection_closed(None));
            return;
        }
        let target = write.first().copied().unwrap_or(0x81) & 0x0f;
        let source = if serial {
            0x80 | (target.saturating_add(8) << 4)
        } else {
            0x90
        };
        answer
            .reply(vec![source, 0x41, 0xff])
            .reply(vec![source, 0x51, 0xff]);
    })
}

/// The transport configuration of a probe: serial addressing for a serial
/// camera, the default otherwise.
#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
fn probe_config(_serial: bool) -> TransportConfig {
    TransportConfig::default()
}

/// A serial camera sends as a stream; otherwise as datagrams.
#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
fn probe_semantics(serial: bool) -> SendSemantics {
    if serial {
        SendSemantics::Stream
    } else {
        SendSemantics::Datagram
    }
}

fn assert_metrics_shape(snapshot: MetricsSnapshot) {
    assert_eq!(snapshot.active, 0);
    assert_eq!(snapshot.pending, 0);
    assert_eq!(snapshot.admitted, 0);
    assert_eq!(snapshot.terminal, 0);
    assert_eq!(snapshot.cache_updates, 0);
    // Issue #571: the field-debugging counters are part of the published
    // snapshot, and an idle session reports every one of them at zero.
    assert_eq!(snapshot.ack_timeouts, 0);
    assert_eq!(snapshot.completion_timeouts, 0);
    assert_eq!(snapshot.inquiry_timeouts, 0);
    assert_eq!(snapshot.busy_errors, 0);
    assert_eq!(snapshot.protocol_errors, 0);
    assert_eq!(snapshot.retries_scheduled, 0);
    assert_eq!(snapshot.received_frames, 0);
    assert_eq!(snapshot.ignored_unmatched_sequenced_replies, 0);
    assert_eq!(snapshot.ignored_malformed_frames, 0);
    assert!(matches!(
        snapshot.session,
        grafton_visca::observability::SessionStatus::Running
    ));
}

#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
fn assert_unknown(cache: &StateCache, key: StateKey) {
    assert_eq!(cache.value(key), StateEntry::Unknown);
}

#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
fn assert_set(cache: &StateCache, value: grafton_visca::state_cache::KnownState) {
    assert_eq!(cache.value(value.key()), StateEntry::Set(value));
}

#[cfg(feature = "blocking")]
mod blocking_observability {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use grafton_visca::{blocking::Session, types::PanTiltLimitCorner};

    use super::*;
    use fake_camera::BlockingWire;

    /// A blocking transport onto a probe camera.
    fn probe_wire(serial: bool, fail_send: bool) -> BlockingWire {
        let config = probe_config(serial);
        probe_camera(serial, fail_send)
            .blocking_wire()
            .with_addressing(if serial {
                AddressingMode::Serial
            } else {
                AddressingMode::Ip
            })
            .with_semantics(probe_semantics(serial))
            .with_config(config)
    }

    #[test]
    fn metrics_diagnostics_bounds_and_cache_apply_only_after_terminal() {
        let session =
            Session::open(probe_wire(false, false), &SessionConfig::new(profile())).unwrap();
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .unwrap();
        let cache = camera.state_cache();
        assert_eq!(cache.target(), CameraId::CAMERA_1);
        assert_unknown(&cache, StateKey::PanTiltLimit);
        assert_metrics_shape(session.metrics().unwrap());

        let diagnostics = session.subscribe_diagnostics(128).unwrap();
        assert!(diagnostics.try_recv().is_none());
        camera.streaming().set_multicast(true).unwrap();
        assert_set(
            &cache,
            grafton_visca::state_cache::KnownState::StreamingMulticast(true),
        );
        camera
            .execute(
                &grafton_visca::command::pan_tilt::ClearLimit::for_target(
                    &grafton_visca::config::TargetRegistration::new(
                        grafton_visca::types::IndividualCameraId::CAMERA_1,
                        grafton_visca::profiles::ProfileSpec::from_compile_time::<
                            grafton_visca::profiles::PtzOpticsG2,
                        >()
                        .expect("source profile"),
                    )
                    .expect("target registration"),
                    PanTiltLimitCorner::UpRight,
                )
                .expect("profile-safe limit clear"),
            )
            .unwrap();
        assert_set(
            &cache,
            grafton_visca::state_cache::KnownState::PanTiltLimit(
                grafton_visca::state_cache::PanTiltLimitUpdate::clear(PanTiltLimitCorner::UpRight),
            ),
        );
        let received = std::iter::from_fn(|| diagnostics.try_recv()).count();
        assert!((1..=128).contains(&received));
        assert!(diagnostics.try_recv().is_none());
        let metrics = session.metrics().unwrap();
        assert!(metrics.cache_updates >= 2);
        assert_eq!(metrics.received_frames, 4);
        session.shutdown().unwrap();
    }

    #[test]
    fn cache_views_share_same_target_isolate_targets_and_reset() {
        let mut config = SessionConfig::new(profile());
        config = config
            .with_target(
                grafton_visca::types::IndividualCameraId::CAMERA_2,
                profile(),
            )
            .unwrap();
        let session = Session::open(probe_wire(true, false), &config).unwrap();
        let first = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .unwrap();
        let first_view = first.state_cache();
        let first_clone = first_view.clone();
        let second_view = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_2,
            )
            .unwrap()
            .state_cache();
        assert_unknown(&first_view, StateKey::StreamingMulticast);
        assert_unknown(&second_view, StateKey::StreamingMulticast);
        first.streaming().set_multicast(true).unwrap();
        assert_set(
            &first_view,
            grafton_visca::state_cache::KnownState::StreamingMulticast(true),
        );
        assert_set(
            &first_clone,
            grafton_visca::state_cache::KnownState::StreamingMulticast(true),
        );
        assert_unknown(&second_view, StateKey::StreamingMulticast);
        session.shutdown().unwrap();

        let fresh =
            Session::open(probe_wire(false, false), &SessionConfig::new(profile())).unwrap();
        assert_unknown(
            &fresh
                .camera::<grafton_visca::profiles::PtzOpticsG2>(
                    grafton_visca::types::IndividualCameraId::CAMERA_1,
                )
                .unwrap()
                .state_cache(),
            StateKey::StreamingMulticast,
        );
        fresh.shutdown().unwrap();
    }

    #[test]
    fn write_failure_does_not_update_cache() {
        let session =
            Session::open(probe_wire(false, true), &SessionConfig::new(profile())).unwrap();
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .unwrap();
        let cache = camera.state_cache();
        assert!(camera.streaming().set_multicast(true).is_err());
        assert_unknown(&cache, StateKey::StreamingMulticast);
        session.shutdown().unwrap();
    }

    #[test]
    fn metrics_are_observable_while_another_thread_waits_on_a_request() {
        let session = Session::open(
            // A camera that accepts every write and never answers, so a
            // request stays in flight for as long as the test needs.
            FakeCamera::silent()
                .blocking_wire()
                .with_semantics(SendSemantics::Stream),
            &SessionConfig::new(profile()),
        )
        .expect("open blocking session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("camera");

        thread::scope(|scope| {
            let request = scope.spawn(|| camera.streaming().set_multicast(true));
            let deadline = Instant::now() + Duration::from_secs(2);
            let metrics = loop {
                let started = Instant::now();
                let metrics = session.metrics().expect("metrics");
                assert!(
                    started.elapsed() < Duration::from_millis(100),
                    "metrics must not wait behind another caller's request"
                );
                if metrics.writes == 1 {
                    break metrics;
                }
                assert!(Instant::now() < deadline, "the request was never written");
                thread::sleep(Duration::from_millis(1));
            };
            assert_eq!(metrics.active, 1);

            session.shutdown().expect("shutdown");
            assert!(request.join().expect("request thread panicked").is_err());
        });
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod tokio_observability {
    use std::{sync::Arc, time::Duration};

    use grafton_visca::{r#async::Session, runtime::TokioRuntime};

    use super::*;
    use fake_camera::AsyncWire;

    /// An async transport onto a probe camera.
    fn probe_wire(serial: bool, fail_send: bool) -> AsyncWire {
        let config = probe_config(serial);
        probe_camera(serial, fail_send)
            .async_wire()
            .with_addressing(if serial {
                AddressingMode::Serial
            } else {
                AddressingMode::Ip
            })
            .with_semantics(probe_semantics(serial))
            .with_config(config)
    }

    async fn session() -> Session {
        Session::open(
            probe_wire(false, false),
            &SessionConfig::new(profile()),
            TokioRuntime::from_current().unwrap(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn metrics_and_subscription_bounds_and_slow_subscriber() {
        let session = session().await;
        assert_metrics_shape(session.metrics().await.unwrap());
        assert!(session.subscribe_diagnostics(0).await.is_err());
        assert!(session.subscribe_diagnostics(129).await.is_err());
        let mut subscriptions = Vec::new();
        for _ in 0..4 {
            subscriptions.push(session.subscribe_diagnostics(1).await.unwrap());
        }
        assert!(session.subscribe_diagnostics(1).await.is_err());
        drop(subscriptions.pop());
        let _reclaimed = session.subscribe_diagnostics(1).await.unwrap();
        drop(subscriptions);
        drop(_reclaimed);

        let slow = session.subscribe_diagnostics(1).await.unwrap();
        let fast = session.subscribe_diagnostics(16).await.unwrap();
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .unwrap();
        camera.streaming().set_multicast(true).await.unwrap();
        camera.streaming().set_multicast(false).await.unwrap();
        let snapshot = session.metrics().await.unwrap();
        assert!(snapshot.dropped_diagnostic_events > 0);
        assert_eq!(snapshot.received_frames, 4);
        assert!(fast.try_recv().is_some());
        assert!(slow.try_recv().is_some());
        camera.streaming().set_multicast(true).await.unwrap();
        session.shutdown().unwrap();
    }

    #[tokio::test]
    async fn public_events_are_sanitized_and_shutdown_rejects_work() {
        let session = session().await;
        let subscription = session.subscribe_diagnostics(8).await.unwrap();
        session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .unwrap()
            .streaming()
            .set_multicast(true)
            .await
            .unwrap();
        let event = subscription.try_recv().expect("diagnostic event");
        assert!(matches!(
            event,
            grafton_visca::observability::DiagnosticEvent::Admitted { .. }
                | grafton_visca::observability::DiagnosticEvent::FrameReceived { .. }
                | grafton_visca::observability::DiagnosticEvent::Transition { .. }
                | grafton_visca::observability::DiagnosticEvent::WriteFinished { .. }
        ));
        session.shutdown().unwrap();
        assert!(session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1
            )
            .unwrap()
            .streaming()
            .set_multicast(false)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn empty_subscription_disconnects_after_shutdown() {
        let session = session().await;
        let subscription = session.subscribe_diagnostics(1).await.unwrap();
        session.shutdown().unwrap();
        let mut disconnected = false;
        for _ in 0..2 {
            let result = tokio::time::timeout(Duration::from_secs(1), subscription.recv())
                .await
                .expect("diagnostic receiver should not stall after shutdown");
            if matches!(result, Err(grafton_visca::Error::RuntimeShutdown)) {
                disconnected = true;
                break;
            }
        }
        assert!(disconnected);
    }

    #[tokio::test]
    async fn canonical_cache_is_target_local_and_write_failure_stays_unknown() {
        let mut config = SessionConfig::new(profile());
        config = config
            .with_target(
                grafton_visca::types::IndividualCameraId::CAMERA_2,
                profile(),
            )
            .unwrap();
        let session = Session::open(
            probe_wire(true, false),
            &config,
            TokioRuntime::from_current().unwrap(),
        )
        .await
        .unwrap();
        let first = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .unwrap();
        let view = first.state_cache();
        let second = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_2,
            )
            .unwrap()
            .state_cache();
        assert_unknown(&view, StateKey::StreamingMulticast);
        assert_unknown(&second, StateKey::StreamingMulticast);
        first.streaming().set_multicast(true).await.unwrap();
        assert_set(
            &view,
            grafton_visca::state_cache::KnownState::StreamingMulticast(true),
        );
        assert_unknown(&second, StateKey::StreamingMulticast);
        session.shutdown().unwrap();

        let failed_session = Session::open(
            probe_wire(false, true),
            &SessionConfig::new(profile()),
            TokioRuntime::from_current().unwrap(),
        )
        .await
        .unwrap();
        let camera = failed_session
            .camera::<grafton_visca::profiles::PtzOpticsG2>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .unwrap();
        let cache = camera.state_cache();
        assert!(camera.streaming().set_multicast(true).await.is_err());
        assert_unknown(&cache, StateKey::StreamingMulticast);
        failed_session.shutdown().unwrap();
    }

    #[test]
    fn api_types_are_mode_native_and_send_safe() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<grafton_visca::observability::MetricsSnapshot>();
        assert_send_sync::<grafton_visca::r#async::Diagnostics>();
        assert_send_sync::<grafton_visca::state_cache::StateCache>();
        let _ = Arc::new(1_u8);
    }
}

#[cfg(all(feature = "async", feature = "runtime-smol"))]
mod smol_observability {
    use grafton_visca::{r#async::Session, runtime::SmolRuntime};

    use super::*;

    /// An async camera that answers every write with an ACK and a completion
    /// from the writing target.
    fn probe_camera() -> FakeCamera {
        FakeCamera::new(|write, answer| {
            let target = write.first().copied().unwrap_or(0x81) & 0x0f;
            let source = 0x80 | (target.saturating_add(8) << 4);
            answer
                .reply(vec![source, 0x41, 0xff])
                .reply(vec![source, 0x51, 0xff]);
        })
    }

    #[test]
    fn smol_metrics_and_subscription_bounds() {
        smol::block_on(async {
            let session = Session::open(
                probe_camera().async_wire(),
                &SessionConfig::new(profile()),
                SmolRuntime::new(),
            )
            .await
            .unwrap();
            assert_metrics_shape(session.metrics().await.unwrap());
            assert!(session.subscribe_diagnostics(0).await.is_err());
            assert!(session.subscribe_diagnostics(129).await.is_err());
            let subscription = session.subscribe_diagnostics(1).await.unwrap();
            session
                .camera::<grafton_visca::profiles::PtzOpticsG2>(
                    grafton_visca::types::IndividualCameraId::CAMERA_1,
                )
                .unwrap()
                .streaming()
                .set_multicast(true)
                .await
                .unwrap();
            assert!(subscription.try_recv().is_some());
            session.shutdown().unwrap();
        });
    }
}
