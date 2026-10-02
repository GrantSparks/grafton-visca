# 2.0 review decisions (#784)

Decision records for the proposals filed under review index #784, drafted
against `main` @ `894a2e7` (`v2.0.0-rc.2`) and ratified on 2026-10-02. Each
record keeps its original recommendation; its `## Decision` section is what
was ratified. Several decisions were expanded beyond the original
recommendation, on the principle that 2.0 is where breaking changes belong
and that extra effort now is preferred over a weaker or less stable 2.x API.

The ratified decisions are numbered D19-D27 in
`docs/issue_542_design_review.md` ("Decisions from this review").

## Summary

| Decision | Issue | Ratified | Breaking? | Effort |
| --- | --- | --- | --- | --- |
| D19 | [#777](decision-777.md) | Borrowing waits with a cached terminal result; idempotent `cancel(&mut self)`; terminal result kept after cancellation; `CancellationOutcome` non-exhaustive | Yes | M-L |
| D20 | [#783](decision-783.md) | Distinct non-retryable observer-deadline error with `OperationId`; typed stage/certainty context on timeouts | Semantic | M |
| D21 | [#779](decision-779.md) | `stop_all_motion` becomes an owner-level halt with a per-axis report and a one-time fence; raw/custom commands suppressed only when axes are declared | Yes | L-XL |
| D22 | [#781](decision-781.md) | `is_moving` observes across an explicit window (PR #786); `MotionQuery`/`IdleWait` non-exhaustive | Yes | S |
| D23 | [#782](decision-782.md) | Per-axis motion generations; supersession at admission; `settled()` returns evidence | Yes | L |
| D24 | [#780](decision-780.md) | Native worker thread is the only blocking model; no `TransportBusy` on concurrent use | Semantic | XL |
| D25 | [#776](decision-776.md) | Shared executor-free coordinator, first, with generated sequence tests | No (internal) | L |
| D26 | [#778](decision-778.md) | Per-target control reserve for typed stops; capacity is the ordinary bound | Semantic | M |
| D27 | [#788](decision-788.md) | Extensible public types become `#[non_exhaustive]` before 2.0.0 | Yes | M |

#775 (P1, raw-release deferred-slot overwrite) is fixed independently in
PR #785. Its enforced single-slot invariant is the first acceptance example
for D25.

## Implementation order

Each step is its own PR, with full local checks, merged before the next
starts:

1. #785 (#775 fix).
2. D25 coordinator (#776).
3. D19 operation handles (#777).
4. D20 failure context (#783), with the D26 error variant in the same matrix
   update if D26 is ready.
5. Prepared axis/effect metadata and per-target motion generations (shared
   by D21 and D23).
6. D26 control reserve (#778).
7. D24 blocking worker (#780).
8. D21 owner halt (#779).
9. D23 settlement evidence and supersession (#782).
10. D22 (#786, rebased and adjusted to D27's shape) can merge any time before
    D23.
11. D27 public type audit (#788).
12. API snapshot refresh, CHANGELOG and migration review, then
    `v2.0.0-rc.3`.
13. The stable hardware pass (including HW-04 for the halt), then 2.0.0.

## Dependency graph

"A -> B" means A is implemented before B.

```
#775 fix   -> D25 coordinator            first acceptance example
D25        -> D26, D24, D21, D23          built on the shared coordinator
D19        -> D20                          observation timeout leaves a live handle
D19        -> D21, D23                     per-axis results and evidence on borrowing handles
axis metadata / motion generations -> D21, D23
D26        -> D21                          admission for up to three stops per target
D24        -> D21 (blocking)               concurrent STOP dispatch
D22        -> D23                          sampling vocabulary for evidence
D27        -> rc.3                          last shape change before the snapshot freeze
```

## Cross-cutting rules

- New public report, outcome and context types from D19-D26 are
  `#[non_exhaustive]` from the start (D27).
- `Error` and `ErrorKind` are already `#[non_exhaustive]`. The new variants
  (D20 observer deadline, D21 halt supersession, D23 settlement supersession,
  D26 reserve exhausted) go into one consistent failure matrix.
- Each implementation PR adds its CHANGELOG `### Changed`/`### Removed`
  entry naming the decision and any superseded finding, a migration row or
  section, and regenerated API snapshots.

## How decisions were ratified (process for future records)

Follows the process adopted in #692 and written into `CONTRIBUTING.md`
("Changelog discipline for reversals and waivers").

1. Edit the record's `Status:` line to `Accepted`, `Accepted (scoped: ...)`,
   `Deferred to 2.x`, or `Rejected`, with the date (YYYY-MM-DD) and, if
   different from the recommendation, a sentence on the chosen variant. Answer
   or strike each open question.
2. Assign the next free decision number (D19 onward) in
   `docs/issue_542_design_review.md` and add a short entry linking the record.
3. A decision that reverses or supersedes an earlier recorded decision or
   finding gets a new record and number; it must not reuse the issue number of
   the finding it reverses, and the earlier text is not edited in place.
   Relevant here: #777 supersedes part of the #552 exit criteria; #783 revises
   the failure-matrix row from #726/#755; #780 (if a worker is adopted)
   revises the "no per-camera background workers" statement; a 2.x
   supersession rule from #782 revises the #542 settlement contract.
4. When an adopted change is implemented, add a CHANGELOG `### Changed` or
   `### Removed` entry under `## [Unreleased]` naming the decision and any
   superseded finding by issue number; mark breaking ones `**BREAKING**` with
   `(#NNN)`. Add a row or section to `docs/migration_2_0.md` wherever a prior
   user would notice. Regenerate `api/2.0.0-rc.1/*.txt` with the pinned
   toolchain in the same PR (CI requires a CHANGELOG change alongside any
   snapshot change).
5. For a rejected or deferred proposal, keep the record's "User-visible
   limitation" section and copy its substance into the relevant user
   documentation so the limitation is stated, not inferred.
6. Close or relabel the GitHub issue only after the record status is updated.
