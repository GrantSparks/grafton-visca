#!/usr/bin/env bash
set -euo pipefail

# Focused regression checks for the release validator. The fixture projects are
# temporary and use a lockfile-independent cargo metadata invocation.
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
validator="${script_dir}/validate-release.sh"
root_temp="$(mktemp -d)"
trap 'rm -rf "${root_temp}"' EXIT

make_fixture() {
    local destination="$1"
    local version="$2"

    mkdir -p "${destination}/grafton-visca-macros/src" \
        "${destination}/src" "${destination}/docs"
    python3 - "${destination}" "${version}" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
version = sys.argv[2]

(root / "Cargo.toml").write_text(f'''[workspace]
members = [".", "grafton-visca-macros"]
resolver = "2"

[workspace.package]
version = "{version}"
edition = "2021"

[package]
name = "grafton-visca"
version.workspace = true
edition.workspace = true

[dependencies]
grafton-visca-macros = {{ path = "grafton-visca-macros", version = "={version}" }}
''')
(root / "grafton-visca-macros/Cargo.toml").write_text(f'''[package]
name = "grafton-visca-macros"
version.workspace = true
edition.workspace = true

[lib]
proc-macro = true
''')
(root / "src/lib.rs").write_text("#![allow(dead_code)]\n")
(root / "grafton-visca-macros/src/lib.rs").write_text("\n")
(root / "CHANGELOG.md").write_text(
    f"# Changelog\n\n## [Unreleased]\n\n## [{version}] - 2026-08-26\n"
)
(root / ".gitignore").write_text("/Cargo.lock\n/target/\n")
PY
    git -C "${destination}" init --quiet
    git -C "${destination}" config user.email "release-validator@example.invalid"
    git -C "${destination}" config user.name "Release validator"
    git -C "${destination}" add --all
    git -C "${destination}" commit --quiet -m "fixture"
}

commit_fixture_changes() {
    local destination="$1"
    git -C "${destination}" add --all
    git -C "${destination}" commit --quiet -m "fixture update"
}

run_validator() {
    local root="$1"
    local version="$2"
    (cd "${root}" && bash "${validator}" "${version}")
}

source "${script_dir}/expect-failure.sh"

# Every stable version and prerelease uses the software metadata contract.
# No public hardware record, device inventory, or sign-off is required.
for version in 1.99.0 2.0.0-rc.1 2.0.0 3.0.0-rc.1 3.0.0 12.0.0; do
    candidate_root="${root_temp}/candidate-${version}"
    make_fixture "${candidate_root}" "${version}"
    run_validator "${candidate_root}" "v${version}"
done
complete_root="${root_temp}/candidate-2.0.0"

# Optional prose is outside the gate as well: it is neither parsed nor needed.
optional_root="${root_temp}/optional-notes"
make_fixture "${optional_root}" "2.0.0"
printf 'Optional smoke checks are local and private.\n' \
    > "${optional_root}/docs/hardware_release_checklist.md"
commit_fixture_changes "${optional_root}"
run_validator "${optional_root}" v2.0.0
rm "${optional_root}/docs/hardware_release_checklist.md"
commit_fixture_changes "${optional_root}"
run_validator "${optional_root}" v2.0.0

# A library release does not need a committed lockfile. Generating or removing
# the ignored checkout-local lock leaves the candidate clean and valid.
(cd "${complete_root}" && cargo generate-lockfile --offline)
run_validator "${complete_root}" v2.0.0
rm -f "${complete_root}/Cargo.lock"
run_validator "${complete_root}" v2.0.0

# Both published package versions must match the intended tag.
expect_failure "grafton-visca version 2.0.0 does not match tag version 2.0.1" \
    run_validator "${complete_root}" v2.0.1
macro_version_root="${root_temp}/macro-version"
make_fixture "${macro_version_root}" "2.0.0"
python3 - "${macro_version_root}" <<'PY_MUTATION'
from pathlib import Path
import sys
root = Path(sys.argv[1])
macro = root / "grafton-visca-macros/Cargo.toml"
macro.write_text(macro.read_text().replace("version.workspace = true", 'version = "2.0.1"'))
manifest = root / "Cargo.toml"
manifest.write_text(manifest.read_text().replace('version = "=2.0.0"', 'version = "=2.0.1"'))
PY_MUTATION
commit_fixture_changes "${macro_version_root}"
expect_failure "grafton-visca-macros version 2.0.1 does not match tag version 2.0.0" \
    run_validator "${macro_version_root}" v2.0.0

