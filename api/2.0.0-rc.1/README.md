# Public API snapshot: 2.0.0-rc.1

These files are the approved public-surface baseline for the 2.0 release
candidate. They intentionally establish a new semver baseline instead of
comparing the 2.0 API break with the 1.x API. Future changes are reviewed by
diffing the same selected feature surfaces against these files.

The snapshots are generated with the pinned `cargo-public-api` 0.52.0 tool:

```text
cargo install cargo-public-api --version 0.52.0 --locked
cargo public-api --color never [feature arguments]
```

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
