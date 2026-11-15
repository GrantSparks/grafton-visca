//! Reusable executor timers retain the selected clock across resets.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
#![allow(clippy::expect_used)]

use grafton_visca::{Executor, ResettableTimer};
use std::{
    future::Future,
    pin::pin,
    task::{Context, Poll, Waker},
    time::Duration,
};

#[cfg(feature = "runtime-tokio")]
#[tokio::test(start_paused = true)]
async fn tokio_timer_cancels_the_old_deadline_and_rearms_after_completion() {
    let executor = grafton_visca::TokioExecutor::from_current().expect("Tokio context");
    let start = executor.now();
    let mut timer = pin!(executor.timer(start + Duration::from_secs(1)));
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(timer.as_mut().poll(&mut context), Poll::Pending);
    timer.as_mut().reset(start + Duration::from_secs(3));
    tokio::time::advance(Duration::from_secs(1)).await;
    assert_eq!(timer.as_mut().poll(&mut context), Poll::Pending);
    timer.as_mut().await;
    assert_eq!(executor.now(), start + Duration::from_secs(3));

    timer
        .as_mut()
        .reset(executor.now() + Duration::from_secs(2));
    timer.as_mut().await;
    assert_eq!(executor.now(), start + Duration::from_secs(5));
    timer.as_mut().reset(start);
    assert_eq!(timer.as_mut().poll(&mut context), Poll::Ready(()));
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_timer_cancels_the_old_deadline_and_rearms_after_completion() {
    let executor = grafton_visca::SmolExecutor::new();
    executor.block_on(async {
        let start = executor.now();
        let mut timer = pin!(executor.timer(start));
        let mut context = Context::from_waker(Waker::noop());
        assert_eq!(timer.as_mut().poll(&mut context), Poll::Ready(()));
        let deadline = executor.now() + Duration::from_millis(10);
        timer.as_mut().reset(deadline);
        assert_eq!(timer.as_mut().poll(&mut context), Poll::Pending);
        timer.as_mut().await;
        assert!(executor.now() >= deadline);
        timer.as_mut().reset(start);
        assert_eq!(timer.as_mut().poll(&mut context), Poll::Ready(()));
    });
}
