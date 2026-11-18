use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use grafton_visca::runtime::Executor;

/// Counts actor tasks and timer factories through the selected executor.
#[derive(Clone)]
pub(crate) struct CountingExecutor<E> {
    pub(crate) inner: E,
    pub(crate) spawned: Arc<AtomicUsize>,
    pub(crate) timer_calls: Arc<AtomicUsize>,
}

impl<E: Executor> Executor for CountingExecutor<E> {
    type Timer = E::Timer;

    fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
        self.timer_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.timer(deadline)
    }

    fn spawn_detached<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        self.inner.spawn_detached(future);
    }

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        self.inner.sleep(duration)
    }

    fn now(&self) -> std::time::Instant {
        self.inner.now()
    }
}
