//! #838: input that arrives during an idle interval wins before due work.
use super::*;
#[cfg(feature = "runtime-tokio")]
use crate::runtime::owner::turn::{ReceiveClass, Selected};

#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn full_idle_read_timeouts_do_not_delay_the_next_reply() {
    for transport in [TransportKind::Datagram, TransportKind::Stream] {
        for (deadline_ms, reply_ms) in [(205, 202), (400, 210), (205, 205)] {
            let initial = Instant::now();
            let runtime = ManualRuntime::with_polling_sleeps(initial);
            let mut owner_policy = policy(1);
            owner_policy.protocol.transport = transport;
            owner_policy.read_timeout = Duration::from_millis(200);
            let (handle, mut actor) = AsyncOwnerActor::new(owner_policy, runtime.clone()).unwrap();
            let (frames, receives) = flume::bounded(1);
            let mut driver = UngatedDriver { receives };
            let mut request = inquiry();
            let RuntimeRequest::Inquiry { context, .. } = &mut request else {
                unreachable!()
            };
            context.timeout.inquiry = Duration::from_millis(deadline_ms);
            let (completion, admitted) = handle.core.enqueue_admission(request, None).unwrap();
            let admission = actor.core.receivers.admissions.try_recv().unwrap();
            actor
                .handle_event(
                    OwnerEvent::Admission(Ok(admission)),
                    &mut driver,
                    &runtime,
                    initial,
                    false,
                )
                .await;
            admitted.recv_async().await.unwrap().unwrap();
            let mut run = Box::pin(actor.run(driver));
            let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
            assert!(run.as_mut().poll(&mut cx).is_pending());
            runtime.advance(Duration::from_millis(200));
            for _ in 0..3 {
                assert!(run.as_mut().poll(&mut cx).is_pending());
            }
            runtime.advance(Duration::from_millis(reply_ms - 200));
            frames
                .send(OwnerReceive::Frames(
                    vec![DecodedFrame {
                        target: crate::target::TargetSlot::CAMERA_1,
                        sequence: None,
                        response: DecodedResponse::InquiryReply {
                            payload: smallvec::smallvec![3],
                        },
                    }]
                    .into(),
                ))
                .unwrap();
            for _ in 0..4 {
                assert!(run.as_mut().poll(&mut cx).is_pending());
            }
            assert!(
                matches!(completion.try_recv(), Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [3])
            );
            handle.shutdown().unwrap();
            assert_eq!(run.await.state, SessionState::Shutdown);
        }
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn a_paced_receive_at_its_response_deadline_wins_the_timer_race() {
    let initial = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(initial);
    let (handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
    let (frames, receives) = flume::bounded(1);
    let mut driver = UngatedDriver { receives };
    let mut request = inquiry();
    let RuntimeRequest::Inquiry { context, .. } = &mut request else {
        unreachable!()
    };
    context.timeout.inquiry = Duration::from_millis(5);
    let (completion, admitted) = handle.core.enqueue_admission(request, None).unwrap();
    let admission = actor.core.receivers.admissions.try_recv().unwrap();
    actor
        .handle_event(
            OwnerEvent::Admission(Ok(admission)),
            &mut driver,
            &runtime,
            initial,
            false,
        )
        .await;
    admitted.recv_async().await.unwrap().unwrap();
    let outcome = actor
        .handle_event(
            OwnerEvent::Receive {
                result: OwnerReceive::NoProgress(
                    crate::runtime::owner::NoProgressReason::EagerIdle,
                ),
                received_at: initial,
            },
            &mut driver,
            &runtime,
            initial,
            false,
        )
        .await;
    actor.core.finish(
        Selected::Receive {
            due_at_receive: Default::default(),
            class: ReceiveClass::NoInput,
        },
        None,
        outcome,
    );
    let mut run = Box::pin(actor.run(driver));
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    for _ in 0..3 {
        assert!(run.as_mut().poll(&mut cx).is_pending());
    }
    runtime.advance(Duration::from_millis(5));
    frames
        .send(OwnerReceive::Frames(
            vec![DecodedFrame {
                target: crate::target::TargetSlot::CAMERA_1,
                sequence: None,
                response: DecodedResponse::InquiryReply {
                    payload: smallvec::smallvec![3],
                },
            }]
            .into(),
        ))
        .unwrap();
    for _ in 0..4 {
        assert!(run.as_mut().poll(&mut cx).is_pending());
    }
    assert!(matches!(
        completion.try_recv(),
        Some(RuntimeOutcome::Reply { .. })
    ));
    handle.shutdown().unwrap();
    assert_eq!(run.await.state, SessionState::Shutdown);
}

struct TimedIdle {
    runtime: ManualRuntime,
    wait: Duration,
    fault: bool,
}

impl AsyncOwnerDriver for TimedIdle {
    async fn write(&mut self, _write: WireWrite<'_>) -> WriteOutcome {
        unreachable!("the idle-read fixture never writes")
    }

    async fn receive(
        &mut self,
        _buffers: &mut super::super::super::OwnerBuffers,
        _frame_limit: usize,
    ) -> OwnerReceive {
        Executor::sleep(&self.runtime, self.wait).await;
        if self.fault {
            OwnerReceive::read_error(Error::io_timeout())
        } else {
            OwnerReceive::NoProgress(crate::runtime::owner::NoProgressReason::EagerIdle)
        }
    }
}
impl RetainedStreamInput for TimedIdle {}

#[tokio::test(start_paused = true)]
async fn a_transports_own_full_timeout_is_not_eager_idle() {
    let timeout = Duration::from_millis(200);
    for fault in [false, true] {
        for wait in [Duration::from_millis(50), timeout] {
            let runtime = ManualRuntime::with_polling_sleeps(Instant::now());
            let (_handle, mut actor) = AsyncOwnerActor::new(policy(1), runtime.clone()).unwrap();
            let mut driver = TimedIdle {
                runtime: runtime.clone(),
                wait,
                fault,
            };
            let mut receive = Box::pin(AsyncOwnerActor::<ManualRuntime>::receive_within(
                &mut driver,
                actor.core.state.buffers(),
                1,
                &runtime,
                timeout,
                actor.timers.read.as_mut(),
            ));
            let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
            assert!(receive.as_mut().poll(&mut cx).is_pending());
            runtime.advance(wait);
            let Poll::Ready(result) = receive.as_mut().poll(&mut cx) else {
                panic!("the timed idle driver completed")
            };
            assert_eq!(
                matches!(
                    result,
                    OwnerReceive::NoProgress(crate::runtime::owner::NoProgressReason::IdleTimeout)
                ),
                wait == timeout
            );
        }
    }
}
