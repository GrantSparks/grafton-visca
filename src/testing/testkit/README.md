# Test Infrastructure: Executor Selection Strategy

## Problem Statement

The `DeterministicExecutor` has fundamental issues when handling timeout-based tests, causing tests to hang indefinitely. This is due to the complex interaction between virtual time and the runtime's internal timeout handling (see issue #394).

## Solution: Separate Test Strategies

Rather than trying to force deterministic execution on inherently time-dependent code, we use different executors for different test scenarios:

### Driving a `DeterministicExecutor`

`Executor::block_on` is **not** available on `DeterministicExecutor`: it panics
immediately (see issue #600). A naive `block_on` cannot advance the virtual clock,
so any future that awaits a timer parks forever, which is exactly the hang that
issue #394 diagnosed. Use one of the drivers that do advance virtual time:

| Driver | Use when |
| --- | --- |
| `DeterministicExecutor::run_until(fut)` | The future borrows from its environment or is not `Send`/`'static`. Polls the future in place while driving background tasks and advancing time. |
| `DeterministicExecutor::block_on_bg(fut)` | The future is `Send + 'static`. Spawns it and drives it alongside background tasks. |
| `drive_until_idle` / `drive_until_stalled` / `DeterministicClock::advance` | You want to single-step the executor or the clock by hand. |

Both drivers panic instead of spinning forever if no forward progress is made, so
a livelock surfaces as a test failure rather than a hung CI job.


### When to Use DeterministicExecutor

Use `DeterministicExecutor` for:
- Logic and sequencing tests
- Protocol correctness tests
- Tests that don't rely on actual timeouts
- Tests where deterministic execution order is important
- Tests that need to control time advancement explicitly

Example:
```rust
use grafton_visca::testing::testkit::deterministic_executor::DeterministicExecutor;

#[test]
fn test_protocol_logic() {
    let (executor, clock) = DeterministicExecutor::new();
    // Test protocol logic without relying on actual timeouts
}
```

### When to Use Real Runtime Executors

Use real runtime executors (Tokio, smol) for:
- Timeout behavior tests
- Tests that require actual time progression
- Tests involving concurrent timeouts
- Shutdown with pending timeouts
- Integration tests with real timing requirements

Example:
```rust
#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn test_timeout_behavior() {
    use grafton_visca::TokioExecutor;
    let executor = std::sync::Arc::new(TokioExecutor::from_current().unwrap());
    // Test actual timeout behavior
}
```

## Test Utilities

The `executor_selection` module provides utilities to help choose the appropriate executor:

- `TestExecutorType`: Enum of available executor types
- `TestExecutorSelector`: Trait for selecting appropriate executors
- `logic_test!` macro: Creates tests with DeterministicExecutor
- `timeout_test!` macro: Creates tests with real runtime executors

## Migration Guide

When updating existing tests:

1. **Identify test type**: Does the test rely on actual timeout behavior?
   - Yes → Use real runtime executor
   - No → Use DeterministicExecutor

2. **Update test attributes**:
   - For timeout tests: Use `#[tokio::test]` or equivalent
   - For logic tests: Use `#[test]` with DeterministicExecutor

3. **Document the choice**: Add a comment explaining why a particular executor was chosen

## Background

The root cause of the timeout handling issues:

1. **Virtual Time vs Runtime Coordination**: The `DeterministicExecutor` uses virtual time, but the runtime spawns background loops that manage timeouts using different timing mechanisms.

2. **Infinite Loop Potential**: This creates situations where the executor waits for the runtime, and the runtime waits for time advancement, resulting in deadlock.

3. **Complex Interactions**: The `block_on` implementation would need to handle virtual time advancement, background task coordination, and timeout future handling - making it fragile and hard to maintain. Because that entry point could only ever mislead, `DeterministicExecutor::block_on` now panics and points callers at `run_until` / `block_on_bg` or at a real runtime (issue #600).

## Best Practices

1. **Default to logic tests**: When possible, design tests to verify logic rather than timing
2. **Isolate timeout behavior**: Keep timeout-specific tests separate and minimal
3. **Document executor choice**: Always explain why a particular executor was chosen
4. **Use test utilities**: Leverage the provided macros and utilities for consistency
