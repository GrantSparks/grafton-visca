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
* Raw, Sony, and retry framing reuse their warmed buffers. A retry reuses the
  already encoded command rather than rebuilding wire bytes.
* Lifecycle storage is bounded by owner admission, completion observers,
  cancellation waiters, and the diagnostic/applied-state queue limits.
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
| Retry encoded-command reuse | `runtime::engine` unit test `inert_wire_is_inline_and_reused_across_retry_without_reallocation`, plus `runtime::owner` unit test `warmed_raw_and_sony_retries_reuse_owner_buffers` |
| Bounded submission and completion observers | `tests/issue_561_blocking_queue.rs` and the `runtime::owner` unit tests |
| Dynamic/static future construction | `tests/dyn_api_integration_test.rs`, `tokio_dynamic_future_construction_matches_one_explicit_static_box` |
| Downstream derive encoding | `tests/issue_517_inquiry_encoding` coverage |

The release workflow should invoke these named tests explicitly in addition to
the broad feature matrix so a cfg change cannot turn a release gate into zero
executed tests.

## Feature and API validation

The canonical matrix must cover, at minimum:

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
```

Add blocking+async coexistence and the applicable serial combinations. The
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
coexistence feature sets. Run the fuzz/property suite and Miri library suite
with failures treated as failures, not advisory output. Run Synemantic's
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
