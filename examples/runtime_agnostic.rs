//! Runnable custom [`Executor`](grafton_visca::runtime::Executor) adapter.
//!
//! This deliberately small reference uses one OS thread per spawned task,
//! `pollster` to drive futures, and `async-io` for timers. It favors clarity over
//! scalability while exercising every timing/task primitive that a
//! caller-provided grafton-visca executor must implement.
//!
//! A real camera integration would pass `CustomExecutor` to the async
//! `Session::open` constructor together with a caller-owned async transport,
//! then drive application futures with `pollster::block_on`. Production
//! integrations should adapt their existing runtime instead of creating a thread
//! for every spawned task.
//!
//! Run with:
//! ```sh
//! cargo run --example runtime_agnostic --features async
//! ```

use std::{
    future::Future,
    pin::Pin,
    time::{Duration, Instant},
};

use grafton_visca::{runtime::Executor, runtime::ResettableTimer, Error};

/// Minimal executor adapter built from runtime-neutral async crates.
#[derive(Debug, Clone, Copy)]
struct CustomExecutor;

impl CustomExecutor {
    fn new() -> Self {
        Self
    }
}

/// Reuses async-io's timer registration when an owner changes its deadline.
#[derive(Debug)]
struct CustomTimer(async_io::Timer);

impl Future for CustomTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, context: &mut std::task::Context<'_>) -> std::task::Poll<()> {
        Pin::new(&mut self.get_mut().0).poll(context).map(|_| ())
    }
}

impl ResettableTimer for CustomTimer {
    fn reset(self: Pin<&mut Self>, deadline: Instant) {
        self.get_mut().0.set_at(deadline);
    }
}

impl Executor for CustomExecutor {
    type Timer = CustomTimer;

    fn timer(&self, deadline: Instant) -> Self::Timer {
        CustomTimer(async_io::Timer::at(deadline))
    }

    fn spawn_detached<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        std::thread::spawn(move || pollster::block_on(future));
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        async move {
            async_io::Timer::after(duration).await;
        }
    }
}

fn main() -> Result<(), Error> {
    let executor = CustomExecutor::new();

    let (sender, receiver) = flume::bounded(1);
    executor.spawn_detached(async move {
        let _ = sender.send(6_u8 * 7);
    });
    let answer =
        pollster::block_on(receiver.recv_async()).map_err(|error| Error::InvalidState {
            reason: error.to_string().into(),
        })?;
    println!("spawn/join result: {answer}");

    pollster::block_on(executor.sleep(Duration::from_millis(5)));
    println!("sleep completed");

    let mut timer = Box::pin(executor.timer(executor.now()));
    pollster::block_on(timer.as_mut());
    timer
        .as_mut()
        .reset(executor.now() + Duration::from_millis(5));
    pollster::block_on(timer.as_mut());
    println!("reusable timer reset completed");

    let quick = pollster::block_on(futures_lite::future::or(async { Ok("ready") }, async {
        executor.sleep(Duration::from_millis(50)).await;
        Err(grafton_visca::Error::io_timeout())
    }))?;
    println!("quick timeout-wrapped future: {quick}");

    let timed_out = pollster::block_on(futures_lite::future::or(
        async {
            async_io::Timer::after(Duration::from_millis(50)).await;
            Ok(())
        },
        async {
            executor.sleep(Duration::from_millis(5)).await;
            Err(grafton_visca::Error::io_timeout())
        },
    ));
    match timed_out {
        Err(Error::Timeout { .. }) => println!("slow future timed out as expected"),
        Err(error) => return Err(error),
        Ok(()) => {
            return Err(Error::InvalidState {
                reason: "custom executor timeout completed unexpectedly".into(),
            });
        }
    }

    println!("custom Executor contract exercised successfully");
    Ok(())
}
