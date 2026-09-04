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
    "- Retained release record.\n\n"
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

write_policy_marker() {
    local marker_path="$1"
    local boundary="$2"
    python3 - "${fixture}" "${marker_path}" "${boundary}" <<'PY'
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
path = root / sys.argv[2]
path.parent.mkdir(parents=True, exist_ok=True)
path.write_text(f"{sys.argv[3]}\n")
PY
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

# 1. A release cut may retain every Unreleased line in order, add explanatory
# release text, and move it under one strict dated release heading.
make_fixture release-cut
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
changelog = path.read_text().replace(
    "## [Unreleased]\n\n",
    "## [Unreleased]\n\n## [2.0.0-rc.1] - 2026-09-04\n\n",
)
path.write_text(
    changelog.replace(
        "\n## [1.1.0] - 2026-08-01\n",
        "\nNo physical hardware validation has been performed, and no hardware "
        "support has been verified for this release candidate.\n\n"
        "## [1.1.0] - 2026-08-01\n",
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: cut release record" \
    -m "Move the retained Unreleased notes into the release candidate."
run_validator

# 2. Unreleased may evolve after the history-policy boundary and before the
# cut. The release transition must be checked against its direct parent, not
# against a stale policy-boundary snapshot.
make_fixture release-cut-after-unreleased-edit
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(path.read_text().replace("Initial development", "Revised development"))
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: revise development record" \
    -m "Keep the mutable Unreleased entry current before cutting a release."
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(
    path.read_text().replace(
        "## [Unreleased]\n\n",
        "## [Unreleased]\n\n## [2.0.0-rc.1] - 2026-09-04\n\n",
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: cut revised release record" \
    -m "Move the direct parent's retained Unreleased notes into the candidate."
run_validator

# 3. Rewriting an already released entry is rejected.
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

# 4. The release-cut exception cannot hide an edit to the pre-existing released
# suffix.
make_fixture release-cut-released-history
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
changelog = path.read_text().replace(
    "## [Unreleased]\n\n",
    "## [Unreleased]\n\n## [2.0.0-rc.1] - 2026-09-04\n\n",
)
path.write_text(changelog.replace("Published behavior.", "Rewritten behavior."))
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: cut and rewrite release history"
expect_failure "content outside [Unreleased] is immutable"

# 5. A release cut cannot drop a prior Unreleased line.
make_fixture release-cut-dropped-note
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(
    path.read_text().replace(
        "## [Unreleased]\n\n- Initial development record.\n\n"
        "- Retained release record.\n\n",
        "## [Unreleased]\n\n## [2.0.0-rc.1] - 2026-09-04\n\n"
        "- Retained release record.\n\n",
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: drop prior release note"
expect_failure "content outside [Unreleased] is immutable"

# 6. A release cut cannot reorder prior Unreleased lines.
make_fixture release-cut-reordered-notes
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(
    path.read_text().replace(
        "## [Unreleased]\n\n- Initial development record.\n\n"
        "- Retained release record.\n\n",
        "## [Unreleased]\n\n## [2.0.0-rc.1] - 2026-09-04\n\n"
        "- Retained release record.\n\n- Initial development record.\n\n",
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: reorder prior release notes"
expect_failure "content outside [Unreleased] is immutable"

# 7. A later commit cannot rewrite the released suffix after a valid cut.
make_fixture release-cut-postcut-released-history
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(
    path.read_text().replace(
        "## [Unreleased]\n\n",
        "## [Unreleased]\n\n## [2.0.0-rc.1] - 2026-09-04\n\n",
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: cut release record" \
    -m "Move the retained Unreleased notes into the release candidate."
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(path.read_text().replace("Published behavior.", "Rewritten behavior."))
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: rewrite cut release history" \
    -m "Attempt to alter the immutable suffix after the release cut."
expect_failure "content outside [Unreleased] is immutable"

# 8. The released suffix comparison is byte-for-byte: a line-ending rewrite
# after a valid cut is also rejected.
make_fixture release-cut-line-ending-history
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_bytes(path.read_bytes().replace(b"\n", b"\r\n"))
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "test: establish CRLF release record" \
    -m "Use a byte-sensitive released suffix baseline."
base="$(git -C "${fixture}" rev-parse HEAD)"
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_bytes(
    path.read_bytes().replace(
        b"## [Unreleased]\r\n\r\n",
        b"## [Unreleased]\r\n\r\n## [2.0.0-rc.1] - 2026-09-04\r\n\r\n",
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: cut CRLF release record" \
    -m "Move the retained CRLF Unreleased notes into the release candidate."
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_bytes(
    path.read_bytes().replace(
        b"- Published behavior.\r\n", b"- Published behavior.\n"
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: normalize released line ending" \
    -m "Attempt to rewrite an immutable released byte sequence."
expect_failure "content outside [Unreleased] is immutable"

# 9. Newly released notes retain the BREAKING issue-reference requirement.
make_fixture release-cut-breaking-without-issue
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "CHANGELOG.md"
path.write_text(
    path.read_text().replace(
        "## [Unreleased]\n\n",
        "## [Unreleased]\n\n## [2.0.0-rc.1] - 2026-09-04\n\n"
        "- **BREAKING**: Untracked release break.\n\n",
    )
)
PY
git -C "${fixture}" add CHANGELOG.md
git -C "${fixture}" commit -q -m "docs: cut untracked breaking release"
expect_failure '`**BREAKING**` bullet is missing an issue reference'

# 10. Regenerating an API snapshot without a changelog hunk is rejected.
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

# 11. An exact BREAKING label without the required issue-reference shape fails.
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

# 12. A source commit with a subject but no explanatory body fails.
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

# 13. A base without a marker may establish the reviewed body-policy boundary
# once, including over pre-policy history that would otherwise lack bodies.
make_fixture boundary-bootstrap
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "src/lib.rs"
path.write_text(path.read_text() + "pub fn pre_policy() {}\n")
PY
git -C "${fixture}" add src/lib.rs
git -C "${fixture}" commit -q -m "feat: pre-policy source history"
bootstrap_boundary="$(git -C "${fixture}" rev-parse HEAD)"
write_policy_marker ".github/change-record-body-policy-boundary" "${bootstrap_boundary}"
git -C "${fixture}" add .github/change-record-body-policy-boundary
git -C "${fixture}" commit -q -m "ci: establish body policy boundary" \
    -m "Bootstrap reviewed commit-body enforcement."
run_validator

# 14. Once a base contains its marker, an ordinary documented source change may
# proceed only while HEAD retains that exact reviewed marker.
make_fixture retained-boundary
retained_boundary="${base}"
write_policy_marker ".github/change-record-body-policy-boundary" "${retained_boundary}"
git -C "${fixture}" add .github/change-record-body-policy-boundary
git -C "${fixture}" commit -q -m "ci: establish retained boundary" \
    -m "Record the immutable reviewed body-policy boundary."
base="$(git -C "${fixture}" rev-parse HEAD)"
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "src/lib.rs"
path.write_text(path.read_text() + "pub fn documented() {}\n")
PY
git -C "${fixture}" add src/lib.rs
git -C "${fixture}" commit -q -m "feat: documented source change" \
    -m "Keep the source commit within the established policy."
run_validator

# 15. A PR cannot hide a bodyless source commit by advancing a marker that was
# already present in its merge base. The original boundary must remain intact.
make_fixture boundary-advance-attack
reviewed_boundary="${base}"
write_policy_marker ".github/change-record-body-policy-boundary" "${reviewed_boundary}"
git -C "${fixture}" add .github/change-record-body-policy-boundary
git -C "${fixture}" commit -q -m "ci: establish immutable boundary" \
    -m "Record the reviewed body-policy boundary."
base="$(git -C "${fixture}" rev-parse HEAD)"
python3 - "${fixture}" <<'PY'
import pathlib
import sys

path = pathlib.Path(sys.argv[1]) / "src/lib.rs"
path.write_text(path.read_text() + "pub fn hidden_by_marker() {}\n")
PY
git -C "${fixture}" add src/lib.rs
git -C "${fixture}" commit -q -m "feat: bodyless source commit"
bodyless_commit="$(git -C "${fixture}" rev-parse HEAD)"
write_policy_marker ".github/change-record-body-policy-boundary" "${bodyless_commit}"
git -C "${fixture}" add .github/change-record-body-policy-boundary
git -C "${fixture}" commit -q -m "ci: advance body policy boundary" \
    -m "Attempt to skip the preceding source commit."
expect_failure "must retain the merge-base marker"

echo "change-record validator regression checks passed"
