//! Public observability and owner-backed state-cache acceptance coverage.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{observability::MetricsSnapshot, ProfileSpec, SessionConfig};

#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
use grafton_visca::{
    state_cache::{StateCache, StateEntry},
    CameraId, StateKey,
};

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>().expect("profile")
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
        grafton_visca::SessionStatus::Running
    ));
}

#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
fn assert_unknown(cache: &StateCache, key: StateKey) {
    assert_eq!(cache.value(key), StateEntry::Unknown);
}

#[cfg(any(feature = "blocking", feature = "runtime-tokio"))]
fn assert_set(cache: &StateCache, key: StateKey, values: &[i64]) {
    match cache.value(key) {
        StateEntry::Set(value) => assert_eq!(value.as_slice(), values),
        other => panic!("expected Set({values:?}), got {other:?}"),
    }
}

#[cfg(feature = "blocking")]
fn assert_clear(cache: &StateCache, key: StateKey, values: &[i64]) {
    match cache.value(key) {
        StateEntry::Clear(value) => assert_eq!(value.as_slice(), values),
        other => panic!("expected Clear({values:?}), got {other:?}"),
    }
}

#[cfg(feature = "blocking")]
mod blocking_observability {
    use std::{collections::VecDeque, time::Duration};

    use grafton_visca::{
        blocking::Session,
        command::CommandKind,
        command::ImageFreeze,
        command::PanTiltLimitCorner,
        request::builtin::PanTiltLimitClear,
        transport::{
            AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        Error,
    };

    use super::*;

    #[derive(Debug)]
    struct Probe {
        config: TransportConfig,
        responses: VecDeque<Vec<u8>>,
        fail_send: bool,
    }

    impl Probe {
        fn new() -> Self {
            Self {
                config: TransportConfig::default(),
                responses: VecDeque::new(),
                fail_send: false,
            }
        }

        fn serial() -> Self {
            let mut probe = Self::new();
            probe.config.addressing = AddressingMode::Serial;
            probe
        }

        fn receive(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            let response = self.responses.pop_front().ok_or(Error::Timeout)?;
            dst[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }
    }

    impl HasTransportConfig for Probe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for Probe {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            if self.fail_send {
                return Err(Error::ConnectionClosed { reason: None });
            }
            let target = bytes.first().copied().unwrap_or(0x81) & 0x0f;
            let source = if self.config.addressing == AddressingMode::Serial {
                0x80 | (target.saturating_add(8) << 4)
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
    fn metrics_drain_bounds_and_cache_apply_only_after_terminal() {
        let session = Session::open(Probe::new(), SessionConfig::new(profile())).unwrap();
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .unwrap();
        let cache = camera.state_cache();
        assert_eq!(cache.target(), CameraId::CAMERA_1);
        assert_unknown(&cache, StateKey::ImageFreeze);
        assert_unknown(&cache, StateKey::PanTiltLimits);
        assert_metrics_shape(session.metrics().unwrap());

        assert!(session.drain_diagnostics().unwrap().is_empty());
        camera.execute(&ImageFreeze::on()).unwrap();
        assert_set(&cache, StateKey::ImageFreeze, &[1]);
        camera
            .execute(&PanTiltLimitClear::new(PanTiltLimitCorner::UpRight))
            .unwrap();
        assert_clear(
            &cache,
            StateKey::PanTiltLimits,
            &[PanTiltLimitCorner::UpRight.to_byte().into()],
        );
        let drained = session.drain_diagnostics().unwrap();
        assert!(drained.len() <= 128);
        assert!(session.drain_diagnostics().unwrap().is_empty());
        let metrics = session.metrics().unwrap();
        assert!(metrics.cache_updates >= 2);
        assert_eq!(metrics.received_frames, 4);
        session.shutdown().unwrap();
    }

    #[test]
    fn cache_views_share_same_target_isolate_targets_and_reset() {
        let mut config = SessionConfig::new(profile());
        config
            .register_target(CameraId::CAMERA_2, profile())
            .unwrap();
        let session = Session::open(Probe::serial(), config).unwrap();
        let first = session
            .camera_for::<grafton_visca::profiles::PtzOpticsG2>(CameraId::CAMERA_1)
            .unwrap();
        let first_view = first.state_cache();
        let first_clone = first_view.clone();
        let second_view = session
            .camera_for::<grafton_visca::profiles::PtzOpticsG2>(CameraId::CAMERA_2)
            .unwrap()
            .state_cache();
        assert_unknown(&first_view, StateKey::ImageFreeze);
        assert_unknown(&second_view, StateKey::ImageFreeze);
        first.execute(&ImageFreeze::on()).unwrap();
        assert_set(&first_view, StateKey::ImageFreeze, &[1]);
        assert_set(&first_clone, StateKey::ImageFreeze, &[1]);
        assert_unknown(&second_view, StateKey::ImageFreeze);
        session.shutdown().unwrap();

        let fresh = Session::open(Probe::new(), SessionConfig::new(profile())).unwrap();
        assert_unknown(
            &fresh
                .camera::<grafton_visca::profiles::PtzOpticsG2>()
                .unwrap()
                .state_cache(),
            StateKey::ImageFreeze,
        );
        fresh.shutdown().unwrap();
    }

    #[test]
    fn write_failure_does_not_update_cache() {
        let mut probe = Probe::new();
        probe.fail_send = true;
        let session = Session::open(probe, SessionConfig::new(profile())).unwrap();
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .unwrap();
        let cache = camera.state_cache();
        assert!(camera.execute(&ImageFreeze::on()).is_err());
        assert_unknown(&cache, StateKey::ImageFreeze);
        session.shutdown().unwrap();
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod tokio_observability {
    use std::{future::Future, sync::Arc, time::Duration};

    use grafton_visca::{
        command::ImageFreeze,
        runtime::TokioRuntime,
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        Error, Session,
    };

    use super::*;

    #[derive(Debug)]
    struct Probe {
        config: TransportConfig,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
        fail_send: bool,
    }

    impl Probe {
        fn new() -> Self {
            let (response_tx, responses) = flume::unbounded();
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx,
                fail_send: false,
            }
        }

        fn serial() -> Self {
            let mut probe = Self::new();
            probe.config.addressing = AddressingMode::Serial;
            probe
        }
    }

    impl HasTransportConfig for Probe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for Probe {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let tx = self.response_tx.clone();
            let target = bytes.first().copied().unwrap_or(0x81) & 0x0f;
            let source = if self.config.addressing == AddressingMode::Serial {
                0x80 | (target.saturating_add(8) << 4)
            } else {
                0x90
            };
            let fail_send = self.fail_send;
            async move {
                if fail_send {
                    return Err(Error::ConnectionClosed { reason: None });
                }
                tx.send_async(vec![source, 0x41, 0xff])
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                tx.send_async(vec![source, 0x51, 0xff])
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
                    .responses
                    .recv_async()
                    .await
                    .map_err(|_| Error::RuntimeShutdown)?;
                dst[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
        }

        fn send_semantics(&self) -> SendSemantics {
            if self.config.addressing == AddressingMode::Serial {
                SendSemantics::Stream
            } else {
                SendSemantics::Datagram
            }
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(self.config.addressing)
        }
    }

    async fn session() -> Session {
        Session::open(
            Probe::new(),
            SessionConfig::new(profile()),
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
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .unwrap();
        camera.execute(&ImageFreeze::on()).await.unwrap();
        camera.execute(&ImageFreeze::off()).await.unwrap();
        let snapshot = session.metrics().await.unwrap();
        assert!(snapshot.dropped_diagnostic_events > 0);
        assert_eq!(snapshot.received_frames, 4);
        assert!(fast.try_recv().is_some());
        assert!(slow.try_recv().is_some());
        camera.execute(&ImageFreeze::on()).await.unwrap();
        session.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn public_events_are_sanitized_and_shutdown_rejects_work() {
        let session = session().await;
        let subscription = session.subscribe_diagnostics(8).await.unwrap();
        session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .unwrap()
            .execute(&ImageFreeze::on())
            .await
            .unwrap();
        let event = subscription.try_recv().expect("diagnostic event");
        assert!(matches!(
            event,
            grafton_visca::DiagnosticEvent::Admitted { .. }
                | grafton_visca::DiagnosticEvent::FrameReceived { .. }
                | grafton_visca::DiagnosticEvent::Transition { .. }
                | grafton_visca::DiagnosticEvent::WriteFinished { .. }
        ));
        session.shutdown().await.unwrap();
        assert!(session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .unwrap()
            .execute(&ImageFreeze::off())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn empty_subscription_disconnects_after_shutdown() {
        let session = session().await;
        let subscription = session.subscribe_diagnostics(1).await.unwrap();
        session.shutdown().await.unwrap();
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
        config
            .register_target(CameraId::CAMERA_2, profile())
            .unwrap();
        let session = Session::open(
            Probe::serial(),
            config,
            TokioRuntime::from_current().unwrap(),
        )
        .await
        .unwrap();
        let first = session
            .camera_for::<grafton_visca::profiles::PtzOpticsG2>(CameraId::CAMERA_1)
            .unwrap();
        let view = first.state_cache();
        let second = session
            .camera_for::<grafton_visca::profiles::PtzOpticsG2>(CameraId::CAMERA_2)
            .unwrap()
            .state_cache();
        assert_unknown(&view, StateKey::ImageFreeze);
        assert_unknown(&second, StateKey::ImageFreeze);
        first.execute(&ImageFreeze::on()).await.unwrap();
        assert_set(&view, StateKey::ImageFreeze, &[1]);
        assert_unknown(&second, StateKey::ImageFreeze);
        session.shutdown().await.unwrap();

        let mut failed = Probe::new();
        failed.fail_send = true;
        let failed_session = Session::open(
            failed,
            SessionConfig::new(profile()),
            TokioRuntime::from_current().unwrap(),
        )
        .await
        .unwrap();
        let camera = failed_session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .unwrap();
        let cache = camera.state_cache();
        assert!(camera.execute(&ImageFreeze::on()).await.is_err());
        assert_unknown(&cache, StateKey::ImageFreeze);
        failed_session.shutdown().await.unwrap();
    }

    #[test]
    fn api_types_are_mode_native_and_send_safe() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<grafton_visca::MetricsSnapshot>();
        assert_send_sync::<grafton_visca::DiagnosticSubscription>();
        assert_send_sync::<grafton_visca::StateCache>();
        let _ = Arc::new(1_u8);
    }
}

#[cfg(all(feature = "async", feature = "runtime-smol"))]
mod smol_observability {
    use std::future::Future;

    use grafton_visca::{
        runtime::SmolRuntime,
        transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
        Error, Session,
    };

    use super::*;

    #[derive(Debug)]
    struct Probe {
        config: TransportConfig,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
    }

    impl Probe {
        fn new() -> Self {
            let (response_tx, responses) = flume::unbounded();
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx,
            }
        }
    }

    impl HasTransportConfig for Probe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for Probe {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let tx = self.response_tx.clone();
            let target = bytes.first().copied().unwrap_or(0x81) & 0x0f;
            let source = 0x80 | (target.saturating_add(8) << 4);
            async move {
                tx.send_async(vec![source, 0x41, 0xff]).await.unwrap();
                tx.send_async(vec![source, 0x51, 0xff]).await.unwrap();
                Ok(())
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn recv_into<'a>(
            &'a mut self,
            dst: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            async move {
                let bytes = self.responses.recv_async().await.unwrap();
                dst[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[test]
    fn smol_metrics_and_subscription_bounds() {
        smol::block_on(async {
            let session = Session::open(
                Probe::new(),
                SessionConfig::new(profile()),
                SmolRuntime::new(),
            )
            .await
            .unwrap();
            assert_metrics_shape(session.metrics().await.unwrap());
            assert!(session.subscribe_diagnostics(0).await.is_err());
            assert!(session.subscribe_diagnostics(129).await.is_err());
            let subscription = session.subscribe_diagnostics(1).await.unwrap();
            session
                .camera::<grafton_visca::profiles::PtzOpticsG2>()
                .unwrap()
                .execute(&grafton_visca::command::ImageFreeze::on())
                .await
                .unwrap();
            assert!(subscription.try_recv().is_some());
            session.shutdown().await.unwrap();
        });
    }
}
