# Public API snapshot: 2.0.0-rc.1

These files are the approved public-surface baseline for the 2.0 release
candidate. They intentionally establish a new semver baseline instead of
comparing the 2.0 API break with the 1.x API. Future changes are reviewed by
diffing the same selected feature surfaces against these files.

They exist only for CI and for review; `api/` is in the `exclude` list of the
root `Cargo.toml`, so none of it ships in the published crate.

## Regenerating

The snapshots are generated with the pinned `cargo-public-api` 0.52.0 tool on
the nightly toolchain pinned by `.github/workflows/ci.yml`:

```bash
rustup toolchain install nightly-2026-08-26 --profile minimal
cargo install cargo-public-api --version 0.52.0 --locked

cargo +nightly-2026-08-26 public-api --color never --simplified \
    --no-default-features --features blocking > api/2.0.0-rc.1/blocking.txt
cargo +nightly-2026-08-26 public-api --color never --simplified \
    --no-default-features --features runtime-tokio,dyn-api > api/2.0.0-rc.1/tokio-dyn.txt
cargo +nightly-2026-08-26 public-api --color never --simplified \
    --all-features > api/2.0.0-rc.1/all-features.txt
```

`cargo public-api` builds rustdoc JSON, which only nightly rustdoc emits, and
the emitted item order and formatting change between nightly releases. Running
it through the same pinned nightly CI uses is what keeps these files a stable
byte comparison instead of a moving target; a nightly bump in the workflow has
to be accompanied by regenerated snapshots.

## Which surfaces, and why these

Three surfaces are tracked: `blocking`, `tokio-dyn`, and `all-features`.

- `blocking` (`--no-default-features --features blocking`) is a strict superset
  of the bare `--no-default-features` surface, so it pins the core types and the
  blocking facade in one file.
- `tokio-dyn` (`--no-default-features --features runtime-tokio,dyn-api`) is a
  strict superset of the `--features async` surface, so it pins the async
  facade together with a runtime adapter and the dynamic trait-object API.
- `all-features` is the superset surface. It is what catches everything the two
  reduced legs cannot see: serde/schemars/ts-rs derives, the serial transports,
  and the smol adapter (whose items mirror the tokio ones item for item).

Together the two reduced legs cover the union of the previously tracked
`no-default`, `blocking`, `async`, `tokio-dyn`, and `coexistence` surfaces to
within a single line, and every item outside that union is inside
`all-features`. Adding a fourth surface buys duplicated coverage, not new
coverage.

## What is and is not in the files

`--simplified` (`--omit blanket-impls`) is used for every surface. Blanket
implementations such as `impl<T> Any for T`, `impl<T> Borrow<T> for T`, and
`impl<T, U> Into<U> for T` are emitted for every public type, are identical for
every public type, and are ~42% of the unfiltered output; they say nothing about
this crate's API. Everything semver-relevant is kept, including auto-trait
impls (`Send`, `Sync`, `Unpin`) — losing one of those on a public type is a real
break for downstream async callers — and auto-derived impls (`Clone`, `Debug`,
`Eq`).

No module allowlist is used, so an accidentally public command, value, derive,
profile, transport, operation, observability, cache, or dynamic item still
shows up as a diff. A snapshot update must be intentional, regenerated with the
pinned tool, and reviewed together with the corresponding public API change.
