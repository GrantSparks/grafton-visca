# Decision record: Temporal observation window for `is_moving` (#781)

Status: Accepted — 2026-10-02

Decision number: D22 (`docs/issue_542_design_review.md`). Implemented in PR
#786, adjusted by D27 (#788).

## Decision

1. `MotionQuery` carries an explicit observation `window` (default
   `MotionQuery::DEFAULT_WINDOW`, 100 ms, matching `IdleWait`'s default
   interval).
2. The baseline snapshot is read, and the owner clock is sampled when it has
   been received. The final snapshot may start only at least `window` after
   that instant, so every selected axis is compared across at least `window`.
   Profile inquiry pacing can only lengthen the window.
3. `true` means some selected axis changed by more than its tolerance across
   the window. `false` means no movement was detected over at least
   `window`. It is not proof of physical rest; creep within tolerance, or a
   return to the start position within the window, is not detected.
4. Insufficient evidence is an error, never `false`. A zero or
   unrepresentable window is `InvalidParameter` before any I/O. A window that
   cannot elapse before the single owner-clock deadline (`window` + the
   inquiry budget) is `Timeout`.
5. One pure `MotionWindow` core serves the blocking, async and both dynamic
   facades.
6. `MotionQuery` and `IdleWait` are `#[non_exhaustive]` with constructors
   and `with_*` methods (D27). Fields stay readable, and a later stability
   field can be added without a break.
7. `IdleWait` / `wait_until_idle` semantics are unchanged; `IdleWait.interval`
   already separates samples.

## Alternatives not adopted

- Richer result enum (moving / not detected / insufficient): insufficient
  evidence is already a distinct `Err`, and `settled()` evidence (D23)
  carries the sampling vocabulary.
- Builder with private fields: `#[non_exhaustive]` plus constructors gives
  the same evolvability with readable fields, matching the D27 rule.
- Multiple samples across the window: multiplies inquiry load; the endpoint
  comparison captures cumulative drift.
- A fixed internal window: callers could not trade latency for sensitivity.

## Dependencies

- D23 (settlement evidence) describes polled stability with this window and
  tolerance vocabulary.
- D27 (#788) fixes the struct shape.
