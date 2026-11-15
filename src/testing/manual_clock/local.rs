//! Runtime-free scheduling and task results on the owner's one manual clock.

use std::{
    fmt,
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

use super::{Bell, ManualClock, ManualClockTimer, Source};
use crate::{
    executor::{ExecError, Executor},
    Error,
};

/// The runtime-free scheduling backend selected by `ManualClockExecutor::local`.
///
/// It uses the same manual clock as its wrapping executor and drives tasks only
/// while that executor's `run_until` or `block_on` is running.
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
}

/// A detached task's join future. Panics and lost runtime tasks return errors.
pub struct TaskJoin<T: 'static> {
    result: flume::r#async::RecvFut<'static, Result<T, ExecError>>,
}

impl<T: 'static> fmt::Debug for TaskJoin<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TaskJoin")
            .field("result", &self.result)
            .finish()
    }
}

impl<T: 'static> TaskJoin<T> {
    pub(super) fn new(receiver: flume::Receiver<Result<T, ExecError>>) -> Self {
        Self {
            result: receiver.into_recv_async(),
        }
    }
}

impl<T: 'static> Future for TaskJoin<T> {
    type Output = Result<T, ExecError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.get_mut().result).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => Poll::Ready(Err(ExecError::TaskFailed(
                "task ended without a result".into(),
            ))),
        }
    }
}

/// Catch task-body panics at the poll boundary; never catch the caller's driver.
pub(super) struct TaskOutcome<F>(Pin<Box<F>>);

impl<F> TaskOutcome<F> {
    pub(super) fn new(future: F) -> Self {
        Self(Box::pin(future))
    }
}

impl<F: Future> Future for TaskOutcome<F> {
    type Output = Result<F::Output, ExecError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match catch_unwind(AssertUnwindSafe(|| self.get_mut().0.as_mut().poll(cx))) {
            Ok(Poll::Ready(value)) => Poll::Ready(Ok(value)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| {
                        payload
                            .downcast_ref::<&str>()
                            .map(|message| (*message).to_owned())
                    })
                    .unwrap_or_else(|| "task panicked with a non-string payload".to_owned());
                Poll::Ready(Err(ExecError::TaskFailed(message)))
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
    type Join<T>
        = TaskJoin<T>
    where
        T: Send + 'static;
    type Detach = ();
    type Timer = ManualClockTimer;

    fn timer(&self, deadline: Instant) -> Self::Timer {
        self.clock.timer_at(deadline)
    }

    fn spawn_with_detach<F>(&self, future: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (result, receiver) = flume::bounded(1);
        self.scheduler
            .spawn(async move {
                let value = TaskOutcome::new(future).await;
                let _ = result.send(value);
            })
            .detach();
        (TaskJoin::new(receiver), ())
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
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

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        self.clock.sleep_async(duration)
    }

    fn timeout<'a, F, T>(
        &'a self,
        duration: Duration,
        future: F,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        futures_lite::future::or(async move { Ok(future.await) }, async move {
            self.clock.sleep_async(duration).await;
            Err(Error::io_timeout())
        })
    }

    fn now(&self) -> Instant {
        self.clock.now()
    }
}
