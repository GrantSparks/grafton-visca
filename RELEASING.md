# Releasing grafton-visca 2.0

This document is the source of truth for the 2.0 prerelease and final
release. The workspace contains two crates, and publication is irreversible.
Software release gates, the exact release tag, and CI on that exact release
commit are required for every publication. Hardware smoke checks are optional
and private for both stable releases and prereleases; they require no public
device inventory, run record or sign-off (#1008).

## Version and changelog

1. Choose the release version. The stable release being prepared is `2.0.0`,
   following `2.0.0-rc.3`. The first candidate was `2.0.0-rc.1`.
2. Set `[workspace.package].version` in `Cargo.toml`; the macro crate inherits
   this value from the workspace.
3. Pin the main crate's `grafton-visca-macros` dependency to the exact same
   version (`=2.0.0` for this release).
4. Keep the root `Cargo.lock` ignored: this is a library workspace and release
   validation must work from a clean clone without a tracked lockfile. The
   validator checks both package manifests and the exact macro dependency with
   lockfile-independent `cargo metadata`. The publication workflow generates a
   checkout-local lockfile immediately before its locked package/publish
   commands.
5. Keep the 2.0 notes under `## [Unreleased]` until the release commit is
   ready. Unreleased states the net change since the last published version
   (the newest `v<version>` tag), not the churn between candidates. At release
   time, move them to `## [2.0.0] - YYYY-MM-DD` and
   restore an empty `Unreleased` heading. `validate-release.sh` requires an
   empty Unreleased section and the tag's dated heading first. Fold the notes
   from any unpublished dated section into Unreleased before the next cut.

The public API job generates listings from the base and proposed source for
`blocking`, `async` and `all-features`. Review the uploaded API diffs with
the source and changelog before release. Listings stay in CI artifacts or
`target/public-api/`; release tags preserve the source needed to regenerate
them. See [Public API Snapshots](CONTRIBUTING.md#public-api-snapshots).

The tag, both package manifests, the exact macro dependency, and changelog
heading must agree. Do not reuse a published version for different source.

## Candidate validation

Run the software gates first on the clean commit proposed in the release PR,
using its merge base with `origin/main` for the change-record comparison. The
PR run is a review gate, not the tag identity. After that PR passes and merges,
repeat the complete matrix on the exact resulting `origin/main` commit as
shown below. Only this post-merge run defines `$release_commit`; every later
paired-downstream, CI, tag, and publication check must name that same SHA.

Maintainers may run a local hardware smoke check when useful. Keep its notes
and captures private; no public hardware record is required for publication.
See the [optional smoke-check guidance](docs/hardware_release_checklist.md).

Release tags must not carry semver build metadata: `v2.0.0+meta` has exactly
the same precedence as `v2.0.0`, so the validator refuses metadata-bearing tags
rather than allowing one release to be published under two identities.

Validate only a committed, clean candidate. Do not validate a convenient
working tree and then tag a different `HEAD`. After the release PR merges, use
a detached checkout of the exact `main` commit that will be tagged, and fail
closed if either its contents or identity changes while the matrix runs. Set
`$previous_release_tag` to the immediately preceding immutable release so the
change-record check covers the release delta instead of comparing `main` with
itself:

```sh
release_tag=v2.0.0
previous_release_tag=v2.0.0-rc.3
git fetch origin main --tags
git switch --detach origin/main
test -z "$(git status --porcelain=v1 --untracked-files=all)"
release_commit="$(git rev-parse --verify HEAD^{commit})"
previous_release_commit="$(git rev-parse --verify "${previous_release_tag}^{commit}")"
printf 'Release candidate: %s (%s)\n' "$release_tag" "$release_commit"
```

Run the candidate software checks from that checkout. The canonical feature
matrix is `.github/scripts/test-all-features.sh`. Use its `list-json` mode to
inspect the supported legs and its `test`, `clippy` and `doc` modes to validate
them. The post-tag steps below cover the tag-triggered CI gate.

```sh
cargo +1.98.0 metadata --no-deps --format-version 1
bash .github/scripts/test-validate-release.sh
python3 .github/scripts/validate-change-record.py "$previous_release_commit" "$release_commit"
bash .github/scripts/test-validate-change-record.sh
cargo +nightly-2026-08-26 fmt --all -- --check
bash .github/scripts/test-all-features.sh test
bash .github/scripts/test-all-features.sh clippy
bash .github/scripts/test-all-features.sh doc
cargo +1.98.0 test --all-features --test api_stability_test
cargo +1.98.0 test --all-features --test integration issue_542_async_facade::
cargo +1.98.0 test --all-features --test integration issue_555_observability::
git diff --check
```

When the commands finish, prove that the candidate still names the same clean
tree before accepting their result:

```sh
test "$(git rev-parse --verify HEAD^{commit})" = "$release_commit"
test -z "$(git status --porcelain=v1 --untracked-files=all)"
```

The change-record validator requires a full-history checkout with tags. It
keeps changelog sections immutable once their `v<version>` tag is reachable
from the compared base. It also protects the sections below them, requires `**BREAKING**` records to name an issue, and requires
explanatory commit bodies for source changes. The public API comparison job
requires API drift to carry a changelog change. Correct a published release note
with an Unreleased entry; never rewrite the published text to make the current
release look internally consistent.

Check the declared `rust-version` and inspect the generated documentation.
The candidate's CI run includes the Cargo Deny gate for RustSec advisories,
licenses, and dependency sources. Record registry validation separately from downstream validation.

Inspect package contents before publication:

```sh
cargo +1.98.0 generate-lockfile
cargo +1.98.0 package -p grafton-visca-macros --locked --no-verify
cargo +1.98.0 package -p grafton-visca-macros --list
cargo +1.98.0 package -p grafton-visca --list
```

The main package cannot resolve its exact same-version macro dependency from a
registry until the macro package is available there. Package and publish the
macro first, then perform the main-package dry run after the exact index entry
is visible.

## Paired downstream candidate validation

The checked-in [downstream contract fixture](tests/fixtures/downstream_2_0/)
checks the 2.0 API in public CI. For a release coordinated with a downstream
consumer, also validate the exact pair of committed candidates. Keep private
repository names, commit identifiers, CI links, commands and logs in private
maintainer records. The public release record can report the result without
identifying the consumer.

Check out both recorded SHAs in clean, disposable detached worktrees. Use a
separate Cargo home to override the two packages before they are published.
Set the paths to the local grafton-visca candidate and place this configuration
outside both repositories.

```toml
# $pair_cargo_home/config.toml
[patch.crates-io]
grafton-visca = { path = "/absolute/path/to/grafton-visca-candidate" }
grafton-visca-macros = { path = "/absolute/path/to/grafton-visca-candidate/grafton-visca-macros" }
```

Run the downstream consumer's required checks from its detached worktree using
that Cargo home for workspaces that depend on grafton-visca. Keep unrelated
locked workspaces on their ordinary Cargo home. Verify both worktrees still
resolve to the recorded SHAs after the checks. Inspect any resolver-generated
lockfile changes and keep a record of them with the result. Other tracked
changes require a fresh clean candidate and a new run.

After both exact crate versions appear in the registry, repeat the downstream
checks using the released crates and record that result separately. Use the
consumer's normal lockfile update and verify that the build resolves both
crates from the registry. Run validation that requires private repository
access through the consumer's authorized private environment.

## Release tag and publication order

The release PR must pass the full software matrix before it merges.
After maintainers review and merge the PR, complete the
post-merge candidate checks and any applicable paired downstream validation.
Those steps establish the final `$release_commit`.

Wait for `.github/workflows/ci.yml` to pass for that exact `main` commit. Fetch
`origin/main` again and require it still equals `$release_commit`; if `main`
advanced, start over with the new committed candidate rather than tagging an
old validation result. Rerun the release validator from that exact checkout,
then create an immutable annotated release tag that explicitly names the
recorded commit:

```sh
git fetch origin main --tags
test "$(git rev-parse --verify origin/main^{commit})" = "$release_commit"
git switch --detach "$release_commit"
test -z "$(git status --porcelain=v1 --untracked-files=all)"
bash .github/scripts/validate-release.sh v2.0.0 --test-package
git tag -a v2.0.0 "$release_commit" -m "grafton-visca 2.0.0"
git push origin v2.0.0
```

Wait next for the CI run triggered by that **tag push** to complete
successfully. A prior `main` run for the same commit is necessary but not
sufficient: the tag run must report the exact workflow path,
`.github/workflows/ci.yml`, `event=push`, `head_branch=v2.0.0`, and
`head_sha=$release_commit`. Do not dispatch publication until that tag run is
green. The publication workflow independently queries all pages of the Actions
API and enforces the same exact tag-run identity before it can package either
crate.

The release tag must be annotated and must peel to the checked-out `HEAD`. An
external repository ruleset should make release tags protected and
immutable by disallowing force-updates and deletion and limiting who can
create them.

The release automation validates the tag and manifests, packages the macro
crate, dry-runs and publishes `grafton-visca-macros`, waits for its exact
registry index entry, then packages, dry-runs, and publishes `grafton-visca`.
The pre-tag validator's `--test-package` check builds both packages, then
runs the library's default unit tests and compiles its all-feature unit tests
from the unpacked package outside the workspace. It uses the unpacked macro
package so the check also works before that version is on crates.io.

Before source or CI validation, the release automation checks the tag syntax,
resolves the fully qualified `refs/tags/<tag>` ref, and verifies that the
annotated tag peels to the checked-out `HEAD`. It also verifies the exact
main-push and tag-push release-HEAD CI runs.
For each crate, the automation builds the local payload, waits for the exact
version on crates.io, and compares the files in the two packages. Only the embedded `Cargo.lock` may have different contents.
Library dependents resolve their own lockfiles, and matching
`.cargo_vcs_info.json` pins the source commit. The verifier prints both archive
SHA-256 digests for the record. Both packages must pass the content check.
A dispatch runs the current publication workflow while checking out the
immutable release tag. It configures an
isolated `CURL_HOME/.curlrc` with the payload verifier's descriptive crates.io
User-Agent outside that checkout, so an old tagged verifier can be retried
without changing tagged source or package bytes.

For later releases, repeat this process with the new version: update both
workspace packages and the exact macro dependency together, move the changelog
notes to the matching heading, rerun the complete software matrix, generate a
fresh local lockfile for packaging, and use a new immutable tag that matches
the version. Hardware smoke checks remain optional and private.

## Partial-publication recovery

If the macro crate publishes but the main crate fails, do not change the
source behind that version. Diagnose the failure and retry the main package
from the exact tagged commit. The registry payload verifier requires
identical package contents apart from the embedded `Cargo.lock`. If source
or package metadata must change, choose a new patch version, update both
crates and the changelog, and run the release validation again. Never move a tag to bypass a
failed validation gate.

After successful publication, verify both package pages and generated docs,
then create the repository release from the matching annotated tag. These are
post-publication checks; this document does not claim that they have been run.