# A compatible range and a different exact pin both fail the paired-crate pin.
for pin in 2.0.0 '=2.0.1'; do
    pin_root="${root_temp}/pin-${pin}"
    make_fixture "${pin_root}" "2.0.0"
    python3 - "${pin_root}/Cargo.toml" "${pin}" <<'PY_MUTATION'
from pathlib import Path
import sys
path = Path(sys.argv[1])
path.write_text(path.read_text().replace('version = "=2.0.0"', f'version = "{sys.argv[2]}"'))
PY_MUTATION
    commit_fixture_changes "${pin_root}"
    expect_failure "is not pinned to the tag version =2.0.0" \
        run_validator "${pin_root}" v2.0.0
done

# The cut requires empty Unreleased notes and the intended strict date first.
for record in missing-heading wrong-version superseded misordered wrong-date-format; do
    record_root="${root_temp}/record-${record}"
    make_fixture "${record_root}" "2.0.0"
    prefix=$'# Changelog\n\n'
    case "${record}" in
        missing-heading) heading='' ;;
        wrong-version) heading='## [2.0.1] - 2026-08-26' ;;
        superseded) heading=$'## [2.0.1] - 2026-08-27\n\n## [2.0.0] - 2026-08-26' ;;
        misordered)
            prefix+=$'## [2.0.1] - 2026-08-27\n\n'
            heading='## [2.0.0] - 2026-08-26'
            ;;
        wrong-date-format) heading='## [2.0.0] - 2026/08/26' ;;
    esac
    printf '%s## [Unreleased]\n\n%s\n' "${prefix}" "${heading}" > "${record_root}/CHANGELOG.md"
    commit_fixture_changes "${record_root}"
    expect_failure "CHANGELOG.md must start its release history with '## [2.0.0] - YYYY-MM-DD'" \
        run_validator "${record_root}" v2.0.0
done

record_root="${root_temp}/record-unreleased"
make_fixture "${record_root}" "2.0.0"
printf '# Changelog\n\n## [Unreleased]\n\n- Release note.\n\n## [2.0.0] - 2026-08-26\n' \
    > "${record_root}/CHANGELOG.md"
commit_fixture_changes "${record_root}"
expect_failure "CHANGELOG.md has a non-empty [Unreleased] section" \
    run_validator "${record_root}" v2.0.0

printf '# Changelog\n\n## [Unreleased]\n\n## [2.0.0] - 2026-08-26\n\n- Release note.\n' \
    > "${record_root}/CHANGELOG.md"
commit_fixture_changes "${record_root}"
run_validator "${record_root}" v2.0.0

# Build metadata is never an alternate release identity, including on an RC.
metadata_root="${root_temp}/metadata"
make_fixture "${metadata_root}" "2.0.0+meta"
expect_failure "build metadata" run_validator "${metadata_root}" v2.0.0+meta
expect_failure "build metadata" run_validator "${metadata_root}" v2.0.0-rc.1+meta

# The validator must reject every kind of tracked or nonignored untracked
# drift before reading release metadata from the non-candidate worktree.
for dirty_kind in staged unstaged untracked; do
    dirty_root="${root_temp}/dirty-${dirty_kind}"
    make_fixture "${dirty_root}" "2.0.0"
    case "${dirty_kind}" in
        staged)
            printf '\n# staged mutation\n' >> "${dirty_root}/CHANGELOG.md"
            git -C "${dirty_root}" add CHANGELOG.md
            ;;
        unstaged)
            printf '\n# unstaged mutation\n' >> "${dirty_root}/CHANGELOG.md"
            ;;
        untracked)
            printf 'untracked candidate input\n' > "${dirty_root}/release-notes.tmp"
            ;;
    esac
    expect_failure "release validation requires a clean candidate worktree" \
        run_validator "${dirty_root}" v2.0.0
done

unborn_root="${root_temp}/unborn"
git init --quiet "${unborn_root}"
expect_failure "requires HEAD to resolve to a committed candidate" \
    run_validator "${unborn_root}" v2.0.0

outside_root="${root_temp}/outside"
mkdir "${outside_root}"
expect_failure "release validation requires a Git worktree" \
    run_validator "${outside_root}" v2.0.0

for malformed_tag in V2.0.0 'v2.0.0 ' ' v2.0.0' refs/tags/v2.0.0 v02.0.0 v2.0.0- v2.0; do
    expect_failure "release tag must have the form" \
        run_validator "${complete_root}" "${malformed_tag}"
done

# Publication consumes the same version output after successful validation.
GITHUB_OUTPUT="${root_temp}/version-output" run_validator "${complete_root}" v2.0.0
grep -Fx 'version=2.0.0' "${root_temp}/version-output"

echo "release validator regression checks passed"
