# 2.0 review decisions (#784)

Proposed decision records for the proposals filed under review index #784,
drafted against `main` @ `894a2e7` (`v2.0.0-rc.2`). Every record has status
"Proposed — awaiting maintainer ratification"; nothing here is decided.

These records live in `docs/decisions/2.0/`, a new directory. Ratified
decisions are currently a numbered list in `docs/issue_542_design_review.md`
("Decisions from this review", items 1-18). On ratification, each accepted or
deferred record should also get the next free number in that list (D19 onward)
with a one-paragraph summary linking its record here.

## Summary

| Issue | Title | Recommendation | Breaking? | Effort | Depends on |
| --- | --- | --- | --- | --- | --- |
| [#777](decision-777.md) | Reusable operation observation, separate cancellation intent | Accept for 2.0 (borrowing `&mut self` waits with cached terminal; `CancellationOutcome` non-exhaustive); defer `&self` idempotent cancel and dual delivery to 2.x | Yes (receiver change; supersedes #552 criteria) | M | none |
| [#783](decision-783.md) | Timeout stage and operation certainty | Accept for 2.0 (distinct non-retryable observation-deadline error); defer structured stage/certainty report to 2.x | Semantic (compile-compatible) | S-M | #777 |
| [#779](decision-779.md) | Owner-level halt, prompt STOP dispatch, per-axis results | Defer to 2.x (new `halt` owner action); 2.0: async/dyn `stop_all_motion` submits all stops before awaiting, plus docs | No (2.0 slice); 2.x additive | S now; L-XL later | #778, #777, #776, #780 |
| [#782](decision-782.md) | Settlement evidence and supersession | Defer to 2.x; 2.0: document current meaning and reserve a supersession outcome | No (docs) | S now; L later | #781, #777, #779 |
| [#780](decision-780.md) | Native blocking owner worker | Defer to 2.x; 2.0: loosen blocking submission and `TransportBusy` contract in docs | No (docs) | S now; XL later | #776, #778 |
| [#776](decision-776.md) | Explicit boundary arbitration and invariant replay | Defer to 2.x; 2.0 slice is #775's enforced single-slot invariant | No (internal) | L | #775 |
| [#778](decision-778.md) | Bounded admission reserved for stop/halt | Accept for 2.0 (per-target reserve for intrinsically Urgent typed stops; distinct rejection error; `MetricsSnapshot` non-exhaustive) | Semantic (capacity meaning, new variant, non-exhaustive struct) | M | none (#776 optional) |

Handled separately and not recorded here:

- **#775** (P1, raw-release deferred-slot overwrite) is fixed in its own PR
  (#785), independently of any proposal.
- **#781** (`is_moving` temporal window) is fixed in its own PR (#786). That
  PR chooses the sampling and evidence contract, which is also awaiting
  ratification. It keeps `MotionQuery` parallel to `IdleWait` (public fields,
  not `#[non_exhaustive]`) and lists the builder/private-field alternative
  for the 2.0 freeze decision.

## Recommended order of ratification

Consistent with #784's suggested order (decide public-shape questions before
the API freeze; implement correctness fixes first and independently):

1. #775 fix merged (no ratification needed beyond review).
2. #777 operation ownership (largest API-shape decision).
3. #783 failure context (depends on what an observation timeout leaves
   behind, i.e. #777).
4. #779 halt meaning (decides what `stop_all_motion` promises in 2.0 and
   reserves the 2.x halt).
5. #781 sampling contract (in its PR), then #782 settlement evidence.
6. #780 blocking progress model.
7. #776 coordination seam scope.
8. #778 control reserve (implementation order: before the 2.x halt; it can be
   ratified any time after #779's meaning is set, and early ratification is
   fine because it does not depend on the others).

Implementation order for 2.0 items: #775, #777, #783, #778, #779 (2.0 slice),
documentation slices of #780 and #782, then API snapshot refresh, CHANGELOG,
migration rows, and only then the stable hardware pass.

## Dependency graph

"A -> B" means A should be decided or implemented before B.

```
#775 fix        -> #776 seam (2.x)          first acceptance example
#776 seam       -> #779 halt (2.x)           ordering/suppression rules
#776 seam       -> #780 worker (2.x)         shared executor-free coordination
#776 seam       -> #782 generations (2.x)    supersession rules
#777 (2.0)      -> #783 (2.0)                observation timeout leaves a live handle
#777 (2.0)      -> #782 (2.x)                re-observation / additive report method
#777 (2.0)      -> #779 (2.x)                per-axis result collection
#778 (2.0)      -> #779 (2.0 slice and 2.x)  admission for up to three stops per target
#780 (2.x)      -> #779 (blocking)           concurrent STOP dispatch in blocking mode
#780 (2.x)      -> #778 (blocking)           reserve is only reachable without owner-turn contention
#781 fix PR     -> #782 (2.x)                sampling policy that evidence describes
#779 (2.x)     <-> #782 (2.x)                share one owner-ordered motion generation
```

## Cross-cutting 2.0 items that keep 2.x additive

- `#[non_exhaustive]` on `CancellationOutcome` (#777) and `MetricsSnapshot`
  (#778); new report types introduced in 2.x (#779, #782, #783) should be
  `#[non_exhaustive]` from the start.
- `Error` and `ErrorKind` are already `#[non_exhaustive]`, so new variants for
  #778/#782/#783 are compile-compatible; reclassifying an existing path is
  still a semantic change and is only proposed for #783 before 2.0.0.
- Documentation reservations in #780 and #782 are what make their 2.x
  implementations compatible; if those are not landed, the corresponding 2.x
  work becomes a documented-contract change.

## How to ratify

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
