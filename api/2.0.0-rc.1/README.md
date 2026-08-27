# Public API snapshot: 2.0.0-rc.1

These files are the approved public-surface baseline for the 2.0 release
candidate. They intentionally establish a new semver baseline instead of
comparing the 2.0 API break with the 1.x API. Future changes are reviewed by
diffing the same selected feature surfaces against these files.

The snapshots are generated with the pinned `cargo-public-api` 0.52.0 tool on
the nightly toolchain pinned by `.github/workflows/ci.yml`:

```text
rustup toolchain install nightly-2026-08-26 --profile minimal
cargo install cargo-public-api --version 0.52.0 --locked
cargo +nightly-2026-08-26 public-api --color never [feature arguments]
```

`cargo public-api` builds rustdoc JSON, which only nightly rustdoc emits, and
the emitted item order and formatting change between nightly releases. Running
it through the same pinned nightly CI uses is what keeps these files a stable
byte comparison instead of a moving target; a nightly bump in the workflow has
to be accompanied by regenerated snapshots.

CI compares the complete deterministic, unfiltered `cargo public-api` output
for each surface. No simplification filters are used, so blanket, auto-trait,
and auto-derived implementations remain part of the baseline. Keeping every
public command, value, derive, profile, transport,
operation, observability, cache, and dynamic item in the snapshot prevents a
module allowlist from hiding an accidental public API change.

Surfaces tracked here are `no-default`, `blocking`, `async`, `tokio-dyn`,
`smol-dyn`, `coexistence`, and `all-features`. A snapshot update must be
intentional, regenerated with the pinned tool, and reviewed together with the
corresponding public API change.
