//! Issue #564: `Error::requires_new_session()` classification and the
//! documented poison/close → rebuild recovery recipe.
//!
//! Issue #542 makes transport close, explicit shutdown, and poison distinct
//! terminal errors that all share `ErrorKind::IoClosed`. These tests pin the
//! public classification that separates them, and they exercise the recovery
//! recipe end to end: a terminal session is abandoned, a fresh session is built
//! from the same reusable `SessionConfig`, and a command is driven through it.

#![cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]

use grafton_visca::{ProfileSpec, SessionConfig};

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>().expect("profile")
}

#[cfg(feature = "blocking")]
mod blocking_recovery {
    use std::{collections::VecDeque, io, sync::Arc, time::Duration};

    use grafton_visca::{
        blocking::Session,
        command::{CommandKind, ImageFreeze},
        state_cache::StateEntry,
        transport::{
            AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        Error, ErrorKind, StateKey,
    };

    use super::{profile, SessionConfig};

    /// A stream transport whose writes fail after a configured number of
    /// successful sends, and which can report peer closure as a zero-byte read.
    #[derive(Debug)]
    struct StreamProbe {
        config: TransportConfig,
        responses: VecDeque<Vec<u8>>,
        writes_before_failure: usize,
        close_after_write: bool,
    }

    impl StreamProbe {
        fn healthy() -> Self {
            Self {
                // Serial addressing selects stream send semantics, which is the
                // transport class the spec poisons on a write failure.
                config: TransportConfig {
                    addressing: AddressingMode::Serial,
                    ..TransportConfig::default()
                },
                responses: VecDeque::new(),
                writes_before_failure: usize::MAX,
                close_after_write: false,
            }
        }

        fn failing_after(writes: usize) -> Self {
            Self {
                writes_before_failure: writes,
                ..Self::healthy()
            }
        }

        fn closing_after_write() -> Self {
            Self {
                close_after_write: true,
                ..Self::healthy()
            }
        }
    }

    impl HasTransportConfig for StreamProbe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for StreamProbe {
        fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            if self.writes_before_failure == 0 {
                return Err(Error::Io(Arc::new(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "peer went away mid-frame",
                ))));
            }
            self.writes_before_failure -= 1;
            let target = bytes.first().copied().unwrap_or(0x81) & 0x0f;
            let source = 0x80 | (target.saturating_add(8) << 4);
            self.responses.push_back(vec![source, 0x41, 0xff]);
            self.responses.push_back(vec![source, 0x51, 0xff]);
            Ok(())
        }

        fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            self.recv_into_with_timeout(dst, Duration::from_millis(1))
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            if self.close_after_write {
                // A zero-byte read is the peer closing the connection.
                return Ok(0);
            }
            let response = self.responses.pop_front().ok_or(Error::Timeout)?;
            dst[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(self.config.addressing)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Stream
        }
    }

    #[test]
    fn poisoned_session_requires_a_new_session_and_the_rebuilt_one_works() {
        // The reusable configuration is what recovery keeps.
        let config = SessionConfig::new(profile());

        let session = Session::open(StreamProbe::failing_after(1), config.clone())
            .expect("open the original session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view");
        camera
            .execute(&ImageFreeze::on())
            .expect("the first command applies before the transport fails");

        let poisoned = camera
            .execute(&ImageFreeze::off())
            .expect_err("a stream write failure poisons the session");
        assert!(
            matches!(poisoned, Error::StreamPoisoned { .. }),
            "expected the exact terminal session error, got {poisoned}"
        );
        assert!(
            poisoned.requires_new_session(),
            "a poisoned session must classify as needing a replacement"
        );

        // A poisoned session is terminal: every later operation keeps reporting
        // a condition that classifies the same way, and it is never revived.
        let retained = camera
            .execute(&ImageFreeze::on())
            .expect_err("a poisoned session never accepts new work");
        assert!(retained.requires_new_session());
        assert!(session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view")
            .inquire(&grafton_visca::command::ZoomPositionInquiry)
            .expect_err("a poisoned session rejects inquiries too")
            .requires_new_session());

        // Recovery: drop everything bound to the dead owner and rebuild from
        // the same reusable configuration with a fresh transport.
        drop(camera);
        drop(session);

        let rebuilt =
            Session::open(StreamProbe::healthy(), config).expect("rebuild a fresh session");
        let rebuilt_camera = rebuilt
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view on the rebuilt session");
        let cache = rebuilt_camera.state_cache();
        assert_eq!(
            cache.value(StateKey::ImageFreeze),
            StateEntry::Unknown,
            "a fresh session starts with an unknown state cache"
        );
        rebuilt_camera
            .execute(&ImageFreeze::on())
            .expect("the rebuilt session drives a command through to completion");
        match cache.value(StateKey::ImageFreeze) {
            StateEntry::Set(value) => assert_eq!(value.as_slice(), &[1]),
            other => {
                panic!("expected the rebuilt session to record the applied state, got {other:?}")
            }
        }
        rebuilt.shutdown().expect("shutdown the rebuilt session");
    }

    #[test]
    fn peer_closure_requires_a_new_session() {
        let session = Session::open(
            StreamProbe::closing_after_write(),
            SessionConfig::new(profile()),
        )
        .expect("open a session");
        let closed = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view")
            .execute(&ImageFreeze::on())
            .expect_err("a zero-byte read closes the session");
        assert!(
            matches!(closed, Error::ConnectionClosed { .. }),
            "expected the exact terminal session error, got {closed}"
        );
        assert!(closed.requires_new_session());
    }

    #[test]
    fn deliberate_shutdown_does_not_require_a_new_session() {
        let session = Session::open(StreamProbe::healthy(), SessionConfig::new(profile()))
            .expect("open a session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view");
        camera.execute(&ImageFreeze::on()).expect("first command");

        session.shutdown().expect("deliberate shutdown");

        let stopped = camera
            .execute(&ImageFreeze::off())
            .expect_err("a shut-down session rejects new work");
        assert!(
            matches!(stopped, Error::RuntimeShutdown),
            "expected the deliberate shutdown error, got {stopped}"
        );
        assert!(
            !stopped.requires_new_session(),
            "the application ended this session on purpose; it is not a field disconnect"
        );
        // The distinction is invisible to the error kind, which is exactly why
        // the classification is exposed as its own predicate.
        assert_eq!(stopped.kind(), ErrorKind::IoClosed);
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod tokio_recovery {
    use std::{
        future::Future,
        io,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    use grafton_visca::{
        command::ImageFreeze,
        runtime::TokioRuntime,
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        Error, Session,
    };

    use super::{profile, SessionConfig};

    #[derive(Debug)]
    struct StreamProbe {
        config: TransportConfig,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
        writes_before_failure: Arc<AtomicUsize>,
    }

    impl StreamProbe {
        fn new(writes_before_failure: usize) -> Self {
            let (response_tx, responses) = flume::unbounded();
            Self {
                config: TransportConfig {
                    addressing: AddressingMode::Serial,
                    ..TransportConfig::default()
                },
                responses,
                response_tx,
                writes_before_failure: Arc::new(AtomicUsize::new(writes_before_failure)),
            }
        }
    }

    impl HasTransportConfig for StreamProbe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for StreamProbe {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let tx = self.response_tx.clone();
            let target = bytes.first().copied().unwrap_or(0x81) & 0x0f;
            let source = 0x80 | (target.saturating_add(8) << 4);
            let remaining = Arc::clone(&self.writes_before_failure);
            async move {
                if remaining
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                        value.checked_sub(1)
                    })
                    .is_err()
                {
                    return Err(Error::Io(Arc::new(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "peer went away mid-frame",
                    ))));
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
            SendSemantics::Stream
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(self.config.addressing)
        }
    }

    #[tokio::test]
    async fn poisoned_async_session_requires_a_new_session_and_the_rebuilt_one_works() {
        let config = SessionConfig::new(profile());
        let session = Session::open(
            StreamProbe::new(1),
            config.clone(),
            TokioRuntime::from_current().expect("tokio runtime"),
        )
        .await
        .expect("open the original session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view");
        camera
            .execute(&ImageFreeze::on())
            .await
            .expect("the first command applies before the transport fails");

        let poisoned = camera
            .execute(&ImageFreeze::off())
            .await
            .expect_err("a stream write failure poisons the session");
        assert!(
            matches!(poisoned, Error::StreamPoisoned { .. }),
            "expected the exact terminal session error, got {poisoned}"
        );
        assert!(poisoned.requires_new_session());
        assert!(camera
            .execute(&ImageFreeze::on())
            .await
            .expect_err("a poisoned session never accepts new work")
            .requires_new_session());

        drop(camera);
        drop(session);

        let rebuilt = Session::open(
            StreamProbe::new(usize::MAX),
            config,
            TokioRuntime::from_current().expect("tokio runtime"),
        )
        .await
        .expect("rebuild a fresh session");
        rebuilt
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view on the rebuilt session")
            .execute(&ImageFreeze::on())
            .await
            .expect("the rebuilt session drives a command through to completion");
        rebuilt.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn deliberate_async_shutdown_does_not_require_a_new_session() {
        let session = Session::open(
            StreamProbe::new(usize::MAX),
            SessionConfig::new(profile()),
            TokioRuntime::from_current().expect("tokio runtime"),
        )
        .await
        .expect("open a session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view");
        camera
            .execute(&ImageFreeze::on())
            .await
            .expect("first command");

        session.shutdown().await.expect("deliberate shutdown");

        let stopped = camera
            .execute(&ImageFreeze::off())
            .await
            .expect_err("a shut-down session rejects new work");
        assert!(
            matches!(stopped, Error::RuntimeShutdown),
            "expected the deliberate shutdown error, got {stopped}"
        );
        assert!(!stopped.requires_new_session());
    }
}
