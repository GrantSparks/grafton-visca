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
    (cd "${root}" && \
        HARDWARE_CHECKLIST=docs/hardware_release_checklist.md \
        bash "${validator}" "${version}")
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

make_checklist() {
    local variant="$1"
    python3 - "${variant}" <<'PY'
import sys

variant = sys.argv[1]
required_ids = [f"HW-{number:02d}" for number in range(1, 6)]

lines = [
    "# Hardware evidence",
    "",
    "| ID | Required scenario | Status | Firmware / bench | Transcript / evidence |",
    "| --- | --- | --- | --- | --- |",
]
for identifier in required_ids:
    status = "Pass"
    scenario = "Representative hardware release scenario"
    firmware = "Camera firmware 1.2.3 / bench rack A"
    evidence = f"docs/evidence/2.0.0/{identifier.lower()}.md"
    if variant == "pending":
        status = "Pending (Not run)"
        firmware = evidence = "Pending"
    if variant == "lowercase-status" and identifier == "HW-01":
        status = "pass"
    if variant == "placeholder-scenario" and identifier == "HW-01":
        scenario = "N/A"
    if variant == "placeholder-evidence" and identifier == "HW-01":
        evidence = "Blocked"
    if variant == "unverified-firmware" and identifier == "HW-01":
        firmware = "Unverified"
    if variant == "not-run-firmware" and identifier == "HW-01":
        firmware = "Not run"
    if variant == "not-tested-evidence" and identifier == "HW-01":
        evidence = "Not tested"
    if variant == "no-capture-evidence" and identifier == "HW-01":
        evidence = "No capture"
    if variant == "missing" and identifier == "HW-03":
        continue
    lines.append(f"| {identifier} | {scenario} | {status} | {firmware} | {evidence} |")

if variant == "duplicate":
    lines.append("| HW-01 | Duplicate hardware scenario | Pass | Camera firmware 1.2.3 / bench rack A | docs/evidence/2.0.0/hw-01-extra.md |")
elif variant == "unexpected":
    lines.append("| PT-01 | Unexpected hardware scenario | Pass | Camera firmware 1.2.3 / bench rack A | docs/evidence/2.0.0/pt-01.md |")
elif variant == "bad-header":
    lines[2] = "| ID | Owner | Status | Evidence |"
    lines[3] = "| --- | --- | --- | --- |"
elif variant == "bad-width":
    lines.append("| HW-06 | Hardware QA | Pass | Camera firmware 1.2.3 / bench rack A |")

lines.extend(
    [
        "",
        "Hardware-tested commit: 0123456789abcdef0123456789abcdef01234567",
        "Hardware operator/date: Release owner — 2026-08-26 UTC",
        "",
        "Final sign-off: Release owner 2026-08-26",
        "Evidence index: docs/evidence/2.0.0.md",
    ]
)
if variant == "missing-commit":
    lines = [line for line in lines if not line.startswith("Hardware-tested commit:")]
elif variant == "short-commit":
    lines = [
        "Hardware-tested commit: abc123" if line.startswith("Hardware-tested commit:") else line
        for line in lines
    ]
elif variant == "uppercase-commit":
    lines = [
        "Hardware-tested commit: 0123456789ABCDEF0123456789ABCDEF01234567"
        if line.startswith("Hardware-tested commit:") else line
        for line in lines
    ]
elif variant == "missing-operator":
    lines = [line for line in lines if not line.startswith("Hardware operator/date:")]
elif variant == "placeholder-operator":
    lines = [
        "Hardware operator/date: Pending — 2026-08-26 UTC"
        if line.startswith("Hardware operator/date:") else line
        for line in lines
    ]
elif variant == "unknown-operator":
    lines = [
        "Hardware operator/date: Unknown — 2026-08-26 UTC"
        if line.startswith("Hardware operator/date:") else line
        for line in lines
    ]
elif variant == "invalid-operator-date":
    lines = [
        "Hardware operator/date: Release owner — 2026-13-40 UTC"
        if line.startswith("Hardware operator/date:") else line
        for line in lines
    ]
elif variant == "placeholder-signoff":
    lines = [
        "Final sign-off: pending" if line.startswith("Final sign-off:") else line
        for line in lines
    ]
elif variant == "placeholder-evidence-index":
    lines = [
        "Evidence index: TBD" if line.startswith("Evidence index:") else line
        for line in lines
    ]
elif variant == "no-evidence-index":
    lines = [
        "Evidence index: No evidence" if line.startswith("Evidence index:") else line
        for line in lines
    ]
elif variant == "malformed":
    lines = [
        "This is not a hardware matrix.",
        "Hardware-tested commit: not-a-sha",
        "Hardware operator/date: pending",
        "Final sign-off: pending",
        "Evidence index: TBD",
    ]

print("\n".join(lines))
PY
}

pending_checklist="$(make_checklist pending)"
complete_checklist="$(make_checklist complete)"

# Prereleases only need the software metadata contract. Both pending and
# malformed hardware content are intentionally ignored.
for version in 2.0.0-rc.1 3.0.0-rc.1; do
    rc_root="${root_temp}/rc-${version}"
    make_fixture "${rc_root}" "${version}" "${pending_checklist}"
    run_validator "${rc_root}" "v${version}"

    malformed_rc_root="${root_temp}/malformed-rc-${version}"
    make_fixture "${malformed_rc_root}" "${version}" "$(make_checklist malformed)"
    run_validator "${malformed_rc_root}" "v${version}"
