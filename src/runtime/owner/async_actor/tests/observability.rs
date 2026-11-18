//! #851: diagnostics retain terminal causes independently of event delivery.
use super::*;
use crate::observability::DiagnosticCapacity;

async fn subscribe_before_run<R: Runtime + Executor>(
    actor: &mut AsyncOwnerActor<R>,
    runtime: &R,
) -> super::super::super::DiagnosticSubscription {
    let (reply, result) = flume::bounded(1);
    let mut driver = UngatedDriver {
        receives: flume::bounded(1).1,
    };
    actor
        .handle_event(
            OwnerEvent::Control(ControlBoundary::SubscribeDiagnostics {
                capacity: DiagnosticCapacity::MIN,
                reply,
            }),
            &mut driver,
            Executor::now(runtime),
            false,
        )
        .await;
    result.recv_async().await.unwrap().unwrap()
}

#[tokio::test]
async fn async_diagnostic_subscriptions_keep_peer_close_and_poison_causes() {
    for poison in [false, true] {
        let runtime = TokioRuntime::from_current().unwrap();
        let mut owner_policy = policy(1);
        if poison {
            owner_policy.protocol.transport = TransportKind::Stream;
        }
        let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();
        let subscription = subscribe_before_run(&mut actor, &runtime).await;
        let (frames, receives) = flume::bounded(1);
        frames
            .send(if poison {
                OwnerReceive::consumed_error(
                    Error::transport_error(std::io::Error::other(
                        "consumed undecodable stream input",
                    )),
                    crate::runtime::engine::TransportKind::Stream,
                )
            } else {
                OwnerReceive::Fatal(crate::runtime::owner::ReceiveFatal::TransportClosed {
                    cause: None,
                })
            })
            .unwrap();
        let _ = actor.run(UngatedDriver { receives }).await;
        let metrics = handle.metrics().await.unwrap();
        assert_eq!(
            metrics.session,
            if poison {
                crate::SessionStatus::Poisoned
            } else {
                crate::SessionStatus::Closed
            }
        );
        assert_eq!(metrics.active, 0);
        assert_eq!(metrics.pending, 0);
        let terminal = handle.shutdown().unwrap_err();
        assert!(terminal.requires_new_session());
        let reported = loop {
            match subscription.recv_async().await {
                Ok(_) => {}
                Err(error) => break error,
            }
        };
        assert_eq!(reported.to_string(), terminal.to_string());
        assert!(reported.requires_new_session());
    }
}

#[tokio::test]
async fn async_diagnostic_subscription_proves_a_panicked_owner_is_dead() {
    let runtime = TokioRuntime::from_current().unwrap();
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
    let subscription = subscribe_before_run(&mut actor, &runtime).await;
    assert!(tokio::spawn(actor.run(PanickingReceiveDriver))
        .await
        .is_err());
    assert!(matches!(
        subscription.recv_async().await,
        Err(Error::OwnerTerminated)
    ));
    for error in [
        handle.metrics().await.unwrap_err(),
        handle.shutdown().unwrap_err(),
    ] {
        assert!(matches!(error, Error::OwnerTerminated));
        assert!(error.requires_new_session());
    }
}
