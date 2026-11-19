use super::*;
use crate::{runtime::engine::AppliedStateProjection, state_cache::KnownState};
use std::sync::atomic::AtomicUsize;

struct ScriptedTeardownDriver {
    started: flume::Sender<RequestId>,
    write_succeeds: flume::Receiver<bool>,
    reads: flume::Receiver<Option<OwnerReceive>>,
    panic_in_drop: Option<Arc<AtomicUsize>>,
}

impl Drop for ScriptedTeardownDriver {
    fn drop(&mut self) {
        if let Some(count) = &self.panic_in_drop {
            count.fetch_add(1, Ordering::Relaxed);
            panic!("injected driver drop panic");
        }
    }
}

impl RetainedStreamInput for ScriptedTeardownDriver {}

impl AsyncOwnerDriver for ScriptedTeardownDriver {
    async fn write(&mut self, write: WireWrite<'_>) -> WriteOutcome {
        self.started.send_async(write.request).await.unwrap();
        assert!(
            self.write_succeeds.recv_async().await.unwrap(),
            "injected write panic"
        );
        WriteOutcome::Sent(TransmissionMeta::Known)
    }

    async fn receive(&mut self, _buffers: &mut super::super::super::OwnerBuffers) -> OwnerReceive {
        match self.reads.recv_async().await {
            Ok(Some(frame)) => frame,
            Ok(None) => panic!("injected read panic"),
            Err(_) => future::pending().await,
        }
    }
}

struct Peer {
    started: flume::Receiver<RequestId>,
    write_succeeds: flume::Sender<bool>,
    reads: flume::Sender<Option<OwnerReceive>>,
}

impl Peer {
    fn new(panic_in_drop: Option<Arc<AtomicUsize>>) -> (Self, ScriptedTeardownDriver) {
        let (started_tx, started) = flume::bounded(1);
        let (succeed, write_succeeds) = flume::bounded(1);
        let (read_tx, reads) = flume::bounded(2);
        (
            Self {
                started,
                write_succeeds: succeed,
                reads: read_tx,
            },
            ScriptedTeardownDriver {
                started: started_tx,
                write_succeeds,
                reads,
                panic_in_drop,
            },
        )
    }

    fn reply(&self, target: TargetSlot, mut frames: Vec<DecodedFrame>) {
        for frame in &mut frames {
            frame.target = target;
        }
        self.reads.try_send(Some(batch(frames))).unwrap();
    }

    async fn finish_write(&self, request: RequestId, succeeds: bool) {
        assert_eq!(self.started.recv_async().await.unwrap(), request);
        self.write_succeeds.try_send(succeeds).unwrap();
    }
}

type SpawnDriver = fn(
    OwnerPolicy,
    ScriptedTeardownDriver,
) -> (
    AsyncOwnerHandle,
    flume::Receiver<OwnerSnapshot>,
    std::thread::JoinHandle<()>,
);

async fn drop_panic_verdict(spawn: SpawnDriver) {
    for closed_by_peer in [false, true] {
        let drops = Arc::new(AtomicUsize::new(0));
        let (peer, driver) = Peer::new(Some(Arc::clone(&drops)));
        if closed_by_peer {
            peer.reads
                .try_send(Some(OwnerReceive::Fatal(
                    super::super::super::ReceiveFatal::TransportClosed { cause: None },
                )))
                .unwrap();
        }
        let (handle, _snapshots, join) = spawn(policy(1), driver);
        let other = handle.clone();
        let _ = handle.shutdown();
        let _ = other.shutdown();
        let (first, second) = future::zip(handle.wait_closed(), other.wait_closed()).await;
        for result in [first, second, other.wait_closed().await] {
            if closed_by_peer {
                assert!(matches!(result, Err(Error::ConnectionClosed { .. })));
            } else {
                assert!(matches!(result, Err(Error::OwnerTerminated)));
            }
        }
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert!(join.join().is_err());
    }
}

fn setter(target: TargetSlot, enabled: bool) -> RuntimeRequest {
    let mut request = command_for(target);
    if let RuntimeRequest::Command { applied_state, .. } = &mut request {
        *applied_state = Some(AppliedStateProjection::Set(KnownState::ImageFreeze(
            enabled,
        )));
    }
    request
}

async fn uncertain_state_on_panic(spawn: SpawnDriver) {
    let first = TargetSlot::CAMERA_1;
    let second = TargetSlot::CAMERA_2;
    for after_ack in [false, true] {
        let (peer, driver) = Peer::new(None);
        let (handle, _snapshots, join) =
            spawn(two_target_raw_policy(TransportKind::Datagram), driver);
        let changed = handle.state_cache(first);
        let unchanged = handle.state_cache(second);
        for target in [first, second] {
            let receipt = handle.submit(setter(target, true)).await.unwrap();
            peer.finish_write(receipt.id(), true).await;
            peer.reply(
                target,
                vec![ack(ViscaSocket::S1), completion(ViscaSocket::S1)],
            );
            assert!(matches!(
                receipt.terminal().await.unwrap(),
                RuntimeOutcome::Command(CommandOutcome::Applied)
            ));
        }
        assert_eq!(changed.image().freeze(), Some(true));
        assert_eq!(unchanged.image().freeze(), Some(true));

        // An unacknowledged raw command holds the second target's pre-ACK
        // slot, leaving its setter admitted but unwritten.
        let blocker = handle.submit(command_for(second)).await.unwrap();
        peer.finish_write(blocker.id(), true).await;
        let unwritten = handle.submit(setter(second, false)).await.unwrap();
        let uncertain = handle.submit(setter(first, false)).await.unwrap();
        peer.finish_write(uncertain.id(), after_ack).await;
        if after_ack {
            peer.reply(first, vec![ack(ViscaSocket::S1)]);
            peer.reads.try_send(None).unwrap();
        }
        let deadline = handle.deadline_after(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            handle.wait_core_until(&uncertain, deadline).await,
            Err(Error::OwnerTerminated)
        ));
        assert_eq!(changed.image().freeze(), None);
        assert_eq!(unchanged.image().freeze(), Some(true));
        assert!(matches!(
            handle.wait_core_until(&unwritten, deadline).await,
            Err(Error::OwnerTerminated)
        ));
        assert!(matches!(
            handle.wait_closed().await,
            Err(Error::OwnerTerminated)
        ));
        assert!(join.join().is_err());
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_teardown_panics_publish_cache_and_close_results() {
    drop_panic_verdict(run_isolated_tokio_actor).await;
    uncertain_state_on_panic(run_isolated_tokio_actor).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_teardown_panics_publish_cache_and_close_results() {
    smol::block_on(async {
        drop_panic_verdict(run_isolated_smol_actor).await;
        uncertain_state_on_panic(run_isolated_smol_actor).await;
    });
}
