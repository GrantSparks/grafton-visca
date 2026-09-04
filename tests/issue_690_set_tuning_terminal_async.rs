//! Issue #690 terminal-error precedence through the async tuning boundary.
//!
//! The blocking twin is `issue_690_set_tuning_terminal.rs`. Construction-only
//! strict changes and profile-invalid mutable values are rejected while an
//! owner is live, but neither may mask a terminal cause the owner retained.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{future::Future, sync::Arc, time::Duration};

use grafton_visca::{
    completion::AppliedOnly,
    profile::ProfileSpec,
    request::builtin::ZoomStop,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, Executor, OperationalTuning, Session, SessionConfig,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

/// A raw datagram transport that injects one transient receive fault after its
/// first write. Under the strict construction policy, that fault poisons the
/// session while the unsequenced command is awaiting its ACK.
#[derive(Debug)]
struct PoisonOnReadTransport {
    config: TransportConfig,
    sends: usize,
    reads: flume::Sender<Result<Vec<u8>, Error>>,
    replies: flume::Receiver<Result<Vec<u8>, Error>>,
}

impl PoisonOnReadTransport {
    fn new() -> Self {
        let (reads, replies) = flume::unbounded();
        Self {
            config: TransportConfig::default(),
            sends: 0,
            reads,
            replies,
        }
    }
}

impl HasTransportConfig for PoisonOnReadTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for PoisonOnReadTransport {
    fn send(&mut self, _bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        self.sends = self.sends.saturating_add(1);
        if self.sends == 1 {
            let _ = self.reads.send(Err(Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            )))));
        }
        async { Ok(()) }
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
                .map_err(|_| Error::ConnectionClosed { reason: None })??;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn raw_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("raw runtime profile"),
    )
}

async fn live_invalid_update_is_rejected_without_changing_tuning<E: Executor>(executor: E) {
    let session = Session::open(PoisonOnReadTransport::new(), raw_config(), executor)
        .await
        .expect("owner session");
    let installed = OperationalTuning::new().command_spacing(Duration::from_millis(40));
    session
        .set_tuning(installed)
        .await
        .expect("a live owner accepts valid tuning");

    let error = session
        .set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO))
        .await
        .expect_err("a live owner still rejects a zero protocol timeout");
    assert!(matches!(error, Error::InvalidRequest(_)), "got {error:?}");
    assert_eq!(
        session.tuning(),
        installed,
        "a rejected live update leaves the installed tuning unchanged"
    );
    session.shutdown().await.expect("owner shutdown");
}

async fn invalid_updates_after_poison_return_retained_terminal_cause<E: Executor>(executor: E) {
    let config = raw_config().with_strict_unconfirmed_poison(true);
    let session = Session::open(PoisonOnReadTransport::new(), config, executor)
        .await
        .expect("owner session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera view");
    let error = camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .expect("submission")
        .applied()
        .await
        .expect_err("the strict opt-in poisons on the receive fault");
    assert!(matches!(error, Error::StreamPoisoned { .. }));

    let error = session
        .set_tuning(OperationalTuning::new())
        .await
        .expect_err("a strict override cannot mask an existing poison");
    assert!(
        matches!(error, Error::StreamPoisoned { .. }),
        "set_tuning surfaces the retained poison, got {error:?}"
    );
    assert!(error.requires_new_session());

    let error = session
        .set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO))
        .await
        .expect_err("profile validation cannot mask an existing poison");
    assert!(
        matches!(error, Error::StreamPoisoned { .. }),
        "set_tuning gives the poison precedence over InvalidRequest, got {error:?}"
    );
}

async fn invalid_updates_after_shutdown_return_retained_terminal_cause<E: Executor>(executor: E) {
    let session = Session::open(PoisonOnReadTransport::new(), raw_config(), executor)
        .await
        .expect("owner session");
    let closed_handle = session.clone();
    session.close().await.expect("clean owner shutdown");

    let error = closed_handle
        .set_tuning(OperationalTuning::new())
        .await
        .expect_err("a strict override cannot mask a completed shutdown");
    assert!(
        matches!(error, Error::RuntimeShutdown),
        "set_tuning surfaces the retained shutdown, got {error:?}"
    );

    let error = closed_handle
        .set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO))
        .await
        .expect_err("profile validation cannot mask a completed shutdown");
    assert!(
        matches!(error, Error::RuntimeShutdown),
        "set_tuning gives shutdown precedence over InvalidRequest, got {error:?}"
    );
}

macro_rules! runtime_matrix {
    ($($scenario:ident),+ $(,)?) => {
        $(
            mod $scenario {
                #[cfg(feature = "runtime-tokio")]
                #[tokio::test]
                async fn tokio() {
                    let executor =
                        grafton_visca::TokioRuntime::from_current().expect("Tokio runtime");
                    super::$scenario(executor).await;
                }

                #[cfg(feature = "runtime-smol")]
                #[test]
                fn smol() {
                    smol::block_on(super::$scenario(grafton_visca::SmolRuntime::new()));
                }
            }
        )+
    };
}

runtime_matrix!(
    live_invalid_update_is_rejected_without_changing_tuning,
    invalid_updates_after_poison_return_retained_terminal_cause,
    invalid_updates_after_shutdown_return_retained_terminal_cause,
);
