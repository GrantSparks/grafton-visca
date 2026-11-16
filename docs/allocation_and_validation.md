# Allocation and validation contract

The 2.0 release treats allocation behavior and feature coverage as part of the
public engineering contract. The goal is bounded work at the owner boundary,
not a promise that every application-level wrapper is allocation-free.

## Allocation guarantees

The following boundaries are release gates:

* Built-in `Request::write_into` encoding is zero-allocation.
* Derived command and inquiry encoders preserve the same zero-allocation hot
  path.
* Standard payloads remain in inline storage; a command does not allocate once
  merely because it is prepared for a target.
* Raw and Sony receive decoding for standard payloads that fit inline storage
  borrows each frame and recycles bounded batch storage without allocating after
  warm-up, including malformed input, for both stream and datagram reads. Larger
  custom data replies can allocate owned payloads. Datagrams are decoded directly
  from the receive buffer; only byte streams use retained framer storage.
* One immutable wire authority adds one shared allocation at session construction.
  Raw write plans
  copy their encoder; Sony plans clone the existing allocator handle. Neither
  minting nor executing a write plan allocates per-write storage.
* Raw and Sony framing reuse their warmed buffers. Where policy has proof that
  replay is safe (same-sequence Sony recovery or a conclusive camera
  rejection), a retry reuses the already encoded command rather than rebuilding
  wire bytes, and every retry remains inside the one admission-to-terminal
  total budget. A successfully sent raw command is never replayed after ACK,
  completion, or cancellation ambiguity, after a raw receive fault while
  awaiting ACK, or after active retry-budget expiry in `Sending`, `AwaitingAck`,
  `AwaitingCompletion`, or `Executing`; ambiguity is never converted into a
  replay merely to preserve this allocation property.
* Engine decisions append effects into owner-owned scratch storage, and the
  owner drains and recycles its warmed drive queue. Dispatch, write-result,
  ACK/completion and harmless frame turns allocate no effect storage after
  warm-up. Per-request admission and bounded completion-observer storage remain
  lifecycle allocations, separate from the protocol-turn guarantee.
* Async owners retain their four read, write, pacing and wake timers for the
  actor lifetime. A turn resets an existing timer instead of constructing a
  fresh timer future.
* Standard TCP receives read directly into the caller's slice, with no extra
  user-space read-ahead buffer. The owner requests at most `recv_buffer_size`
  bytes from the kernel per read.
* Each raw target retains at most 16 exact inquiry declarations across
  admitted requests, unpaid stream replies and reply-skew holds. The reservation
  union and ledger settlement sets stay inline; declarations are never merged
  through overlap. Stream ledger rows are bounded by `5A + 76` per target,
  where `A` includes that target's control reserve. New declaration admission
  fails before allocation when its fixed reservation set is full.
* Lifecycle storage is bounded by owner admission, completion observers,
  cancellation waiters, and bounded diagnostic staging and subscriber queues.
* Private request, transmission, and correlation-generation identifiers are
  monotonic within a session and are never reused. Exhausting a 64-bit identity
  space fails admission/transmission with `RuntimeIdentityExhausted` instead of
  allowing a stale owner input to alias new work.
* Runtime-profile noun calls use the same concrete futures and operation
  handles as compile-time profiles; `AnyProfile` introduces no trait-object
  projection or extra future box.

These guarantees do not prohibit one allocation when an API explicitly returns
an owned collection, such as a blocking diagnostic drain. They prohibit hidden
per-frame, per-retry, per-handle, or unbounded queue growth.

## Existing characterization tests

The stable test boundaries are:

