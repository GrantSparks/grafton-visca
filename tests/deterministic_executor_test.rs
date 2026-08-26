//! Tests for the DeterministicExecutor integration with the runtime.

#![cfg(all(feature = "test-utils", feature = "mode-async"))]

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use grafton_visca::{
    testing::testkit::deterministic_executor::{DeterministicExecutor, ExecutorExt},
    Executor,
};

/// The executor's own clock must never move on its own, and a registered sleep
/// must only fire once virtual time reaches its deadline.
#[test]
fn virtual_time_moves_only_when_the_clock_is_advanced() {
    let (executor, clock) = DeterministicExecutor::new();
    let start = executor.now();

    let fired = Arc::new(AtomicBool::new(false));
    let fired_in_task = fired.clone();
    let task_executor = executor.clone();
    executor.spawn_detached(async move {
        task_executor.sleep(Duration::from_millis(250)).await;
        fired_in_task.store(true, Ordering::SeqCst);
    });

    executor.run_until_idle();
    assert!(
        clock.has_pending_deadlines(),
        "the sleep should register a deadline with the virtual clock"
    );
    assert_eq!(
        executor.now(),
        start,
        "virtual time must not advance on its own"
    );

    clock.advance(Duration::from_millis(100));
    executor.run_until_idle();
    assert!(
        !fired.load(Ordering::SeqCst),
        "a partial advance must not fire the timer"
    );

    clock.advance(Duration::from_millis(150));
    executor.run_until_idle();
    assert!(
        fired.load(Ordering::SeqCst),
        "the timer must fire once virtual time reaches the deadline"
    );
    assert_eq!(
        executor.now().duration_since(start),
        Duration::from_millis(250),
        "virtual time should have advanced by exactly the sleep duration"
    );
}

/// A retry/backoff loop is the reason this executor exists: several seconds of
/// modelled backoff must cost virtual time, not wall-clock time.
#[test]
fn a_backoff_loop_consumes_virtual_time_not_wall_clock() {
    let (executor, _clock) = DeterministicExecutor::new();
    let virtual_start = executor.now();
    let wall_start = std::time::Instant::now();

    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_in_task = attempts.clone();
    let task_executor = executor.clone();
    executor.spawn_detached(async move {
        for _ in 0..3 {
            attempts_in_task.fetch_add(1, Ordering::SeqCst);
            task_executor.sleep(Duration::from_secs(1)).await;
        }
    });

    executor.run_until_idle();
    while executor.advance_to_next_deadline() {
        executor.run_until_idle();
    }

    assert_eq!(
        attempts.load(Ordering::SeqCst),
        3,
        "every backoff iteration should run"
    );
    assert_eq!(
        executor.now().duration_since(virtual_start),
        Duration::from_secs(3),
        "three one-second backoffs should consume exactly three virtual seconds"
    );
    assert!(
        wall_start.elapsed() < Duration::from_secs(1),
        "three virtual seconds must not cost real seconds (took {:?})",
        wall_start.elapsed()
    );
}

/// `advance_to_next_deadline` must jump to the *earliest* pending deadline, so
/// concurrent timers fire in deadline order rather than spawn order.
#[test]
fn timers_fire_in_deadline_order_not_spawn_order() {
    let (executor, _clock) = DeterministicExecutor::new();
    let order = Arc::new(Mutex::new(Vec::new()));

    for (label, delay) in [("late", 300u64), ("early", 100), ("middle", 200)] {
        let order_in_task = order.clone();
        let task_executor = executor.clone();
        executor.spawn_detached(async move {
            task_executor.sleep(Duration::from_millis(delay)).await;
            order_in_task
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(label);
        });
    }

    executor.run_until_idle();
    while executor.advance_to_next_deadline() {
        executor.run_until_idle();
    }

    let observed = order
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    assert_eq!(
        observed,
        vec!["early", "middle", "late"],
        "timers must fire in deadline order"
    );
}

#[test]
fn test_det_drives_spawned_tasks_smokescreen() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    let (executor, _clock) = DeterministicExecutor::new();
    let flag = Arc::new(AtomicBool::new(false));
    let flag2 = flag.clone();

    executor.spawn_detached(async move {
        flag2.store(true, Ordering::SeqCst);
    });

    assert!(executor.run_until_idle(), "Should have made progress");
    assert!(flag.load(Ordering::SeqCst), "Task should have run");
}

#[test]
fn test_det_sleep_fires_only_when_time_advances_smokescreen() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    let (executor, _clock) = DeterministicExecutor::new();
    let flag = Arc::new(AtomicBool::new(false));
    let flag2 = flag.clone();
    let executor2 = executor.clone();

    executor.spawn_detached(async move {
        executor2.sleep(Duration::from_millis(50)).await;
        flag2.store(true, Ordering::SeqCst);
    });

    executor.run_until_idle();
    assert!(
        !flag.load(Ordering::SeqCst),
        "Sleep should not complete without time advancement"
    );

    assert!(
        executor.advance_to_next_deadline(),
        "Should have advanced to deadline"
    );
    executor.run_until_idle();
    assert!(
        flag.load(Ordering::SeqCst),
        "Sleep should complete after time advancement"
    );
}
