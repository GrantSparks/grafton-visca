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
* Raw and Sony framing reuse their warmed buffers. Where policy has proof that
  replay is safe (same-sequence Sony recovery or a conclusive camera
  rejection), a retry reuses the already encoded command rather than rebuilding
  wire bytes, and every retry remains inside the one admission-to-terminal
  total budget. A successfully sent raw command is never replayed after ACK,
  completion, or cancellation ambiguity, after a raw receive fault while
  awaiting ACK, or after active retry-budget expiry in `Sending`, `AwaitingAck`,
  `AwaitingCompletion`, or `Executing`; ambiguity is never converted into a
  replay merely to preserve this allocation property.
* Lifecycle storage is bounded by owner admission, completion observers,
  cancellation waiters, and the diagnostic/applied-state queue limits.
* Private request, transmission, and correlation-generation identifiers are
  monotonic within a session and are never reused. Exhausting a 64-bit identity
  space fails admission/transmission with `RuntimeIdentityExhausted` instead of
  allowing a stale owner input to alias new work.
* Dynamic noun calls do not add an unnecessary outer future box compared with
  one explicitly boxed static async call. Dynamic/static construction parity is
  measured at the future-construction boundary.

These guarantees do not prohibit one allocation when an API explicitly returns
an owned collection, such as a blocking diagnostic drain. They prohibit hidden
per-frame, per-retry, per-handle, or unbounded queue growth.

## Existing characterization tests

The stable test boundaries are:

| Contract | Test |
| --- | --- |
| Representative built-in and typed request encoding | `tests/issue_548_allocation_baselines.rs`, zero-allocation tests near the end of the file |
| Warmed raw/Sony framing reuse | `tests/issue_548_allocation_baselines.rs`, framing test near the end of the file |
| Safe retry encoded-command reuse | `runtime::engine` unit test `inert_wire_is_inline_and_reused_across_retry_without_reallocation`, plus `runtime::owner` unit test `warmed_raw_and_sony_retries_reuse_owner_buffers`; raw coverage must be limited to conclusive rejection, never ambiguity or active-budget expiry |
| Bounded submission and completion observers | `tests/issue_561_blocking_queue.rs` and the `runtime::owner` unit tests |
| Dynamic/static future construction | `tests/dyn_api_integration_test.rs`, `tokio_dynamic_future_construction_matches_one_explicit_static_box` |
| Downstream derive encoding | `tests/issue_517_inquiry_encoding` coverage |

The commands in this section are the canonical local/release validation
surface. GitHub Actions currently runs the corresponding feature combinations
as broad `--all-targets` entries in its `feature-matrix` job, including the
`test-utils,blocking,runtime-tokio` union; it does not invoke this document's
commands as separate allocation-only CI legs. Run the script and keep these
named boundaries in the release review so a cfg change cannot turn a local
release check into zero executed tests.

## Runtime validation and retry matrix

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

The canonical local/release validation matrix must cover, at minimum. This is
the maintainer command surface; the GitHub Actions feature matrix runs its
feature combinations as separate jobs rather than claiming to execute every
line below verbatim:

```text
cargo test --no-default-features --features blocking
cargo test --no-default-features --features async
cargo test --no-default-features --features runtime-tokio
cargo test --no-default-features --features runtime-smol
cargo check --no-default-features --features runtime-tokio,runtime-smol
cargo test --no-default-features --features runtime-tokio,dyn-api,test-utils \
  --test dyn_api_integration_test
cargo test --no-default-features --features runtime-smol,dyn-api,test-utils \
  --test dyn_api_smol_integration_test
cargo test --no-default-features --features serde,schemars,ts-rs
cargo test --no-default-features --features test-utils
bash .github/scripts/check-blocking-dependency-boundary.sh
```

Add blocking+async coexistence and the applicable serial combinations. The
blocking dependency gate is a release contract: `blocking` and
`transport-serial` must remain native synchronous graphs with no Tokio, smol,
or async executor dependency. The
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
cargo +1.88.0 check --workspace --all-targets --no-default-features
cargo audit
```

Run compile-fail/API-contract tests under blocking, canonical async, dyn, and
coexistence feature sets. Run the fuzz/property suite and the bounded named
pure-library Miri suite with failures treated as failures, not advisory output.
The contributor command is `bash .github/scripts/miri-tests.sh`; it runs the
named Miri filters and feature compile checks, rather than an unbounded Miri
library sweep. Run Synemantic's
external compatibility check once the service/project credentials are
available. Hardware tests are separate and are tracked in
[`hardware_release_checklist.md`](hardware_release_checklist.md).

Serialization, schema, and TypeScript checks validate public data shapes only;
they must not introduce a second protocol or semantic registry. Keep generated
artifacts or snapshots in the release review when the selected ecosystem tool
requires them, and review changes as API changes rather than accepting output
blindly.

See [`observability_and_recovery.md`](observability_and_recovery.md) for the
bounded owner resources that form part of the allocation budget.