| Contract | Test |
| --- | --- |
| Representative built-in and typed request encoding | `tests/issue_548_allocation_baselines.rs::representative_typed_requests_encode_without_heap_allocation` |
| Pure built-in command and inquiry preparation | `tests/issue_548_allocation_baselines.rs::built_in_command_and_inquiry_preparation_stay_inline`, through `testing::preparation` with `test-utils` |
| Profile-aware position preparation and decoder cloning | `tests/issue_548_allocation_baselines.rs::profiled_position_inquiry_preparation_and_decoder_clones_stay_inline`, for G2, BRC-300 and FR7 |
| Warmed receive lowering, decoding and batch recycling | `tests/issue_548_allocation_baselines.rs::warmed_receive_decoding_and_batch_recycling_do_not_allocate`, through `testing::receive::ReceiveProbe` for raw/Sony streams and datagrams |
| Warmed engine decisions and complete owner command/frame turns | `tests/issue_548_allocation_baselines.rs::warmed_owner_command_and_frame_turns_do_not_allocate`, through `testing::turns::TurnProbe`; raw/Sony streams and datagrams run the production turn driver, recursive exact-write handling, ACK/completion and terminal delivery. Admission and the bounded observer queue are staged outside the measurement. |
| Built-in typed camera view selection | `tests/issue_548_allocation_baselines.rs::{blocking_builtin_camera_and_into_typed_views_do_not_allocate,async_builtin_camera_and_into_typed_views_do_not_allocate}` |
| Warmed raw/Sony framing reuse | `tests/issue_548_allocation_baselines.rs::raw_and_sony_framing_reuse_one_warmed_buffer` |
| Safe retry encoded-command reuse | `runtime::engine` unit test `inert_wire_is_inline_and_reused_across_retry_without_reallocation`, plus `tests/issue_548_allocation_baselines.rs::warmed_raw_and_sony_retries_reuse_owner_buffers`; raw coverage must be limited to conclusive rejection, never ambiguity or active-budget expiry |
| Warmed owner-thread executor timer reset | `tests/issue_548_allocation_baselines.rs::{warmed_tokio_timer_resets_do_not_allocate,warmed_smol_timer_resets_do_not_allocate,warmed_manual_clock_timer_resets_do_not_allocate}`. The generic executor checks keep timers pending; ManualClock additionally checks immediate wakes, completed rearming and duplicate pending resets with a retained noop waker. Its wake batch stores four wakes inline; larger fanouts may spill. The actual runtime-free `ManualClockExecutor::local` retains four factories across forty applied commands, control and idle turns in `tests/issue_866_owner_timers.rs::local_manual_clock_actor_reuses_four_timer_factories`; `runtime::owner::owner_turns_reuse_the_same_four_manual_timers` also covers the private owner clock fixture. |
| TCP read size and kernel retention | Linux `transport::entry_point_tests::tcp_queue::{blocking_reads_leave_unrequested_bytes_in_kernel,tokio_reads_leave_unrequested_bytes_in_kernel,smol_reads_leave_unrequested_bytes_in_kernel}` compare the receive queue after one-byte and 256-byte caller reads; async coverage includes split transports |
| Bounded submission and completion observers | `tests/issue_561_blocking_queue.rs` and the `runtime::owner` unit tests |
| Downstream derive encoding | `tests/issue_517_inquiry_encoding` coverage |

The commands in this section are the canonical local/release validation
surface. GitHub Actions currently runs the corresponding feature combinations
as broad `--all-targets` entries in its `feature-matrix` job, including the
`test-utils,blocking,runtime-tokio` union; it does not invoke this document's
commands as separate allocation-only CI legs. Run the script and keep these
named boundaries in the release review so a cfg change cannot turn a local
release check into zero executed tests.

## Runtime validation and retry matrix

Engine timeout fields carry positive intervals; an absent retry budget is
`None`, never a zero-duration sentinel. Construction, retuning and request
admission validate absolute representability using the native owner's sampled
clock. Preparation reads no clock and does not repeat full tuning validation.
The admission check includes composed ACK/completion, cancellation/ambiguity,
and queued-attempt windows. Overflow during defensive deadline construction
saturates toward the latest representable instant, preserving monotonic order.

