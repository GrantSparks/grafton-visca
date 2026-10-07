//! Deterministic async session close and transport-release coverage.
//!
//! Each scenario runs under each enabled runtime, as the cases
//! `close_waits_for_transport_drop::{tokio,smol}` and
//! `close_reports_transport_winner::{tokio,smol}`. The wire's drop is observed
//! through `FakeCamera::wires_dropped`; the EOF-gated transport stays local.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use grafton_visca_test_support::{fake_camera, runtime_matrix};

use grafton_visca::{
    profiles::PtzOpticsG2,
    transport::{
        AsyncTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
    },
    Error, Executor, Session, SessionConfig,
};

use fake_camera::FakeCamera;

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

// Local fake: gates its EOF on a test-controlled release and reports when the
// owner's receive has started, which a scripted camera cannot express.
impl AsyncTransport for GatedCloseTransport {
    async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
        Ok(())
    }

    async fn recv_into<'a>(&'a mut self, _dst: &'a mut [u8]) -> Result<ReceiveOutcome, Error> {
        let _ = self.receive_started.try_send(());
        self.release_eof
            .recv_async()
            .await
            .map_err(|_| Error::RuntimeShutdown)?;
        Ok(ReceiveOutcome::complete(0))
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

async fn close_waits_for_transport_drop<E: Executor>(executor: E) {
    let camera = FakeCamera::silent();
    let session = Session::open(
        camera.async_wire(),
        SessionConfig::from_compile_time::<PtzOpticsG2>().expect("session config"),
        executor.clone(),
    )
    .await
    .expect("open first session");

    assert_eq!(
        camera.wires_dropped(),
        0,
        "the endpoint must remain owned before close"
    );

    // A shutdown from any clone is only a signal. Consuming another clone is
    // the sole barrier, and must wait for the one shared actor/transport.
    let signal = session.clone();
    signal.shutdown().expect("shutdown signal");
    signal.shutdown().expect("idempotent shutdown signal");

    session.close().await.expect("deterministic close");
    assert_eq!(
        camera.wires_dropped(),
        1,
        "close must not resolve before transport drop"
    );

    // The endpoint can be acquired immediately after the barrier resolves.
    let reopened = Session::open(
        camera.async_wire(),
        SessionConfig::from_compile_time::<PtzOpticsG2>().expect("session config"),
        executor,
    )
    .await
    .expect("reopen after close");
    reopened.close().await.expect("close reopened session");
    assert_eq!(camera.wires_dropped(), 2);
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
            Err(Error::ConnectionClosed { reason: None, .. })
        ),
        "close must preserve the transport terminal result that already won"
    );
}

runtime_matrix!(
    close_waits_for_transport_drop,
    close_reports_transport_winner
);
