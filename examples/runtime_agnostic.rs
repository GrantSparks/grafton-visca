//! Runnable custom [`Executor`](grafton_visca::Executor) adapter.
//!
//! This deliberately small reference uses one OS thread per spawned task,
//! `pollster` to drive futures, and `async-io` for timers. It favors clarity over
//! scalability while exercising every timing/task primitive that a
//! caller-provided grafton-visca executor must implement.
//!
//! A real camera integration would pass `CustomExecutor` to the async
//! `Session::open` constructor together with a caller-owned async transport,
//! then drive application futures with `CustomExecutor::block_on`. Production
//! integrations should adapt their existing runtime instead of creating a thread
//! for every spawned task.
//!
//! Run with:
//! ```sh
//! cargo run --example runtime_agnostic --features async
//! ```

use std::{future::Future, pin::Pin, time::Duration};

use grafton_visca::{Error, ExecError, Executor};

/// Minimal executor adapter built from runtime-neutral async crates.
#[derive(Debug, Clone, Copy)]
struct CustomExecutor;

impl CustomExecutor {
    fn new() -> Self {
        Self
    }
}

impl Executor for CustomExecutor {
    type Join<T>
        = Pin<Box<dyn Future<Output = Result<T, ExecError>> + Send + 'static>>
    where
        T: Send + 'static;

    // The spawned task is explicitly detached below, so no extra token is needed.
    type Detach = ();

    fn spawn_with_detach<F>(&self, future: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (result_tx, result_rx) = flume::bounded(1);
        let spawn_error_tx = result_tx.clone();
        let spawn_result = std::thread::Builder::new()
            .name("grafton-visca-custom-executor".to_string())
            .spawn(move || {
                let result = pollster::block_on(future);
                let _ = result_tx.send(Ok(result));
            });
        if let Err(error) = spawn_result {
            let _ = spawn_error_tx.send(Err(ExecError::TaskFailed(error.to_string())));
        }

        let join = async move {
            result_rx
                .recv_async()
                .await
                .map_err(|error| ExecError::JoinFailed(error.to_string()))?
        };

        (Box::pin(join), ())
    }

    fn block_on<F>(&self, future: F) -> F::Output
    where
        F: Future,
    {
        pollster::block_on(future)
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        async move {
            async_io::Timer::after(duration).await;
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn timeout<'a, F, T>(
        &'a self,
        duration: Duration,
        future: F,
    ) -> impl Future<Output = Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        async move {
            futures_lite::future::race(async move { Ok(future.await) }, async move {
                async_io::Timer::after(duration).await;
                Err(Error::Timeout)
            })
            .await
        }
    }
}

fn main() -> Result<(), Error> {
    let executor = CustomExecutor::new();

    let task = executor.spawn(async { 6_u8 * 7 });
    let answer = executor.block_on(task).map_err(Error::from)?;
    println!("spawn/join result: {answer}");

    executor.block_on(executor.sleep(Duration::from_millis(5)));
    println!("sleep completed");

    let quick =
        executor.block_on(executor.timeout(Duration::from_millis(50), async { "ready" }))?;
    println!("quick timeout-wrapped future: {quick}");

    let timed_out = executor.block_on(executor.timeout(Duration::from_millis(5), async {
        async_io::Timer::after(Duration::from_millis(50)).await;
    }));
    match timed_out {
        Err(Error::Timeout) => println!("slow future timed out as expected"),
        Err(error) => return Err(error),
        Ok(()) => {
            return Err(Error::InvalidState(
                "custom executor timeout completed unexpectedly".into(),
            ));
        }
    }

    println!("custom Executor contract exercised successfully");
    Ok(())
}
