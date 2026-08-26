# Releasing grafton-visca 2.0

This checklist is the source of truth for the 2.0 prerelease and final
release. The workspace contains two crates, and publication is irreversible.
Run a release from a clean `main` commit whose pull request passed every
required CI job, including blocking, async, dynamic, and API-contract checks.

## Version and changelog

1. Choose the release version. The first candidate is `2.0.0-rc.1`.
2. Set `[workspace.package].version` in `Cargo.toml`; the macro crate inherits
   this value from the workspace.
3. Pin the main crate's `grafton-visca-macros` dependency to the exact same
   version (`=2.0.0-rc.1`, or the final version being prepared).
4. Keep the root `Cargo.lock` ignored: this is a library workspace and release
   validation must work from a clean clone without a tracked lockfile. The
   validator checks both package manifests and the exact macro dependency with
   lockfile-independent `cargo metadata`. The publication workflow generates a
   checkout-local lockfile immediately before its locked package/publish
   commands.
5. Keep the 2.0 notes under `## [Unreleased]` until the release commit is
   ready. At release time, move them to `## [2.0.0-rc.1] - YYYY-MM-DD` (or the
   final version) and restore an empty `Unreleased` heading.

The tag, both package manifests, the exact macro dependency, and changelog
heading must agree. Do not reuse a published version for different source.

## Candidate validation

Run the release gates on the candidate commit. The hardware rows in
[`docs/hardware_release_checklist.md`](docs/hardware_release_checklist.md)
must be assigned and have evidence before a hardware claim is made; an
unfilled row is `Pending (Not run)`.

The release workflow treats `2.0.0-rc.*` as a candidate: pending hardware rows
are allowed and must remain honestly marked. A final `2.x.y` tag is different:
the validation script rejects it when any checklist table cell is `Pending`,
`Pending (Not run)`, `Blocked`, or `Fail`, or when the checklist has no
non-pending `Final sign-off:` and `Evidence index:` records. Every checklist
status cell must be `Pass`; candidate and software CI gates do not constitute
hardware evidence.

```sh
cargo metadata --no-deps --format-version 1
bash .github/scripts/validate-release.sh v2.0.0-rc.1
bash .github/scripts/test-validate-release.sh
cargo +nightly fmt --all -- --check
bash .github/scripts/test-all-features.sh
cargo clippy --all-targets --all-features -- -D warnings

RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo test --doc --all-features
cargo test --all-features --test api_stability_test
cargo test --all-features --test issue_542_async_facade
cargo test --all-features --test issue_555_observability
cargo metadata --no-deps --format-version 1
git diff --check
```

Also run the supported no-default, blocking-only, async-runtime, dynamic, and
coexistence matrices documented in the repository before tagging. Check the
declared MSRV, inspect the generated documentation, and run the project's
security/audit checks. Hardware, registry, and Synemantic validation results
must be recorded separately; passing local builds does not imply those
results.

Inspect package contents before publication:

```sh
cargo generate-lockfile
cargo package -p grafton-visca-macros --locked --no-verify
cargo package -p grafton-visca-macros --list
cargo package -p grafton-visca --list
```

The main package cannot resolve its exact same-version macro dependency from a
registry until the macro package is available there. Package and publish the
macro first, then perform the main-package dry run after the exact index entry
is visible.

## Candidate tag and publication order

After the release pull request is merged and all candidate evidence is
approved, create an immutable annotated tag, for example:

```sh
git tag -a v2.0.0-rc.1 -m "grafton-visca 2.0.0-rc.1"
git push origin v2.0.0-rc.1
```

The release automation should validate the tag and manifests, package the
macro crate, dry-run and publish `grafton-visca-macros`, wait for its exact
registry index entry, then package, dry-run, and publish `grafton-visca`.
Manual publication is not part of this checklist, and no release is complete
until both package results are independently verified.

For a final release, repeat the same process with `2.0.0`: update both
workspace packages and the exact macro dependency together, move the changelog
notes to the final heading, rerun the complete matrix, complete the hardware
checklist and its final sign-off/evidence records, generate a fresh local lockfile
for packaging, and use a new immutable `v2.0.0` tag. Do not mark any hardware
row complete without the required bench evidence.

## Partial-publication recovery

If the macro crate publishes but the main crate fails, do not change the
source behind that version. Diagnose the failure and retry the main package
from the exact tagged commit only when its payload is unchanged. If source or
package metadata must change, choose a new patch version, update both crates
and the changelog, and run the complete candidate process again. Never move a
tag to bypass a failed validation gate.

After successful publication, verify both package pages and generated docs,
then create the repository release from the matching annotated tag. These are
post-publication checks; this document does not claim that they have been run.
