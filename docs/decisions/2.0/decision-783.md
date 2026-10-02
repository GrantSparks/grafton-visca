# Decision record: Typed timeout stage and operation certainty (#783)

Status: Proposed — awaiting maintainer ratification

Decision number: assigned at ratification. If accepted, the first row of the
canonical owner failure matrix in `src/error.rs:13-32` changes; record that as
a `### Changed` CHANGELOG entry and name the classification it refines (#755;
#726 for the earlier public failure model). This does not reverse D8
(per-request raw uncertainty, #671) and must not reinstate blanket session
poisoning.

## Context

`Error::Timeout` (`src/error.rs:358-360`) is produced both for observer
deadlines on a live operation (`src/runtime/owner/async_actor.rs:1013`, `:1088`,
`:1120`; equivalent paths in `src/runtime/owner/blocking.rs`, e.g. `:1052`,
`:1064`) and for terminal engine failures after the protocol lifecycle gave up
(`src/runtime/engine/mod.rs:1685`, `:3533`, `:3568`, `:3616`, `:3645`), and for
admission-deadline expiry (`src/runtime/owner/async_actor.rs:2829-2835`). The
canonical matrix row at `src/error.rs:20` folds "observer or inquiry/response
deadline" into one outcome. `is_retryable()` (`src/error.rs:796-808`) returns
`true` for every `ErrorKind::Timeout`, so a caller who sees an observer
timeout on a still-live command and asks "retryable?" is told yes, and may
resubmit a command that is still running. `Error` and `ErrorKind` are already
`#[non_exhaustive]` (`src/error.rs:60`, `:196`), so adding a variant is not a
compile break.

## Recommendation

**Accept for 2.0 (narrow: distinct observation-deadline error); defer the
structured stage/certainty report to 2.x.**

What lands in 2.0:

1. A new `Error` variant (name illustrative: `ObservationTimeout`) returned
   only when a caller-side observer deadline on an admitted operation,
   cancellation or inquiry expires while the owner still holds the request.
   `kind()` maps it to `ErrorKind::Timeout` so kind-based code is unchanged;
   `requires_new_session()` is `false`; `is_retryable()` is `false`, with
   documentation that the correct response is to wait again (with #777) or
   reconcile, never to resubmit.
2. `Error::Timeout` keeps meaning "the engine/protocol lifecycle reached a
   terminal deadline" (and other existing non-observer uses).
3. The matrix row in `src/error.rs` is split into the two cases, and
   `is_retryable()` documentation states it classifies temporary conditions
   and is not a replay-safety predicate (no rename).

Deferred to 2.x (additive): a small structured outcome (stage: pre-admission /
observation / terminal / cancellation-attempt / session; certainty: known,
failed-conclusively, unconfirmed) attached to new report types or accessor
methods, and recovery examples per stage.

## Rationale

The one ambiguity that can lead an application to duplicate a physical command
is "observer timeout on a live command" versus "terminal timeout". That needs
a type-level distinction, and changing which variant an existing path returns
is a semantic change that is cheap at rc and awkward after 2.0.0. The rest of
the proposal (certainty snapshots, stage enums) is additive because `Error`
is non-exhaustive and new report types can sit next to existing results.
Pre-admission rejection is already distinct (`RuntimeQueueFull`,
`RuntimeShutdown`), and the unconfirmed cases already have dedicated variants
(`UnsequencedCommandUnconfirmed`, `CancellationUnconfirmed`).

## Breaking-change impact

Semantically breaking, compile-compatible. Code matching `Error::Timeout` for
observer deadlines must also match the new variant; code relying on
`is_retryable()` being `true` for an observer timeout changes behavior (this
is the intent). Must land before 2.0.0 final; afterwards, moving existing
paths to a new variant would be a 3.0-class change. The structured report can
be added in 2.x without breakage.

## Rough effort

S-M. `src/error.rs` (variant, `kind`, `is_retryable`, `requires_new_session`,
`suggested_retry_delay`, matrix), observer-deadline sites in
`src/runtime/owner/{async_actor.rs,blocking.rs,mod.rs}` (the normalization at
`src/runtime/owner/mod.rs:2346-2354` needs review), `tests/error_consistency_test.rs`,
`tests/timeout_category_tests.rs`, owner tests on both facades and dyn, docs
(`docs/observability_and_recovery.md`, `docs/migration_2_0.md` recovery
section), snapshot refresh.

## User-visible limitation if rejected or deferred

Without the 2.0 part, an application cannot tell from the error value whether
its own wait expired (the command may still execute) or the library gave up,
and the generic `is_retryable()` answer is "yes" in both cases. It must track
which call produced the error and treat every `Timeout` from an operation wait
as "outcome unknown, do not replay". Without the deferred part, callers derive
stage/certainty from the variant plus their own knowledge of the call site.

## Dependencies

- #777: with consuming waits, an observation timeout also loses the handle;
  the variant is still useful but the recommended "wait again" response only
  exists if #777 is accepted. Ratify #777 first.
- Interacts with #778 if a distinct control-reserve rejection variant is
  added; keep both in one matrix update.
- Independent of #775/#781 fixes.

## Alternatives considered

- Full structured stage/certainty report in 2.0: larger surface to design and
  document under freeze pressure; additive later.
- Rename/deprecate `is_retryable()`: deprecation is compatible in 2.x; a
  rename now churns every user for a documentation problem.
- Attach context via `Error::WithContext`: string context does not give code
  a typed distinction.

## Open questions for the maintainer

1. Approve a new variant for observer-deadline expiry, with `is_retryable() ==
   false`?
2. Should admission-deadline expiry (`async_actor.rs:2835`) use the new
   variant, `Error::Timeout`, or a third variant? (It is pre-admission: nothing
   was accepted, so replay is safe; this record leans to keeping it distinct
   from observation timeout.)
3. Should the new variant carry the `OperationId`?
