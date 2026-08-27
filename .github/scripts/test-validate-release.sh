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
# Markdown emphasis around the header plus a lower-case status: neither may
# hide the status column, and only `Pass` counts.
emphasised_checklist=$'# Hardware evidence\n\n| ID | **Status** | Evidence |\n| --- | --- | --- |\n| PT-01 | reviewed | bench-001 |\n\nFinal sign-off: Release owner 2026-08-26\nEvidence index: docs/evidence/2.0.0.md\n'
# No `Status` column anywhere: an emptied or restructured checklist proves
# nothing and must not pass by omission.
headerless_checklist=$'# Hardware evidence\n\nNo table here.\n\nFinal sign-off: Release owner 2026-08-26\nEvidence index: docs/evidence/2.0.0.md\n'
# Placeholder sign-off records are not sign-off.
placeholder_signoff_checklist=$'# Hardware evidence\n\n| ID | Status | Evidence |\n| --- | --- | --- |\n| PT-01 | Pass | bench-001 |\n\nFinal sign-off: pending\nEvidence index: TBD\n'

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

# Issue #632: build metadata has the same semver precedence as the base
# version, so `v2.0.0+meta` used to be a stable 2.0 publication that skipped
# the hardware gate entirely. Such tags are refused outright.
metadata_root="${root_temp}/metadata"
make_fixture "${metadata_root}" "2.0.0+meta" "${pending_checklist}"
expect_failure "build metadata" run_validator "${metadata_root}" v2.0.0+meta
expect_failure "build metadata" run_validator "${metadata_root}" v2.0.0-rc.1+meta

# Issue #632: the gate keyed off the literal major version 2, so every later
# major skipped it. It applies to any stable release from 2.0.0 onward.
major_three_root="${root_temp}/major-three"
make_fixture "${major_three_root}" "3.0.0" "${pending_checklist}"
expect_failure "incomplete rows" run_validator "${major_three_root}" v3.0.0

major_twelve_root="${root_temp}/major-twelve"
make_fixture "${major_twelve_root}" "12.0.0" "${pending_checklist}"
expect_failure "incomplete rows" run_validator "${major_twelve_root}" v12.0.0

# ... and it stays satisfiable there, rather than rejecting later majors
# unconditionally.
major_three_complete_root="${root_temp}/major-three-complete"
make_fixture "${major_three_complete_root}" "3.0.0" "${complete_checklist}"
run_validator "${major_three_complete_root}" v3.0.0

# Pre-releases of any major keep the candidate exemption.
major_three_rc_root="${root_temp}/major-three-rc"
make_fixture "${major_three_rc_root}" "3.0.0-rc.1" "${pending_checklist}"
run_validator "${major_three_rc_root}" v3.0.0-rc.1

emphasised_root="${root_temp}/emphasised"
make_fixture "${emphasised_root}" "2.0.0" "${emphasised_checklist}"
expect_failure "non-Pass status cells" run_validator "${emphasised_root}" v2.0.0

headerless_root="${root_temp}/headerless"
make_fixture "${headerless_root}" "2.0.0" "${headerless_checklist}"
expect_failure "no table row under a 'Status' column" \
    run_validator "${headerless_root}" v2.0.0

placeholder_signoff_root="${root_temp}/placeholder-signoff"
make_fixture "${placeholder_signoff_root}" "2.0.0" "${placeholder_signoff_checklist}"
expect_failure "Final sign-off" run_validator "${placeholder_signoff_root}" v2.0.0

# Tag shapes that must never reach the version comparison at all. These fail in
# the shell prologue, so the fixture contents are irrelevant.
for malformed_tag in V2.0.0 'v2.0.0 ' ' v2.0.0' refs/tags/v2.0.0 v02.0.0 v2.0.0- v2.0; do
    expect_failure "release tag must have the form" \
        run_validator "${complete_root}" "${malformed_tag}"
done

echo "release validator regression checks passed"
