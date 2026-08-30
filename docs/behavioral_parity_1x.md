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

## Retained behavior families

| Family | Decision carried into v2 | Direct v2 evidence |
| --- | --- | --- |
| Wire bytes and reply decoding | Built-in commands retain their documented VISCA bytes; fixed replies are length- and socket-strict, while a delimited malformed frame is discarded and a genuine framing-position loss poisons a stream. | `tests/issue_633_golden_wire_bytes.rs`; `tests/issue_672_674_681_decode_consequence.rs` |
| Retry budgets | Retry counts remain category-based and one admission-to-terminal wall-clock budget covers every later noncancelled phase. Sony retries reuse the same sequence; an ambiguous raw send is never replayed. | `src/runtime/engine/tests.rs`: `retry_budget_expires_while_awaiting_sony_ack`, `retry_budget_expires_while_executing_sony_command`, `retry_budget_expires_while_awaiting_inquiry_reply`, and the raw expiry tests |
| Raw correlation | Raw VISCA admits one unacknowledged command per target before ACK, never guesses by FIFO or recency, and reopens socket concurrency only after ownership is established. | `src/runtime/engine/tests.rs`: `raw_gate_serializes_pre_ack_while_sony_allows_pipeline`, `raw_error_policy_requires_unique_socketless_evidence`, `raw_ack_in_awaiting_ack_uses_the_unique_command_candidate` |
| ACK socket assignment | A named ACK is exact when its socket is free. If that socket is occupied but another target socket is free, the uniquely identified candidate falls back to that socket; the ACK is inert only when no socket is free. Socketless ACKs retain first-free compatibility. | `src/runtime/engine/tests.rs`: `socket_assignment_falls_back_from_an_occupied_named_socket_and_never_invents_one`, `ack_naming_an_occupied_socket_falls_back_to_the_free_socket` |
| Raw uncertainty | By default, an unconfirmable raw command fails only that request with `UnsequencedCommandUnconfirmed`, quarantines its correlation, and leaves the session live. `strict_unconfirmed_poison` opts into whole-session `StreamPoisoned`. | `src/runtime/engine/tests.rs`: `raw_active_retry_budget_expiry_quarantines_and_fails_per_request`, `raw_active_retry_budget_expiry_poisons_under_strict_opt_in`, `raw_receive_fault_leaves_unacked_command_and_keeps_the_session`, `raw_receive_fault_poisons_under_strict_opt_in`; `tests/issue_565_transport_faults_blocking.rs` and `tests/issue_565_transport_faults_async.rs` |
| Receive and write failures | A fatal receive closure is normalized to `ConnectionClosed` with the cause text retained. `StreamPoisoned` is reserved for an unknowable stream framing or write position; datagram write failure remains per-request. | `src/runtime/owner/tests.rs`: `fatal_blocking_read_fault_closes_the_stream_session`; `src/runtime/engine/tests.rs`: `stream_write_failure_poisons_with_the_transport_cause_in_the_reason`; `tests/issue_565_transport_faults_blocking.rs` |
| Cancellation and lifecycle | Cancellation is an intent and an observable outcome, not an implicit timeout or detach. Close, shutdown, poison, and old operation handles retain their distinct terminal behavior. | `src/runtime/owner/tests.rs` cancellation/close tests; `tests/issue_680_engine_poison_session_error.rs`; `tests/issue_554_owner_cancellation_blocking.rs` |

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

An ACK/completion/cancellation ambiguity, a transient receive fault while raw
work awaits ACK, or active retry-budget expiry after a raw send does not justify
replaying a possibly executed physical action. The default result is
`UnsequencedCommandUnconfirmed` for that request only. Its socket or sole raw
candidate slot remains quarantined until the ambiguity deadline so a late reply
cannot bind to later work. Reconcile the camera effect before deliberately
resubmitting. The strict opt-in restores a whole-session poison and reports
`StreamPoisoned`.

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
current ledger has 149 command rows, 68 queryable inquiry rows, and 11
decode-only response rows. Every queryable row uses the generated inquiry
policy, with an inquiry-specific deadline and the interim 1 s response default.
The historical comparison is the 1.x timeout category in the original command
declarations; the v2 retry class controls which evidence-backed error paths may
replay, while the timeout category controls the retry count.

The rows with an intentional timeout-category decision are:

| v2 rows | 1.x category | v2 policy | Decision |
| --- | --- | --- | --- |
| `PanTiltStop`, `ZoomStop`, `FocusStop` | Movement | Quick / Movement | Urgent stop deadline; retain movement retry/error semantics. |
| `PanTiltLimitSet`, `PanTiltLimitClear` | Movement | Quick / Standard | Plain limit-state edits are not actuation. |
| `FocusAuto`, `FocusManual`, `FocusToggle` | Movement | Quick / Standard | Focus-mode settings are plain configuration. |
| `FocusOnePush`, `FocusSnap` | Movement | Quick / Movement | Applied-only focus triggers retain movement retry/error semantics with an urgent deadline. |
| `IrisReset`, `IrisUp`, `IrisDown`, `IrisDirect` | Quick | Movement / Movement | Targeted physical aperture operations have exact iris settlement inquiries. |
| `NdFilterDirect`, `NdFilterStepUp`, `NdFilterStepDown` | Quick | Movement / Movement | Targeted physical filter operations have exact ND settlement inquiries. |
| `Sharpness*`, `Gamma`, `NoiseReduction2d*`, `NoiseReduction3d*`, `ImageFlipBoth`, `ImageFlipCombined` | Custom | Quick / Standard | The old `Custom` value was the uncategorized 60 s fallback; these are explicit quick configuration writes in v2. |
| 68 queryable built-in inquiries | Quick | Inquiry / Inquiry | Inquiry response timing is a separate profile fact: v2 uses an interim 1 s deadline while retaining the old quick retry budget. |

The remaining 120 command rows retain their 1.x timeout category, and every
command row has an explicit retry class. The semantic unit test
`intentional_timeout_category_changes_account_for_the_preserved_remainder`
checks the 149-row universe and the 29 changed / 120 preserved split. Update
this table and its explanation whenever that direct test's changed set moves.
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
