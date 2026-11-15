use std::{
    future::Future,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use grafton_visca::{Error, Executor};

/// Counts actor tasks and timer factories through the selected executor.
#[derive(Clone)]
pub(super) struct CountingExecutor<E> {
    pub(super) inner: E,
    pub(super) spawned: Arc<AtomicUsize>,
    pub(super) timer_calls: Arc<AtomicUsize>,
}

impl<E: Executor> Executor for CountingExecutor<E> {
    type Join<T>
        = <E as Executor>::Join<T>
    where
        T: Send + 'static;
    type Detach = <E as Executor>::Detach;

    type Timer = E::Timer;

    fn timer(&self, deadline: std::time::Instant) -> Self::Timer {
        self.timer_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.timer(deadline)
    }

    fn spawn_with_detach<F>(&self, future: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        self.inner.spawn_with_detach(future)
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.inner.block_on(future)
    }

    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        self.inner.sleep(duration)
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
        self.inner.timeout(duration, future)
    }

    fn now(&self) -> std::time::Instant {
        self.inner.now()
    }
}