done

# Hardware is deliberately outside the RC publication contract: even a
# checkout with no checklist file at all must pass the software-only gate.
rc_missing_checklist_root="${root_temp}/rc-missing-checklist"
make_fixture "${rc_missing_checklist_root}" "2.0.0-rc.1" "${pending_checklist}"
rm -f "${rc_missing_checklist_root}/docs/hardware_release_checklist.md"
commit_fixture_changes "${rc_missing_checklist_root}"
run_validator "${rc_missing_checklist_root}" v2.0.0-rc.1

# Stable 2.0+ releases require the five-row matrix and its four records.
pending_root="${root_temp}/pending"
make_fixture "${pending_root}" "2.0.0" "${pending_checklist}"
expect_failure "status to be exactly Pass" run_validator "${pending_root}" v2.0.0

complete_root="${root_temp}/complete"
make_fixture "${complete_root}" "2.0.0" "${complete_checklist}"
run_validator "${complete_root}" v2.0.0

for variant in missing duplicate unexpected bad-header bad-width lowercase-status \
    placeholder-scenario placeholder-evidence unverified-firmware not-run-firmware \
    not-tested-evidence no-capture-evidence no-evidence-index missing-commit \
    short-commit uppercase-commit missing-operator placeholder-operator unknown-operator \
    invalid-operator-date placeholder-signoff placeholder-evidence-index; do
    variant_root="${root_temp}/${variant}"
    make_fixture "${variant_root}" "2.0.0" "$(make_checklist "${variant}")"
    case "${variant}" in
        missing)
            expected="missing required IDs: HW-03"
            ;;
        duplicate)
            expected="duplicate required IDs: HW-01"
            ;;
        unexpected)
            expected="unexpected ID"
            ;;
        bad-header)
            expected="one hardware matrix table"
            ;;
        bad-width)
            expected="row width"
            ;;
        lowercase-status)
            expected="status to be exactly Pass"
            ;;
        placeholder-scenario)
            expected="non-placeholder Required scenario"
            ;;
        placeholder-evidence)
            expected="non-placeholder Transcript / evidence"
            ;;
        unverified-firmware|not-run-firmware)
            expected="non-placeholder Firmware / bench"
            ;;
        not-tested-evidence|no-capture-evidence)
            expected="non-placeholder Transcript / evidence"
            ;;
        no-evidence-index)
            expected="'Evidence index:'"
            ;;
        missing-commit|short-commit|uppercase-commit)
            expected="Hardware-tested commit"
            ;;
        missing-operator)
            expected="one Hardware operator/date"
            ;;
        placeholder-operator|unknown-operator)
            expected="non-placeholder operator"
            ;;
        invalid-operator-date)
            expected="must contain a valid date"
            ;;
        placeholder-signoff)
            expected="'Final sign-off:'"
            ;;
        placeholder-evidence-index)
            expected="'Evidence index:'"
            ;;
    esac
    expect_failure "${expected}" run_validator "${variant_root}" v2.0.0
done

# The gate applies to every later stable major, while their prereleases remain
# software-only candidates.
for version in 3.0.0 12.0.0; do
    later_root="${root_temp}/later-${version}"
    make_fixture "${later_root}" "${version}" "${pending_checklist}"
    expect_failure "status to be exactly Pass" run_validator "${later_root}" "v${version}"
done

later_complete_root="${root_temp}/later-complete"
make_fixture "${later_complete_root}" "3.0.0" "${complete_checklist}"
run_validator "${later_complete_root}" v3.0.0

# Build metadata is never an alternate release identity, including on an RC.
metadata_root="${root_temp}/metadata"
make_fixture "${metadata_root}" "2.0.0+meta" "${pending_checklist}"
expect_failure "build metadata" run_validator "${metadata_root}" v2.0.0+meta
expect_failure "build metadata" run_validator "${metadata_root}" v2.0.0-rc.1+meta

# A pre-2.0 stable release keeps the software-only contract.
one_root="${root_temp}/one"
make_fixture "${one_root}" "1.99.0" "$(make_checklist malformed)"
run_validator "${one_root}" v1.99.0

missing_checklist_root="${root_temp}/missing-checklist"
make_fixture "${missing_checklist_root}" "2.0.0" "${complete_checklist}"
rm -f "${missing_checklist_root}/docs/hardware_release_checklist.md"
commit_fixture_changes "${missing_checklist_root}"
expect_failure "missing hardware release checklist" \
    run_validator "${missing_checklist_root}" v2.0.0

# The validator must fail before reading Cargo metadata or release files from a
# non-candidate worktree. Cover all status classes intentionally excluded by
# neither --porcelain nor the release process.
for dirty_kind in staged unstaged untracked; do
    dirty_root="${root_temp}/dirty-${dirty_kind}"
    make_fixture "${dirty_root}" "2.0.0-rc.1" "${pending_checklist}"
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
        run_validator "${dirty_root}" v2.0.0-rc.1
done

unborn_root="${root_temp}/unborn"
git init --quiet "${unborn_root}"
expect_failure "requires HEAD to resolve to a committed candidate" \
    run_validator "${unborn_root}" v2.0.0-rc.1

for malformed_tag in V2.0.0 'v2.0.0 ' ' v2.0.0' refs/tags/v2.0.0 v02.0.0 v2.0.0- v2.0; do
    expect_failure "release tag must have the form" \
        run_validator "${complete_root}" "${malformed_tag}"
done

echo "release validator regression checks passed"
