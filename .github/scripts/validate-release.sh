#!/usr/bin/env bash
set -euo pipefail

release_tag="${1:-${GITHUB_REF_NAME:-}}"
if [[ ! "${release_tag}" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]]; then
    echo "release tag must have the form vX.Y.Z (received: ${release_tag:-<empty>})" >&2
    exit 1
fi

release_version="${release_tag#v}"

python3 - "${release_version}" <<'PY'
import pathlib
import re
import os
import subprocess
import sys
import tomllib

expected = sys.argv[1]
root = pathlib.Path.cwd()

metadata = subprocess.run(
    # This is intentionally lockfile-independent.  grafton-visca is a library
    # workspace and does not track Cargo.lock; the publication workflow creates
    # an ignored lockfile later, immediately before locked packaging.
    ["cargo", "metadata", "--format-version", "1", "--no-deps"],
    check=True,
    capture_output=True,
    text=True,
).stdout

import json
packages = {package["name"]: package for package in json.loads(metadata)["packages"]}
for name in ("grafton-visca", "grafton-visca-macros"):
    actual = packages[name]["version"]
    if actual != expected:
        raise SystemExit(f"{name} version {actual} does not match tag version {expected}")

manifest = tomllib.loads((root / "Cargo.toml").read_text())
macro_dependency = manifest["dependencies"]["grafton-visca-macros"]["version"]
if macro_dependency != f"={expected}":
    raise SystemExit(
        "grafton-visca-macros dependency version "
        f"{macro_dependency} is not pinned to the tag version ={expected}"
    )

changelog = (root / "CHANGELOG.md").read_text()
heading = re.compile(
    rf"^## \[{re.escape(expected)}\] - \d{{4}}-\d{{2}}-\d{{2}}$", re.MULTILINE
)
if not heading.search(changelog):
    raise SystemExit(
        f"CHANGELOG.md must contain a dated '## [{expected}] - YYYY-MM-DD' heading"
    )

# Stable 2.x publication requires physical-camera evidence in addition to the
# software gates above. Candidate tags deliberately remain publishable while
# their hardware rows are still marked pending; a final tag is not.
if re.fullmatch(r"2\.\d+\.\d+", expected):
    checklist = root / os.environ.get(
        "HARDWARE_CHECKLIST", "docs/hardware_release_checklist.md"
    )
    if not checklist.is_file():
        raise SystemExit(f"missing hardware release checklist: {checklist}")

    incomplete = []
    nonpassing = []
    status_column = None
    checklist_text = checklist.read_text()
    for line_number, line in enumerate(checklist_text.splitlines(), 1):
        if "|" not in line:
            status_column = None
            continue
        cells = [cell.strip() for cell in line.split("|")]
        if "Status" in cells:
            status_column = cells.index("Status")
            continue
        for cell in cells:
            if (
                cell.startswith("Pending")
                or cell in {"Blocked", "Fail"}
            ):
                incomplete.append(f"line {line_number}: {cell}")
        if status_column is not None and status_column < len(cells):
            status = cells[status_column]
            if status != "---" and status != "Pass":
                nonpassing.append(f"line {line_number}: {status or '<empty>'}")
    if incomplete:
        sample = "; ".join(incomplete[:5])
        suffix = "" if len(incomplete) <= 5 else f"; ... ({len(incomplete)} total)"
        raise SystemExit(
            "stable 2.x publication requires every hardware/checklist row to "
            f"have evidence; incomplete rows: {sample}{suffix}"
        )
    if nonpassing:
        sample = "; ".join(nonpassing[:5])
        suffix = "" if len(nonpassing) <= 5 else f"; ... ({len(nonpassing)} total)"
        raise SystemExit(
            "stable 2.x publication requires every hardware/checklist status "
            f"to be Pass; non-Pass status cells: {sample}{suffix}"
        )

    signoff = re.search(
        r"^Final sign-off:\s*(?!Pending\b)(\S.+)$", checklist_text, re.MULTILINE
    )
    evidence = re.search(
        r"^Evidence index:\s*(?!Pending\b)(\S.+)$", checklist_text, re.MULTILINE
    )
    if signoff is None or evidence is None:
        raise SystemExit(
            "stable 2.x publication requires non-pending 'Final sign-off:' "
            "and 'Evidence index:' records in docs/hardware_release_checklist.md"
        )
PY

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    echo "version=${release_version}" >> "${GITHUB_OUTPUT}"
fi

echo "release metadata is consistent for ${release_tag}"
