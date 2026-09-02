#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
validator="${script_dir}/validate-change-record.py"
fixture_root="$(mktemp -d)"

cleanup() {
    rm -rf -- "${fixture_root:?}"
}
trap cleanup EXIT

make_fixture() {
    local name="$1"
    fixture="${fixture_root}/${name}"
    mkdir -p "${fixture}/api/2.0.0-rc.1" "${fixture}/src"
    git -C "${fixture}" init -q
    git -C "${fixture}" config user.name "Change Record Fixture"
    git -C "${fixture}" config user.email "change-record@example.invalid"

    python3 - "${fixture}" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
(root / "CHANGELOG.md").write_text(
    "# Changelog\n\n"
    "## [Unreleased]\n\n"
    "- Initial development record.\n\n"
    "## [1.1.0] - 2026-08-01\n\n"
    "- Published behavior.\n"
)
(root / "api/2.0.0-rc.1/all-features.txt").write_text("pub struct Example\n")
(root / "src/lib.rs").write_text("pub struct Example;\n")
PY
    git -C "${fixture}" add .
    git -C "${fixture}" commit -q -m "test: establish fixture" \
        -m "Create the release-record validator baseline."
    base="$(git -C "${fixture}" rev-parse HEAD)"
}

run_validator() {
    (
        cd "${fixture}"
        python3 "${validator}" "${base}" HEAD
    )
}

expect_failure() {
    local expected="$1"
    local log_file="${fixture_root}/failure.log"
    local status=0
    run_validator >"${log_file}" 2>&1 || status=$?
    if [[ "${status}" -eq 0 ]]; then
        cat "${log_file}" >&2
        echo "expected change-record validation to fail: ${expected}" >&2
        return 1
    fi
    if ! grep -Fq -- "${expected}" "${log_file}"; then
        cat "${log_file}" >&2
        echo "validator failure did not contain: ${expected}" >&2
        return 1
    fi
}

# A source/API change with a body, a referenced breaking record, and no edit to
# released history is the complete positive case.
make_fixture good
python3 - "${fixture}" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
changelog = (root / "CHANGELOG.md").read_text()
(root / "CHANGELOG.md").write_text(
    changelog.replace(
        "- Initial development record.\n",
        "- Initial development record.\n"
        "- **BREAKING** (#123): Change the public example.\n",
    )
)
(root / "api/2.0.0-rc.1/all-features.txt").write_text(
    "pub struct Example\npub fn example()\n"
)
(root / "src/lib.rs").write_text("pub struct Example;\npub fn example() {}\n")
PY
git -C "${fixture}" add .
git -C "${fixture}" commit -q -m "feat: change the example API" \
    -m "Record and exercise the public API change."
run_validator

# 1. Rewriting an already released entry is rejected.
make_fixture released-history
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(path.read_text().replace("Published behavior.", "Rewritten behavior."))
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: rewrite released history"
expect_failure "content outside [Unreleased] is immutable"

# 2. Regenerating an API snapshot without a changelog hunk is rejected.
make_fixture api-without-record
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "api/2.0.0-rc.1/all-features.txt"
path.write_text(path.read_text() + "pub fn unrecorded()\n")
PY
git -C "${fixture}" add api/2.0.0-rc.1/all-features.txt
git -C "${fixture}" commit -q -m "test: mutate API snapshot"
expect_failure "public API snapshot changes require a CHANGELOG.md hunk"

# 3. An exact BREAKING label without the required issue-reference shape fails.
make_fixture breaking-without-issue
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(
    path.read_text().replace(
        "- Initial development record.\n",
        "- Initial development record.\n- **BREAKING**: Untracked break.\n",
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: omit breaking issue"
expect_failure '`**BREAKING**` bullet is missing an issue reference'

# 4. A source commit with a subject but no explanatory body fails.
make_fixture source-without-body
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "src/lib.rs"
path.write_text(path.read_text() + "pub fn unrecorded() {}\n")
PY
git -C "${fixture}" add src/lib.rs
git -C "${fixture}" commit -q -m "feat: omit commit body"
expect_failure "touches src/ but has an empty commit body"

echo "change-record validator regression checks passed"
