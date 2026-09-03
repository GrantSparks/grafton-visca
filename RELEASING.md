# Releasing grafton-visca 2.0

This checklist is the source of truth for the 2.0 prerelease and final
release. The workspace contains two crates, and publication is irreversible.
Qualify hardware first from an immutable, non-release tag on the reviewed 2.0
pull-request tip. Publish only from a clean `main` commit whose pull request
passed every required CI job, including blocking, async, dynamic, and
API-contract checks.

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

Run the hardware-free gates on the commit proposed for qualification. The
strict release-tag validator runs after the bench evidence and dated changelog
heading are committed; it is expected to reject the still-pending unpublished
hardware candidate. The hardware rows in
[`docs/hardware_release_checklist.md`](docs/hardware_release_checklist.md)
must be assigned and have evidence before any 2.0 release tag is created; an
unfilled row is `Pending (Not run)`.

Ratified decision D8 in #732 applies the hardware gate to every publishable
release tag with major version 2 or higher, including `2.0.0-rc.*` and other
prereleases. The validation script rejects such a tag when any checklist table
cell is `Pending`, `Pending (Not run)`, `Blocked`, or `Fail` (in any letter
case), when the checklist carries no table row under a `Status` column, when a
required scenario ID is absent or duplicated, when the unpublished candidate
provenance is incomplete, or when the checklist has no non-placeholder
`Final sign-off:` and `Evidence index:` records. Every checklist status cell
must be exactly `Pass`; software CI does not constitute hardware evidence.

Pending hardware work belongs only on an unpublished `hardware-candidate-*`
tag described below. That tag is a qualification anchor, not a semantic
version, release, crates.io package, or permission to describe hardware as
verified.

Release tags must not carry semver build metadata: `v2.0.0+meta` has exactly the
same precedence as `v2.0.0`, so the validator refuses metadata-bearing tags
outright rather than letting one release be published under two identities.

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

## Unpublished hardware qualification tag

Before the final merge to `main`, choose a reviewed 2.0 pull-request tip for
physical qualification. The exact commit must already have a successful run
of `.github/workflows/ci.yml`. Create and push an annotated tag whose name
starts with `hardware-candidate-`, embeds the intended release version, and
ends with an eight-digit UTC date plus a positive attempt number:

```sh
git tag -a hardware-candidate-v2.0.0-rc.1-20260903.1 \
  -m "grafton-visca 2.0.0-rc.1 hardware qualification attempt 1"
git push origin refs/tags/hardware-candidate-v2.0.0-rc.1-20260903.1
```

The non-`v` prefix is a safety boundary. It does not match `ci.yml`'s
`v*.*.*` release-tag push selector, and both
`publish-existing-release.yml` and `verify-release-tag.sh` reject it as a
release input. Never invoke publication for this tag. Treat it as immutable:
if any source, dependency, feature, profile, protocol, or runtime behavior
changes, retain the old tag as historical evidence, create a new numbered
hardware-candidate tag, and rerun the affected hardware matrix.

Before running the bench, record all five provenance fields in the hardware
checklist: the short tag name, annotated tag-object ID, peeled commit ID, exact
successful Actions run URL for that commit, and operator/date. Obtain the IDs
without abbreviating them:

```sh
git rev-parse refs/tags/hardware-candidate-v2.0.0-rc.1-20260903.1
git rev-parse 'refs/tags/hardware-candidate-v2.0.0-rc.1-20260903.1^{commit}'
```

Run every hardware scenario from that peeled commit and attach the artifacts
to the checklist. Commit the evidence and final sign-off to the 2.0 pull
request. From the hardware-candidate commit to the proposed release tree, only
reviewed evidence, changelog, and release metadata may change. Any executable
or dependency change invalidates the qualification and requires a new
hardware-candidate tag and hardware pass.

## Release tag and publication order

Once hardware evidence is complete, prepare the final release metadata in the
2.0 pull request and rerun the full matrix. Only after maintainers accept the
implementation and evidence should that pull request merge to `main`. Wait for
CI to pass on the exact resulting `main` commit, rerun the strict validator
there, then create the immutable annotated release tag, for example:

```sh
bash .github/scripts/validate-release.sh v2.0.0-rc.1
git tag -a v2.0.0-rc.1 -m "grafton-visca 2.0.0-rc.1"
git push origin v2.0.0-rc.1
```

An external repository ruleset must make release tags protected and immutable
before this workflow is enabled: disallow force-updates and deletion of
release tags, and limit who can create them. The workflow checks
`git ls-remote --refs` at
provenance, immediately before each crate publication, and during final
verification, comparing the remote tag object's ID with the validated local
annotated-tag object. Those repeated checks detect a tag that drifted between
checkpoints, but they cannot atomically prevent a force-move after a check and
do not replace server-side protected/immutable-tag enforcement.

The release automation should validate the tag and manifests, package the
macro crate, dry-run and publish `grafton-visca-macros`, wait for its exact
registry index entry, then package, dry-run, and publish `grafton-visca`.
Before any source or CI validation, the automation accepts only the strict tag
syntax, resolves the fully qualified `refs/tags/<tag>` ref, and verifies that
it is an annotated tag whose peeled commit is the checked-out `HEAD`. It then
queries the Actions workflow-runs API for `.github/workflows/ci.yml` at that
exact commit, follows pagination, and requires the latest run/attempt to be
completed successfully. Manual publication is not part of this checklist. For
each crate, the automation builds the local `.crate` payload and, after the
exact registry version is visible, downloads the crates.io payload and requires
SHA-256 and byte-for-byte equality before treating publication (including a
retry of an existing version) as successful. No release is complete until both
package results are independently verified.

For a final release, repeat the same process with `2.0.0`: update both
workspace packages and the exact macro dependency together, move the changelog
notes to the final heading, rerun the complete matrix against a new unpublished
hardware-candidate tag, complete the hardware checklist and its final
sign-off/evidence records, generate a fresh local lockfile for packaging, and
use a new immutable `v2.0.0` tag. Do not mark any hardware row complete without
the required bench evidence.

## Partial-publication recovery

If the macro crate publishes but the main crate fails, do not change the
source behind that version. Diagnose the failure and retry the main package
from the exact tagged commit only when its payload is unchanged; the automated
registry-payload equality gate must pass for that retry. If source or package
metadata must change, choose a new patch version, update both crates and the
changelog, and run the complete candidate process again. Never move a tag to
bypass a failed validation gate. Protected/immutable tag rules remain a
repository prerequisite: the workflow's repeated remote-object checks can
detect drift between checkpoints, but cannot atomically prevent a force-move
after a check or substitute for server-side enforcement.

After successful publication, verify both package pages and generated docs,
then create the repository release from the matching annotated tag. These are
post-publication checks; this document does not claim that they have been run.
