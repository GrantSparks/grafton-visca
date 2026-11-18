//! Runtime-free scheduling on the owner's one manual clock.

use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

use super::{Bell, ManualClock, ManualClockTimer, Source};
use crate::executor::Executor;

/// The runtime-free scheduling backend selected by `ManualClockExecutor::local`.
///
/// It uses the same manual clock as its wrapping executor and drives tasks only
/// while that executor's `run_until` is running.
#[derive(Clone)]
pub struct LocalExecutor {
    clock: ManualClock,
    scheduler: Arc<async_executor::Executor<'static>>,
}

impl fmt::Debug for LocalExecutor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalExecutor")
            .field("clock", &self.clock)
            .finish_non_exhaustive()
    }
}

impl LocalExecutor {
    pub(super) fn new(clock: &ManualClock) -> Self {
        Self {
            clock: clock.clone(),
            scheduler: Arc::new(async_executor::Executor::new()),
        }
    }

    pub(super) fn block_on<F: Future>(&self, future: F) -> F::Output {
        let _participant = self.clock.enter();
        let (notify, notified) = flume::bounded(1);
        let notify = Arc::new(Notify {
            sender: notify,
            ready: AtomicBool::new(true),
        });
        let mut driver = Driver {
            future: Box::pin(self.scheduler.run(future)),
            waker: Waker::from(Arc::clone(&notify)),
            notified,
            notify,
        };
        loop {
            if let Some(value) = self.clock.wait_on(None, &mut driver) {
                return value;
            }
        }
    }
}

struct Notify {
    sender: flume::Sender<()>,
    ready: AtomicBool,
}
impl Wake for Notify {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.ready.store(true, Ordering::Release);
        let _ = self.sender.try_send(());
    }
}

/// Poll the executor on the caller's thread and park through the clock, so
/// already-entered callers count as blocked while waiting for async progress.
struct Driver<F> {
    future: Pin<Box<F>>,
    waker: Waker,
    notified: flume::Receiver<()>,
    notify: Arc<Notify>,
}

impl<F: Future> Source<F::Output> for Driver<F> {
    fn poll(&mut self) -> Option<F::Output> {
        // A verification poll must not re-poll idle tracked tasks: that would
        // change the clock epoch and invalidate its own verification round.
        if !self.notify.ready.swap(false, Ordering::AcqRel) {
            return None;
        }
        match self
            .future
            .as_mut()
            .poll(&mut Context::from_waker(&self.waker))
        {
            Poll::Ready(value) => Some(value),
            Poll::Pending => None,
        }
    }

    fn block(&mut self, bell: &flume::Receiver<Bell>) -> Result<F::Output, Bell> {
        loop {
            let reason = flume::Selector::new()
                .recv(&self.notified, |_| None)
                .recv(bell, |reason| Some(reason.unwrap_or(Bell::Expired)))
                .wait();
            if let Some(reason) = reason {
                return Err(reason);
            }
            if let Some(value) = self.poll() {
                return Ok(value);
            }
        }
    }
}

impl Executor for LocalExecutor {
    type Timer = ManualClockTimer;

    fn timer(&self, deadline: Instant) -> Self::Timer {
        self.clock.timer_at(deadline)
    }

    fn spawn_detached<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.scheduler.spawn(future).detach();
    }

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        self.clock.sleep_async(duration)
    }

    fn now(&self) -> Instant {
        self.clock.now()
    }
}
