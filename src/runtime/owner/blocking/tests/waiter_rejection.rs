//! Actual blocking waiters over the deterministic owner boundary harness.
use super::*;
use crate::runtime::owner::{boundary, tests::waiter_rejection as fixture};

#[test]
fn blocking_two_axis_waiter_restarts_after_owner_expiry() {
    let clock = ManualClock::new();
    let _participant = clock.enter();
    let mut state = OwnerState::new(fixture::policy(), clock.now()).unwrap();
    let (core, ends) = OwnerHandleCore::new(&state);
    let permits = core.permits.clone();
    let owner = BlockingOwnerHandle {
        core,
        clock: Clock::Manual(clock.clone()),
        worker_id: thread::current().id(),
        worker: Arc::new(Mutex::new(None)),
    };
    let queries = fixture::queries(owner.validated_tuning());
    let caller_clock = clock.clone();
    let registration = clock.register();
    let caller = thread::spawn(move || {
        let _participant = registration.enter();
        let deadline = caller_clock.now() + fixture::BUDGET;
        let error = owner
            .wait_until_idle(fixture::wait(), &queries)
            .unwrap_err();
        assert!(matches!(error, Error::Timeout { .. }));
        assert_eq!(
            error.failure_context(),
            Error::query_timeout().failure_context()
        );
        // The harness injects the owner's selected_at=deadline while this
        // caller is parked. Catch the shared clock up before restarting:
        // every subsequent admission then uses a real, unexpired budget.
        caller_clock.advance(deadline.saturating_duration_since(caller_clock.now()));
        owner
            .wait_until_idle(fixture::wait(), &queries)
            .expect("immediate fresh two-axis waiter must not see RuntimeQueueFull");
    });
    let owner_clock = Clock::Manual(clock.clone());
    let admission = owner_clock
        .select()
        .recv(&ends.receivers.admissions, |value| value.unwrap())
        .wait();
    fixture::assert_axis(&admission, 0x47);
    // The same selected_at seam as the pure owner turns deterministically
    // makes the owner win before the blocking caller services its timer.
    boundary::tests::expire_waiter_from_owner(admission, &mut state, clock.now() + fixture::BUDGET);
    for register in [0x47, 0x48, 0x47, 0x48] {
        // Clock-aware receive allows the actual waiter's sample interval
        // to advance only after both owner and caller have parked.
        let admission = owner_clock
            .select()
            .recv(&ends.receivers.admissions, |value| value.unwrap())
            .wait();
        fixture::answer_position(&mut state, admission, register, &clock);
    }
    caller.join().unwrap();
    assert_eq!(permits.available(), 1);
    assert!(ends.receivers.admissions.is_empty());
    let metrics = state.metrics();
    assert_eq!(metrics.admission_rejected, 1);
    assert_eq!(metrics.admitted, 4);
    assert_eq!(metrics.writes, 4);
    assert_eq!(metrics.terminal, 4);
}
