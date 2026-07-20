# Releasing grafton-visca 1.x

This checklist is the source of truth for a 1.x release. The workspace contains
two same-version crates, and crates.io publication is irreversible. Run the
release from a clean `main` commit whose pull request passed every required CI
job, including both blocking and async/dynamic semver checks.

## 1. Choose And Record The Version

1. Choose the next semantic version. Do not infer it from the current
   `Unreleased` section: fixes normally use a patch release; additive public API
   normally uses a minor release.
2. Set `[workspace.package].version` in `Cargo.toml`.
3. Pin the `grafton-visca-macros` dependency in the main crate to the exact same
   version (`=X.Y.Z`). Both package manifests inherit the workspace version.
4. Run `cargo check --workspace --no-default-features` to refresh the two local
   workspace entries in `Cargo.lock`, then verify that no unrelated dependency
   versions changed.
5. Move the release notes from `## [Unreleased]` to
   `## [X.Y.Z] - YYYY-MM-DD`, then restore an empty `Unreleased` heading.

The release workflow rejects a tag unless the `vX.Y.Z` tag, workspace packages,
macro dependency, lockfile, and changelog heading all agree.

## 2. Validate The Release Commit

Run the same gates expected by CI:

```sh
cargo +nightly fmt --all -- --check
bash .github/scripts/test-all-features.sh
cargo clippy --all-targets --all-features -- -D warnings

RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --no-default-features
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --no-default-features --features runtime-tokio
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo test --doc --no-default-features
cargo test --doc --no-default-features --features runtime-tokio
cargo test --doc --all-features

cargo +1.88.0 check --workspace --all-targets --no-default-features
cargo +1.88.0 check --workspace --all-targets --no-default-features --features runtime-tokio,dyn-api,test-utils

cargo test --no-default-features --test api_stability_test
cargo test --no-default-features --features runtime-tokio --test api_stability_test
cargo test --no-default-features --features runtime-tokio,dyn-api --test api_stability_test
cargo audit
```

Inspect the publish file lists and build the macro payload before tagging:

```sh
cargo package -p grafton-visca-macros --locked --no-verify
cargo package -p grafton-visca-macros --list
cargo package -p grafton-visca --list
```

Cargo cannot build the main `.crate` archive until its exact same-version macro
dependency exists in the registry. The workflow packages and dry-runs the main
crate only after the macro package is published and its exact index entry is
visible. The pre-tag file-list inspection still catches accidental inclusions or
omissions without pretending the dependency can already resolve.

## 3. Tag And Publish

After the release pull request is merged and required checks pass on `main`:

```sh
git tag -s vX.Y.Z -m "grafton-visca X.Y.Z"
git push origin vX.Y.Z
```

The tag workflow then:

1. validates tag/version/changelog consistency;
2. packages the macro payload and inspects both crate file lists;
3. dry-runs and publishes `grafton-visca-macros` with `--locked`;
4. polls crates.io for the exact macro version with a bounded timeout;
5. packages, dry-runs, and publishes `grafton-visca` with `--locked`; and
6. verifies that both exact versions are available from crates.io.

Do not manually publish the main crate first. Do not create or move a release
tag to bypass a failed validation gate.

## 4. Partial-Publish Recovery

If the macro crate publishes but the main crate fails, do not reuse the version
for different source. The macro package is already immutable.

1. Keep the tag and source unchanged.
2. Diagnose the main-crate dry-run or publish failure.
3. If the fix does not change either package payload, rerun the failed workflow
   job or publish the main crate from the exact tagged commit.
4. If source or package metadata must change, choose a new patch version, update
   both workspace versions and the changelog, and run the complete process again.

After success, verify both package pages and docs.rs, then create the GitHub
release from the signed tag using the matching changelog section.
