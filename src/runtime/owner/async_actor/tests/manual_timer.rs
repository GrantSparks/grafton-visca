//! Resettable timers on the actor tests' explicitly controlled clock.

use super::*;
use crate::executor::ResettableTimer;

pub(super) struct ManualTimer {
    runtime: ManualRuntime,
    deadline: Instant,
    duration: Duration,
    announced: bool,
}

impl ManualTimer {
    pub(super) fn new(runtime: ManualRuntime, deadline: Instant) -> Self {
        let duration = deadline.saturating_duration_since(*runtime.now.lock().unwrap());
        Self {
            runtime,
            deadline,
            duration,
            announced: false,
        }
    }
}

impl Future for ManualTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        if !this.announced {
            if let Some(observed) = &this.runtime.sleep_started {
                let _ = observed.try_send(this.duration);
            }
            this.announced = true;
        }
        if this.runtime.polling_sleeps && *this.runtime.now.lock().unwrap() >= this.deadline {
            Poll::Ready(())
        } else {
            let mut wakers = this.runtime.sleep_wakers.lock().unwrap();
            if !wakers.iter().any(|waker| waker.will_wake(context.waker())) {
                wakers.push(context.waker().clone());
            }
            Poll::Pending
        }
    }
}

impl ResettableTimer for ManualTimer {
    fn reset(self: Pin<&mut Self>, deadline: Instant) {
        let this = self.get_mut();
        this.deadline = deadline;
        this.duration = deadline.saturating_duration_since(*this.runtime.now.lock().unwrap());
        this.announced = false;
    }
}

#[tokio::test]
async fn manual_actor_timer_can_be_reset_after_completion() {
    let initial = Instant::now();
    let runtime = ManualRuntime::with_polling_sleeps(initial);
    let mut timer = pin!(runtime.timer(initial + Duration::from_millis(10)));
    assert!(future::poll_once(timer.as_mut()).await.is_none());
    runtime.advance(Duration::from_millis(10));
    assert_eq!(future::poll_once(timer.as_mut()).await, Some(()));
    timer.as_mut().reset(initial + Duration::from_millis(20));
    assert!(future::poll_once(timer.as_mut()).await.is_none());
    runtime.advance(Duration::from_millis(10));
    assert_eq!(future::poll_once(timer.as_mut()).await, Some(()));
}