The one-hour bound applies to explicit retry-timing overrides. It does not cap
profile deadlines or their derived budgets and backoff: a valid two-hour
completion deadline may produce a four-hour retry budget. Engine validation
preserves that domain while rejecting zero timeouts, unordered backoffs and
intervals that cannot fit the sampled clock (#861).

This compact matrix ties the safety-critical runtime rules to their source
seams and characterization tests. The larger profile, noun, and feature
inventories remain in their dedicated gates.

| Contract | Source authority | Characterization tests |
| --- | --- | --- |
| Raw positional-candidate gate and evidence-only ACK/error routing (`Sending`/`AwaitingAck`/`AwaitingCompletion`; normally one candidate, with one audited two-candidate `Urgent` exception whose ACK/error binds to neither; terminal uncertainty moves into one keyed `Socket`/`PreAck`/`InquiryUnkeyed`/`AllResponses` hold table and cannot consume a response; `AwaitingCompletion` is the completion-only, socketless shape that holds the target channel exclusively; no command FIFO or temporal recency; inquiry FIFO only when eligible; a raw named ACK supersedes and immediately terminalizes a stale owner while socketless ACKs retain first-free compatibility) | `src/runtime/engine/mod.rs`: `raw_command_gate_blocks`, `raw_hold_blocks_dispatch`, `unique_raw_command_candidate`, `resolve_raw`, `assign_ack_socket` | `src/runtime/engine/tests.rs`: `raw_gate_serializes_pre_ack_while_sony_allows_pipeline`, `urgent_raw_command_bypasses_preack_gate_and_ambiguous_ack_binds_neither`, `raw_ack_timeout_hold_rejects_late_ack_and_protects_successor`, `raw_keyed_holds_merge_by_scope_and_expire_independently`, `raw_error_policy_requires_unique_socketless_evidence`, `raw_ack_in_awaiting_ack_uses_the_unique_command_candidate`, `raw_socket_assignment_never_falls_back_from_an_occupied_named_socket`, `raw_ack_reusing_an_occupied_socket_displaces_the_stale_owner`; `tests/issue_714_async_raw_emergency_stop.rs`; `tests/issue_673_blocking_raw_emergency_stop.rs` |
| Exact fixed-frame lengths and socket nibbles (ACK/completion/network change 3 bytes; error 4 bytes; fixed ACK/completion/error nibble `0` socketless, `1..=2` S1/S2, `3..=15` rejected; variable data reply only for socket 0) | `src/protocol/response.rs`: `decode_basic`, `decode_fixed_socket` | `src/protocol/response.rs`: `test_decode_rejects_trailing_bytes_on_fixed_replies`, `test_decode_fixed_socket_nibbles_are_strict`, `test_decode_socket_zero_data_reply_remains_variable` |
| Empty UDP discard, one overall deadline, and async cooperative yield | `src/transport/blocking/udp.rs`, `src/transport/async_udp.rs`, `src/runtime/owner/async_actor.rs` | `recv_into_with_timeout_does_not_restart_after_empty_datagram`, `recv_yields_before_polling_after_empty_datagram`, `recv_yields_after_each_empty_datagram` |
| Admission-to-terminal retry budget through every later noncancelled backoff/ready/send/ACK/execution/reply phase; default raw active expiry fails only that request and quarantines its correlation, strict opt-in poisons, safe ready expiry retains its last cause, and cancellation quarantine is never shortened | `src/runtime/engine/mod.rs`: `schedule_retry`, `next_due`, `apply_due`, `receive_fault` | `retry_budget_expires_while_awaiting_sony_ack`, `retry_budget_expires_while_executing_sony_command`, `retry_budget_expires_while_awaiting_inquiry_reply`, `raw_ready_retry_budget_expiry_reports_last_error_without_poisoning`, `raw_active_retry_budget_expiry_quarantines_and_fails_per_request`, `raw_active_retry_budget_expiry_poisons_under_strict_opt_in` |
| Monotonic IDs and closed exhaustion boundary | `src/runtime/engine/mod.rs`: `IdAllocator`, `allocate_request_id`, `allocate_transmission_id`, `allocate_generation` | `src/runtime/engine/tests.rs`: `request_transmission_and_generation_allocators_stop_at_exhaustion` |
| Owner-only controls and urgent cancellation obeying/advancing shared spacing | `src/runtime/engine/mod.rs`: `drain_pending_cancellations`, `emit_cancel`; `src/noun_table.rs`: owner-control dispositions | `requested_cancellation_waits_for_shared_command_spacing`, `pending_cancellation_transmits_before_ordinary_work_and_advances_spacing`, `tests/api_contract/fail/system_protocol_controls_are_internal.rs` |
| Profile inquiry gates and applied-axis support before encoding/admission | `src/prepared.rs`: `prepare_inquiry`, `lower_targeted_settlement`, `lower_applied_only_settlement` | `src/prepared.rs`: `gated_builtin_inquiry_rejects_unsupported_runtime_surface`, `unsupported_applied_only_axes_fail_before_encoding`; `tests/issue_551_request_contract.rs` |

## Feature and API validation

The canonical feature matrix is maintained in
`.github/scripts/test-all-features.sh`. Inspect its `list-json` output for the
exact commands and run its test, clippy and doc modes; CI builds its jobs from
that same list rather than a copied command block:

```text
bash .github/scripts/test-all-features.sh list-json
bash .github/scripts/test-all-features.sh
bash .github/scripts/test-all-features.sh clippy
bash .github/scripts/test-all-features.sh doc
bash .github/scripts/check-blocking-dependency-boundary.sh
```

The blocking dependency gate is a release contract: `blocking` and
`transport-serial` remain native synchronous graphs without an async executor.
The
`issue_548_supported_surface_inventory` test is the closed inventory authority
for profiles, transports, capability gates, nouns, derives, and test utilities;
the built-in command ledger in `src/command/semantics.rs` remains the sole
semantic source of truth.

## Tooling release gates

Run the following on the tagged release candidate, with the required tools
installed:

```text
cargo +nightly fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo test --doc --no-default-features
cargo test --doc --all-features
cargo check --workspace --all-targets --no-default-features
cargo audit
```

Run compile-fail/API-contract tests under blocking, canonical async, dyn, and
coexistence feature sets. Run the fuzz/property suite with failures treated as
failures, not advisory output. Run Synemantic's
external compatibility check once the service/project credentials are
available. Hardware tests are separate and are tracked in
[`hardware_release_checklist.md`](hardware_release_checklist.md). Under #753's
proportional policy (which supersedes #738/D8 in #732), an RC may have pending
hardware rows only when its release notes state that no physical hardware
validation has been performed and no hardware support has been verified for
that release candidate.

Serialization, schema, and TypeScript checks validate public data shapes only;
they must not introduce a second protocol or semantic registry. Keep generated
artifacts or snapshots in the release review when the selected ecosystem tool
requires them, and review changes as API changes rather than accepting output
blindly.

See [`observability_and_recovery.md`](observability_and_recovery.md) for the
bounded owner resources that form part of the allocation budget.
