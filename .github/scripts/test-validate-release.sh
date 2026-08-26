#!/usr/bin/env bash
set -euo pipefail

# Regression checks for the release validator. The fixtures deliberately live
# in mktemp directories so these checks never modify the real hardware
# checklist or require a tracked Cargo.lock.
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
validator="${script_dir}/validate-release.sh"

make_fixture() {
    local destination="$1"
    local version="$2"
    local checklist="$3"

    mkdir -p "${destination}/grafton-visca-macros/src" \
        "${destination}/src" "${destination}/docs"
    python3 - "${destination}" "${version}" "${checklist}" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
version = sys.argv[2]
checklist = sys.argv[3]

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
    f"# Changelog\n\n## [{version}] - 2026-08-26\n"
)
(root / "docs/hardware_release_checklist.md").write_text(checklist)
PY
}

expect_failure() {
    local expected_text="$1"
    shift
    local log_file
    log_file="$(mktemp)"
    local status=0
    "$@" >"${log_file}" 2>&1 || status=$?
    if [[ "${status}" -eq 0 ]]; then
        cat "${log_file}"
        rm -f "${log_file}"
        echo "expected validator failure containing: ${expected_text}" >&2
        return 1
    fi
    if ! grep -Fq "${expected_text}" "${log_file}"; then
        cat "${log_file}" >&2
        rm -f "${log_file}"
        echo "validator failure did not contain: ${expected_text}" >&2
        return 1
    fi
    rm -f "${log_file}"
}

run_validator() {
    local root="$1"
    local version="$2"
    (cd "${root}" && \
        HARDWARE_CHECKLIST=docs/hardware_release_checklist.md \
        bash "${validator}" "${version}")
}

root_temp="$(mktemp -d)"
trap 'rm -rf "${root_temp}"' EXIT

pending_checklist=$'# Hardware evidence\n\n| ID | Status | Evidence |\n| --- | --- | --- |\n| PT-01 | Pending (Not run) | Pending |\n'
blocked_checklist=$'# Hardware evidence\n\n| ID | Status | Evidence |\n| --- | --- | --- |\n| PT-01 | Blocked | bench issue |\n'
unsigned_checklist=$'# Hardware evidence\n\n| ID | Status | Evidence |\n| --- | --- | --- |\n| PT-01 | Pass | bench-001 |\n'
nonpassing_checklist=$'# Hardware evidence\n\n| ID | Status | Evidence |\n| --- | --- | --- |\n| PT-01 | Review | bench-001 |\n'
complete_checklist=$'# Hardware evidence\n\n| ID | Status | Evidence |\n| --- | --- | --- |\n| PT-01 | Pass | bench-001 |\n| FW-01 | Pass | bench-002 |\n\nFinal sign-off: Release owner 2026-08-26\nEvidence index: docs/evidence/2.0.0.md\n'

rc_root="${root_temp}/rc"
make_fixture "${rc_root}" "2.0.0-rc.1" "${pending_checklist}"
test ! -e "${rc_root}/Cargo.lock"
(cd "${rc_root}" && bash "${validator}" v2.0.0-rc.1)

pending_root="${root_temp}/pending"
make_fixture "${pending_root}" "2.0.0" "${pending_checklist}"
expect_failure "incomplete rows" run_validator "${pending_root}" v2.0.0

blocked_root="${root_temp}/blocked"
make_fixture "${blocked_root}" "2.0.0" "${blocked_checklist}"
expect_failure "incomplete rows" run_validator "${blocked_root}" v2.0.0

unsigned_root="${root_temp}/unsigned"
make_fixture "${unsigned_root}" "2.0.0" "${unsigned_checklist}"
expect_failure "Final sign-off" run_validator "${unsigned_root}" v2.0.0

nonpassing_root="${root_temp}/nonpassing"
make_fixture "${nonpassing_root}" "2.0.0" "${nonpassing_checklist}"
expect_failure "non-Pass status cells" run_validator "${nonpassing_root}" v2.0.0

complete_root="${root_temp}/complete"
make_fixture "${complete_root}" "2.0.0" "${complete_checklist}"
run_validator "${complete_root}" v2.0.0

echo "release validator regression checks passed"
