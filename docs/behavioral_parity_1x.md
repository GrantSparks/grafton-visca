# Historical 1.x behavior decisions and retained direct tests

2.0 is a clean API and architecture break. Callers do not receive 1.x
compatibility shims, deprecated aliases, or the old scheduler surface. A
protocol behavior can still be retained when it is observable, useful, and
safe to carry forward. This page records those historical decisions and points
maintainers to the direct v2 regressions and wire/decode goldens that preserve
them.

## How to use this guide

The decisions below are historical evidence, not a promise that every 1.x
implementation detail remains part of the public API. The direct v2 regression
tests and goldens named here are the authoritative evidence for behavior that
the current implementation retains. When new behavior is introduced, review
the implementation and its direct tests together, decide explicitly whether it
supersedes one of these decisions, and record the user-visible consequence in
the migration guide and changelog.

The guide points to production paths and their focused test boundaries: owner,
engine, framer, encoder, and public facade. It does not introduce a second
scheduler or compatibility shim whose behavior could drift from what users
receive.

## Executable provenance gate

The guide is backed by the executable corpus at
`.github/behavioral-parity-1x/manifest.json`. It pins the complete 1.x oracle
commit `6c7a9d3783861189745536372c4d21de24d4252d`, the old source symbols that
establish each of its twelve required behavior families, and direct current-v2
production-path tests. The validator rejects a missing or duplicate family,
unreachable test definition, absent or ambiguous source symbol, invalid
raw/Sony envelope/profile/receipt evidence, unapproved intentional change, a
zero-match cargo filter, or an ignored mapped test. Every mapping is checked
against validator-owned pins for its source, exact canonical libtest path,
command, envelope, profile, and receipt class. Rust comments and
string/character literals are lexically removed before code evidence is
checked, and every grouped command must report the exact canonical path as
`ok`; suffix matches do not count.

The gate provides audited executable traceability and code evidence. It is not
a Rust semantic analyzer and does not reproduce the VISCA protocol or scheduler
as a second model. Reviewers remain responsible for confirming that the pinned
production-path test asserts the intended behavior; the validator keeps that
reviewed chain from drifting or being satisfied by prose.

Run the complete gate from a non-shallow clone:

```text
bash .github/scripts/validate-behavioral-parity.sh
```

`--skip-tests` is useful only while editing the corpus; it checks provenance
and mappings without executing Cargo and is never the CI command. The dedicated
required CI job uses `fetch-depth: 0` so `git show` reads the pinned object
rather than relying on whatever history happens to be present on a runner. The
corpus is CI machinery and is intentionally excluded from the crate payload.

## Retained behavior families

