//! Actual async waiters, restarted by their admission-rejection wake.
#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
use super::*;
use crate::runtime::owner::tests::waiter_rejection as fixture;
use std::{
    future::Future,
    pin::{pin, Pin},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    task::{Context, Poll, Wake, Waker},
};

type Waiter = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;

/// flume wakes under its reply lock, before enqueueing the reply. Polling
/// the original receiver inline would deadlock. Start a DIFFERENT actual
/// waiter in that wake, so its admission still precedes reject() returning.
struct RestartOnReplyWake {
    next: Mutex<Option<Waiter>>,
    result: Mutex<Option<Result<(), Error>>>,
    enabled: AtomicBool,
    armed: AtomicBool,
    started_in_wake: AtomicBool,
}

impl RestartOnReplyWake {
    fn poll_next(self: &Arc<Self>) {
        let mut next = self.next.lock().unwrap();
        if let Some(waiter) = next.as_mut() {
            let waker = Waker::from(Arc::clone(self));
            if let Poll::Ready(result) = waiter.as_mut().poll(&mut Context::from_waker(&waker)) {
                *next = None;
                *self.result.lock().unwrap() = Some(result);
            }
        }
    }
}

impl Wake for RestartOnReplyWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if self.enabled.load(Ordering::SeqCst) && self.armed.swap(false, Ordering::SeqCst) {
            self.started_in_wake.store(true, Ordering::SeqCst);
            self.poll_next();
        }
    }
}

#[test]
fn async_two_axis_waiter_restarts_from_owner_expiry_wake() {
    use crate::testing::testkit::{ManualClock, ManualClockExecutor};
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let runtime = ManualClockExecutor::local(&clock);
    let (owner, mut actor) = AsyncOwnerActor::new(fixture::policy(), runtime).unwrap();
    let queries = fixture::queries(owner.validated_tuning());
    let permits = owner.core.permits.clone();
    let next_owner = owner.clone();
    let next_queries = queries.clone();
    let restart = Arc::new(RestartOnReplyWake {
        next: Mutex::new(Some(Box::pin(async move {
            next_owner
                .wait_until_idle(fixture::wait(), &next_queries)
                .await
        }))),
        result: Mutex::new(None),
        enabled: AtomicBool::new(false),
        armed: AtomicBool::new(true),
        started_in_wake: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&restart));
    let mut context = Context::from_waker(&waker);
    let original = owner.wait_until_idle(fixture::wait(), &queries);
    let mut original = pin!(original);
    assert!(original.as_mut().poll(&mut context).is_pending());
    let admission = actor.core.receivers.admissions.try_recv().unwrap();
    fixture::assert_axis(&admission, 0x47);
    // The shared clock reaches the deadline while the scheduler holds
    // the caller timer wake, letting the owner win the admission claim.
    clock.advance(fixture::BUDGET);
    restart.enabled.store(true, Ordering::SeqCst);
    crate::runtime::owner::boundary::tests::expire_waiter_from_owner(
        admission,
        &mut actor.core.state,
        clock.now(),
    );
    assert!(restart.started_in_wake.load(Ordering::SeqCst));
    let reply_wake_result = restart.result.lock().unwrap();
    assert!(
        reply_wake_result.is_none(),
        "fresh actual waiter completed in the rejection wake: {reply_wake_result:?}"
    );
    drop(reply_wake_result);
    assert_eq!(
        actor.core.receivers.admissions.len(),
        1,
        "fresh wait_until_idle queued from inside the rejection wake"
    );
    assert_eq!(permits.available(), 0);
    // An absolute-deadline expiry ends this waiter before its second
    // axis. Its timeout remains authoritative over the ready caller timer.
    let Poll::Ready(Err(error)) = original.as_mut().poll(&mut context) else {
        panic!("the first actual waiter must report its owner-side expiry");
    };
    assert!(matches!(error, Error::Timeout { .. }));
    assert_eq!(
        error.failure_context(),
        Error::query_timeout().failure_context()
    );
    for (index, register) in [0x47, 0x48, 0x47, 0x48].into_iter().enumerate() {
        if index == 2 {
            clock.advance(fixture::INTERVAL);
            restart.poll_next();
        }
        let admission = actor.core.receivers.admissions.try_recv().unwrap();
        fixture::answer_position(&mut actor.core.state, admission, register, &clock);
        restart.poll_next();
    }
    restart
        .result
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .expect("fresh two-axis waiter must not see RuntimeQueueFull");
    assert_eq!(permits.available(), 1);
    assert!(actor.core.receivers.admissions.is_empty());
    let metrics = actor.core.state.metrics();
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.admitted, 4);
    assert_eq!(metrics.writes, 4);
    assert_eq!(metrics.terminal, 4);
}
