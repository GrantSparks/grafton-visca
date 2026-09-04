# Releasing grafton-visca 2.0

This document is the source of truth for the 2.0 prerelease and final
release. The workspace contains two crates, and publication is irreversible.
Software release gates, the exact release tag, and CI on that exact release
commit are required for every publication. Physical-camera evidence is a
separate claim: an RC may be published while hardware remains explicitly
unverified, while a stable release with major version 2 or higher requires a
targeted representative hardware pass.

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

Run the software gates on the commit proposed for release. An RC can proceed
with the hardware checklist still marked `Pending (Not run)` or `Unverified`,
provided the release notes and checklist make that status plain and do not
describe hardware support as verified.

For a stable release whose major version is 2 or higher, run the five targeted
representative scenarios in
[`docs/hardware_release_checklist.md`](docs/hardware_release_checklist.md).
Record the exact firmware and transcript or capture link for each scenario,
plus the operator and date for the pass. A neighboring model, firmware, or
software-only test does not substitute for the representative hardware pass.

Before that stable hardware pass, finalize the Rust sources, Cargo manifests,
and dependency metadata at one commit. Record its full SHA as
`Hardware-tested commit:` in the checklist. Tests, documentation, workflows,
and release records may change afterward, but shipped Rust sources and Cargo
manifests may not. The publication workflow compares those source and manifest
paths between the recorded bench commit and the release `HEAD`; any difference
requires another targeted hardware pass.

Release tags must not carry semver build metadata: `v2.0.0+meta` has exactly
the same precedence as `v2.0.0`, so the validator refuses metadata-bearing tags
rather than allowing one release to be published under two identities.

Run the release checks, including the exact-tag and exact-commit CI checks,
before creating the release tag:

```sh
cargo metadata --no-deps --format-version 1
bash .github/scripts/test-validate-release.sh
python3 .github/scripts/validate-change-record.py "$(git merge-base HEAD origin/main)" HEAD
bash .github/scripts/test-validate-change-record.sh
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

The change-record validator requires a full-history checkout. It keeps every
released changelog section immutable, requires public API snapshot changes to
carry an Unreleased record, requires `**BREAKING**` records to name an issue,
and requires explanatory commit bodies for source changes. Correct an old
release note with a dated superseding Unreleased entry; never rewrite the old
text to make the current release look internally consistent.

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

## Release tag and publication order

Once the software implementation and release metadata are complete, rerun the
full software matrix. For a stable release, complete the targeted hardware
checklist before publication; for an RC, retain the explicit unverified status
if hardware has not been run. Only after maintainers accept the implementation
and its documented evidence should the pull request merge to `main`.

Wait for CI to pass on the exact resulting `main` commit, rerun the release
validator there, and create an immutable annotated release tag:

```sh
bash .github/scripts/validate-release.sh v2.0.0-rc.1
git tag -a v2.0.0-rc.1 -m "grafton-visca 2.0.0-rc.1"
git push origin v2.0.0-rc.1
```

The release tag must be annotated and must peel to the checked-out `HEAD`. The
publication workflow queries the Actions API for the exact
`.github/workflows/ci.yml` path and exact release `HEAD` SHA, follows
pagination, and requires the latest run/attempt to be completed successfully.
An external repository ruleset should make release tags protected and
immutable by disallowing force-updates and deletion and limiting who can
create them.

The release automation validates the tag and manifests, packages the macro
crate, dry-runs and publishes `grafton-visca-macros`, waits for its exact
registry index entry, then packages, dry-runs, and publishes `grafton-visca`.
Before any source or CI validation, it accepts only the strict tag syntax,
resolves the fully qualified `refs/tags/<tag>` ref, and verifies that the
annotated tag peels to the checked-out `HEAD`. It also verifies the exact
release-HEAD CI run and compares source/manifests with the recorded stable
bench commit when stable hardware evidence is required. For each crate, the
automation builds the local `.crate` payload and, after the exact registry
version is visible, downloads the crates.io payload and requires SHA-256 and
byte-for-byte equality before treating publication (including a retry of an
existing version) as successful. No release is complete until both package
results are independently verified. A dispatch runs the current publication
workflow while checking out the immutable release tag. It configures an
isolated `CURL_HOME/.curlrc` with the payload verifier's descriptive crates.io
User-Agent outside that checkout, so an old tagged verifier can be retried
without changing tagged source or package bytes.

For a final release, repeat the same process with `2.0.0`: update both
workspace packages and the exact macro dependency together, move the changelog
notes to the final heading, rerun the complete software matrix, complete the
targeted hardware checklist and its evidence/sign-off if this is a stable
release, generate a fresh local lockfile for packaging, and use a new immutable
`v2.0.0` tag. If hardware was not run for an RC, keep that fact visible in the
checklist and release notes.

## Partial-publication recovery

If the macro crate publishes but the main crate fails, do not change the
source behind that version. Diagnose the failure and retry the main package
from the exact tagged commit only when its payload is unchanged; the automated
registry-payload equality gate must pass for that retry. If source or package
metadata must change, choose a new patch version, update both crates and the
changelog, and run the release validation again. For a stable major-2-or-later
release, a source or manifest change after the bench pass also requires a new
targeted hardware pass. Never move a tag to bypass a failed validation gate.

After successful publication, verify both package pages and generated docs,
then create the repository release from the matching annotated tag. These are
post-publication checks; this document does not claim that they have been run.