| Family                        | Decision carried into v2                                                                                                                                                                                                                                                                                                                                                              | Direct v2 evidence                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Wire bytes and reply decoding | Built-in commands retain the 1.x bytes except for the source-backed corrections ratified by #715 under `source-backed-wire-corrections`; fixed replies are length- and socket-strict, while a delimited malformed frame is discarded and a genuine framing-position loss poisons a stream. | `src/issue_715_wire_golden.rs` and `tests/fixtures/issue_715_wire_golden.txt`; `tests/issue_633_golden_wire_bytes.rs`; `src/protocol/sony.rs`; `tests/issue_672_674_681_decode_consequence.rs` |
| Retry budgets                 | Retry counts remain category-based and one admission-to-terminal wall-clock budget covers every later noncancelled phase. Sony retries reuse the same sequence; an ambiguous raw send is never replayed.                                                                                                                                                                              | `src/runtime/engine/tests.rs`: `retry_budget_expires_while_awaiting_sony_ack`, `retry_budget_expires_while_executing_sony_command`, `retry_budget_expires_while_awaiting_inquiry_reply`, and the raw expiry tests                                                                                                                                                                                                                                        |
| Raw correlation               | Raw VISCA ordinarily admits one unacknowledged command per target before ACK, never guesses by FIFO or recency, and reopens socket concurrency only after ownership is established. One intrinsic `Urgent` command may cross one candidate; while both are open, ACK/error binds to neither (#714).                                                                                      | `src/runtime/engine/tests.rs`: `raw_gate_serializes_pre_ack_while_sony_allows_pipeline`, `urgent_raw_command_bypasses_preack_gate_and_ambiguous_ack_binds_neither`, `raw_error_policy_requires_unique_socketless_evidence`, `raw_ack_in_awaiting_ack_uses_the_unique_command_candidate`; `tests/issue_714_async_raw_emergency_stop.rs`                                                                 |
| ACK socket assignment         | A named ACK is exact when its socket is free. If a raw camera names a locally occupied socket, that authoritative reuse supersedes the stale owner, which moves to an unkeyed quarantine (#721); inventing the other socket would misroute cancellation and completion. Sequence-correlated Sony retains the 1.x other-free-socket fallback (#620/#682). Socketless ACKs retain first-free compatibility. | `src/runtime/engine/tests.rs`: `raw_socket_assignment_never_falls_back_from_an_occupied_named_socket`, `raw_ack_reusing_an_occupied_socket_displaces_the_stale_owner`; `tests/issue_565_transport_faults_blocking.rs` and `tests/issue_565_transport_faults_async.rs` (sequenced fallback) |
| Raw uncertainty               | By default, an unconfirmable raw command immediately fails only that request with `UnsequencedCommandUnconfirmed`, leaves an inert keyed correlation hold, and keeps the session live. `strict_unconfirmed_poison` opts into whole-session `StreamPoisoned`; a receive fault poisons immediately only before cancel intent is recorded, while a recorded cancel resolves through its own live pre-ACK/socket cancellation deadline. | `src/runtime/engine/tests.rs`: `raw_active_retry_budget_expiry_quarantines_and_fails_per_request`, `raw_active_retry_budget_expiry_poisons_under_strict_opt_in`, `raw_receive_fault_leaves_unacked_command_and_keeps_the_session`, `raw_receive_fault_poisons_under_strict_opt_in`, `strict_raw_receive_fault_after_cancel_uses_cancellation_resolution`; `tests/issue_565_transport_faults_blocking.rs` and `tests/issue_565_transport_faults_async.rs` |
| Receive and write failures    | A fatal receive closure is normalized to `ConnectionClosed` with the cause text retained. `StreamPoisoned` is reserved for an unknowable stream framing or write position; datagram write failure remains per-request.                                                                                                                                                                | `src/runtime/owner/tests.rs`: `fatal_blocking_read_fault_closes_the_stream_session`; `src/runtime/engine/tests.rs`: `stream_write_failure_poisons_with_the_transport_cause_in_the_reason`; `tests/issue_565_transport_faults_blocking.rs`                                                                                                                                                                                                                |
| Sustained receive faults      | Intentional 2.0 bound: 1.x kept retrying every non-`ConnectionClosed` read error indefinitely. In 2.0, twelve consecutive transient receive faults spanning at least one second close the session as `ConnectionClosed`; a successful read or a five-second gap resets the run. | `src/runtime/owner/tests.rs`: `persistent_transient_faults_close_the_blocking_session_with_their_cause`, `persistent_transient_faults_close_the_async_session_with_their_cause`, `idle_reads_do_not_break_a_transient_fault_run` |
| Cancellation and lifecycle    | Cancellation is an intent and an observable outcome, not an implicit timeout or detach. Close, shutdown, poison, and old operation handles retain their distinct terminal behavior.                                                                                                                                                                                                   | `src/runtime/owner/tests.rs` cancellation/close tests; `tests/issue_680_engine_poison_session_error.rs`; `tests/issue_554_owner_cancellation_blocking.rs`                                                                                                                                                                                                                                                                                                |

## Decisions that need particular care

### Delimited malformed frames

The framer owns the byte boundary. If it has already found an `FF` terminator,
the strict response classifier may reject the resulting frame without making
the stream unusable: the owner records `Ignored(MalformedFrame)` and continues.
Only losing the framing position — for example an unrecoverable buffer
overflow or a boundary-free read beyond `max_buffer_size` — is a stream poison.
Datagrams have the same discard-and-continue consequence because each datagram
is an independent framing unit.

### Raw uncertainty and issue #671

An ACK/completion/cancellation ambiguity or active retry-budget expiry after a
raw send does not justify replaying a possibly executed physical action. The
default result is `UnsequencedCommandUnconfirmed` for that request only. Its
socket or sole raw candidate slot remains quarantined until the ambiguity
deadline so a late reply cannot bind to later work. A transient receive fault
while raw work awaits ACK likewise never authorizes a replay, but by default it
leaves the command in `AwaitingAck`; only a subsequent ACK deadline without an
ACK produces that unconfirmed result. Reconcile the camera effect before
deliberately resubmitting. The strict opt-in instead poisons the whole session
on that receive fault and reports `StreamPoisoned`.

### Receive taxonomy

A read that proves the connection is gone consumes no bytes, so it does not
make the stream's framing position unknowable. Owners report it as
`ConnectionClosed` and retain the underlying receive error's text in the
reason. A failed stream write or unrecoverable framer loss has a different
failure mode: its byte position cannot be established, so it reports
`StreamPoisoned` and requires a fresh session.

## Built-in request policy audit

The request-policy audit reads the built-in command ledger and the concrete
request declarations; it does not maintain a second semantic registry. The
current ledger has 149 command rows, 63 queryable inquiry rows, and 11
decode-only response rows. Every queryable row uses the generated inquiry
policy, with an inquiry-specific deadline and the interim 1 s response default.
The historical comparison is the 1.x timeout category in the original command
declarations; the v2 retry class controls which evidence-backed error paths may
replay, while the timeout category controls the retry count.

The rows with an intentional timeout-category decision are:

| v2 rows                                                                                                                                           | 1.x category            | v2 policy           | Decision                                                                                                                                                                                                                                    |
| ------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `PanTiltStop`, `ZoomStop`, `FocusStop`                                                                                                            | Movement                | Quick / Movement    | Urgent stop deadline; retain movement retry/error semantics.                                                                                                                                                                                |
| `PanTiltLimitSet`, `PanTiltLimitClear`                                                                                                            | Movement                | Quick / Standard    | Plain limit-state edits are not actuation.                                                                                                                                                                                                  |
| `FocusAuto`, `FocusManual`, `FocusToggle`                                                                                                         | Movement                | Quick / Standard    | Focus-mode settings are plain configuration.                                                                                                                                                                                                |
| `FocusOnePush`, `FocusSnap`                                                                                                                       | Movement                | Quick / Movement    | Applied-only focus triggers retain movement retry/error semantics with an urgent deadline.                                                                                                                                                  |
| `IrisReset`, `IrisUp`, `IrisDown`, `IrisDirect`                                                                                                   | Quick                   | Movement / Movement | Targeted iris operations use profile-selected protocol settlement inquiries.                                                                                                                                                                |
| `NdFilterDirect`, `NdFilterStepUp`, `NdFilterStepDown`                                                                                            | Quick                   | Movement / Movement | Targeted ND-filter operations use profile-selected protocol settlement inquiries.                                                                                                                                                           |
| `Sharpness*`, `Gamma`, `NoiseReduction2d`, `NoiseReduction2dOff`, `NoiseReduction3d`, `NoiseReduction3dOff`, `ImageFlipBoth`, `ImageFlipCombined` | Custom                  | Quick / Standard    | The old `Custom` value was the uncategorized 60 s fallback; these are explicit quick configuration writes in v2. The NR level/off rows are restored 1.x controls, and the v1.0.0/v1.1.0 release-tag declarations classify them as `Custom`. |
| 63 queryable built-in inquiries                                                                                                                   | Quick                   | Inquiry / Inquiry   | Inquiry response timing is a separate profile fact: v2 uses an interim 1 s deadline while retaining the old quick retry budget.                                                                                                             |
| `NoiseReduction2dMode`                                                                                                                            | No 1.x command category | Quick / Standard    | New v2 `01 04 50` control. 1.x exposed an NR-mode inquiry but no corresponding setter, so this row is neither a preserved category nor an intentional category change.                                                                      |

The remaining 119 command rows retain their 1.x timeout category. Every command
row has an explicit retry class. The semantic unit test
`timeout_category_partition_preserves_the_1x_provenance_boundary` checks the
149-row universe and the 29 changed / 1 new-v2-without-1.x-category / 119
preserved partition. It additionally pins the current v2 `Quick`/`Standard`
policy for the four restored NR rows and the new-v2 mode control; it does not
reconstruct the historical category. Update this table and its explanation
whenever an explicit partition set moves.
`CommandCancel` remains `Quick`/`Never`; `PushAfPress` and `PushAfRelease`
retain movement error handling; and `PresetSet`/`PresetReset` retain their
preset timeout and retry behavior even though their v2 semantic class is plain.

## Adding or changing behavior

When a behavior changes or a regression is found:

1. Add or update a direct v2 regression test at the production owner, engine,
   framer, encoder, or public facade boundary. Add a wire/decode golden when
   exact bytes or classification are the contract.
2. Describe the decision here when it is useful historical context, including
   the issue or review that motivated it and the direct test that demonstrates
   the current result.
3. Update `docs/migration_2_0.md` and `CHANGELOG.md` when a caller can observe
   the change.
4. Review the new behavior directly against the implementation and tests. Do
   not preserve an old behavior merely because it once existed if it would
   make correlation, framing, cancellation, or physical safety less certain.

Hardware evidence remains separate. Scripted transports and direct owner/engine
tests establish deterministic software behavior; the hardware release checklist
establishes what a particular camera, network, serial adapter, or firmware does.
