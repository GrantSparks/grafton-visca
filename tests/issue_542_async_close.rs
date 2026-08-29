//! Deterministic async session close and transport-release coverage.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use grafton_visca::{
    profiles::PtzOpticsG2,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, Executor, Session, SessionConfig,
};

/// A transport whose endpoint is unavailable until its owner is dropped.
///
/// `recv_into` intentionally remains pending so the only way the owner can
/// finish is by observing the explicit shutdown boundary. The drop event lets
/// the test assert that `close`'s completion is ordered after transport
/// release, rather than merely after the shutdown signal was queued.
#[derive(Debug)]
struct DropProbeTransport {
    config: TransportConfig,
    endpoint_in_use: Arc<AtomicBool>,
    dropped: flume::Sender<()>,
}

/// A transport whose EOF is released only after the caller has queued an
/// explicit shutdown. Both sources are then ready in the owner's select, so
/// the receive-first ordering from architecture §3 can be exercised through
/// the public session facade.
#[derive(Debug)]
struct GatedCloseTransport {
    config: TransportConfig,
    receive_started: flume::Sender<()>,
    release_eof: flume::Receiver<()>,
    dropped: flume::Sender<()>,
}

impl DropProbeTransport {
    fn new(endpoint_in_use: Arc<AtomicBool>, dropped: flume::Sender<()>) -> Result<Self, Error> {
        if endpoint_in_use
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Error::InvalidState(
                "the fake endpoint is still owned by another transport".into(),
            ));
        }
        Ok(Self {
            config: TransportConfig::default(),
            endpoint_in_use,
            dropped,
        })
    }
}

impl Drop for DropProbeTransport {
    fn drop(&mut self) {
        self.endpoint_in_use.store(false, Ordering::Release);
        let _ = self.dropped.try_send(());
    }
}

impl HasTransportConfig for DropProbeTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for DropProbeTransport {
    async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
        Ok(())
    }

    fn recv_into<'a>(
        &'a mut self,
        _dst: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send {
        std::future::pending::<Result<usize, Error>>()
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

impl HasTransportConfig for GatedCloseTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl Drop for GatedCloseTransport {
    fn drop(&mut self) {
        let _ = self.dropped.try_send(());
    }
}

impl AsyncTransport for GatedCloseTransport {
    async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
        Ok(())
    }

    async fn recv_into<'a>(&'a mut self, _dst: &'a mut [u8]) -> Result<usize, Error> {
        let _ = self.receive_started.try_send(());
        self.release_eof
            .recv_async()
            .await
            .map_err(|_| Error::RuntimeShutdown)?;
        Ok(0)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

async fn close_waits_for_transport_drop<E: Executor>(executor: E) {
    let endpoint_in_use = Arc::new(AtomicBool::new(false));
    let (dropped_tx, dropped_events) = flume::bounded(4);
    let transport = DropProbeTransport::new(Arc::clone(&endpoint_in_use), dropped_tx.clone())
        .expect("first transport owns the endpoint");
    let session = Session::open(
        transport,
        SessionConfig::from_compile_time::<PtzOpticsG2>().expect("session config"),
        executor.clone(),
    )
    .await
    .expect("open first session");

    assert!(
        DropProbeTransport::new(Arc::clone(&endpoint_in_use), dropped_tx.clone()).is_err(),
        "the endpoint must remain owned before close"
    );

    // A shutdown from any clone is only a signal. Consuming another clone is
    // the sole barrier, and must wait for the one shared actor/transport.
    let signal = session.clone();
    signal.shutdown().await.expect("shutdown signal");
    signal.shutdown().await.expect("idempotent shutdown signal");

    session.close().await.expect("deterministic close");
    assert!(
        dropped_events.try_recv().is_ok(),
        "close must not resolve before transport drop"
    );
    assert!(!endpoint_in_use.load(Ordering::Acquire));

    // The endpoint can be acquired immediately after the barrier resolves.
    let reopened_transport =
        DropProbeTransport::new(endpoint_in_use, dropped_tx).expect("close released the endpoint");
    let reopened = Session::open(
        reopened_transport,
        SessionConfig::from_compile_time::<PtzOpticsG2>().expect("session config"),
        executor,
    )
    .await
    .expect("reopen after close");
    reopened.close().await.expect("close reopened session");
}

async fn close_reports_transport_winner<E: Executor>(executor: E) {
    let (receive_started_tx, receive_started_rx) = flume::bounded(1);
    let (release_eof_tx, release_eof_rx) = flume::bounded(1);
    let (dropped_tx, dropped_rx) = flume::bounded(1);
    let transport = GatedCloseTransport {
        config: TransportConfig::default(),
        receive_started: receive_started_tx,
        release_eof: release_eof_rx,
        dropped: dropped_tx,
    };
    let session = Session::open(
        transport,
        SessionConfig::from_compile_time::<PtzOpticsG2>().expect("session config"),
        executor.clone(),
    )
    .await
    .expect("open gated session");

    // Wait until the actor has entered its receive branch, then release an EOF
    // and explicitly wait for the transport to be dropped. This proves the
    // transport terminal result already won before close is attempted; it does
    // not rely on a scheduler-specific gap between queuing shutdown and
    // releasing EOF.
    receive_started_rx
        .recv_async()
        .await
        .expect("actor receive started");
    release_eof_tx
        .try_send(())
        .expect("release gated transport EOF");
    dropped_rx
        .recv_async()
        .await
        .expect("transport dropped after terminal publication");

    assert!(
        matches!(
            session.close().await,
            Err(Error::ConnectionClosed { reason: None })
        ),
        "close must preserve the transport terminal result that already won"
    );
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn close_waits_for_transport_drop_under_tokio() {
    close_waits_for_transport_drop(
        grafton_visca::runtime::TokioRuntime::from_current().expect("tokio runtime"),
    )
    .await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn close_waits_for_transport_drop_under_smol() {
    smol::block_on(close_waits_for_transport_drop(
        grafton_visca::runtime::SmolRuntime::new(),
    ));
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn close_reports_transport_winner_under_tokio() {
    close_reports_transport_winner(
        grafton_visca::runtime::TokioRuntime::from_current().expect("tokio runtime"),
    )
    .await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn close_reports_transport_winner_under_smol() {
    smol::block_on(close_reports_transport_winner(
        grafton_visca::runtime::SmolRuntime::new(),
    ));
}
