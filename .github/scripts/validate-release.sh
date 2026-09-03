#!/usr/bin/env bash
set -euo pipefail

release_tag="${1:-${GITHUB_REF_NAME:-}}"

# Build metadata is refused outright rather than stripped and re-gated. This
# keeps one canonical tag per release.
if [[ "${release_tag}" == *+* ]]; then
    echo "release tag must not carry semver build metadata (received: ${release_tag})" >&2
    exit 1
fi

# No surrounding whitespace, uppercase V, refs/tags prefix, or leading zeroes.
if [[ ! "${release_tag}" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]]; then
    echo "release tag must have the form vX.Y.Z or vX.Y.Z-prerelease (received: ${release_tag:-<empty>})" >&2
    exit 1
fi

release_version="${release_tag#v}"

python3 - "${release_version}" <<'PY'
import json
import os
import pathlib
import re
import subprocess
import sys
import tomllib
from datetime import date

expected = sys.argv[1]
root = pathlib.Path.cwd()

version_match = re.fullmatch(
    r"(?P<major>0|[1-9][0-9]*)"
    r"\.(?P<minor>0|[1-9][0-9]*)"
    r"\.(?P<patch>0|[1-9][0-9]*)"
    r"(?:-(?P<prerelease>[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?P<build>\+.*)?",
    expected,
)
if version_match is None:
    raise SystemExit(f"tag version {expected} is not a plain semantic version")
if version_match.group("build") is not None:
    raise SystemExit(f"tag version {expected} carries semver build metadata")

metadata = subprocess.run(
    ["cargo", "metadata", "--format-version", "1", "--no-deps"],
    check=True,
    capture_output=True,
    text=True,
).stdout

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

# A prerelease is a software release candidate. Physical-camera evidence is a
# stable-release gate only, beginning with major version 2.
requires_hardware_evidence = (
    int(version_match.group("major")) >= 2
    and version_match.group("prerelease") is None
)

if requires_hardware_evidence:
    checklist = root / (
        os.environ.get("HARDWARE_CHECKLIST") or "docs/hardware_release_checklist.md"
    )
    if not checklist.is_file():
        raise SystemExit(f"missing hardware release checklist: {checklist}")

    text = checklist.read_text()
    expected_headers = [
        "ID",
        "Required scenario",
        "Status",
        "Firmware / bench",
        "Transcript / evidence",
    ]
    required_ids = [f"HW-{number:02d}" for number in range(1, 6)]

    def table_cells(line):
        stripped = line.strip()
        if not (stripped.startswith("|") and stripped.endswith("|")):
            return None
        return [cell.strip() for cell in stripped[1:-1].split("|")]

    def is_delimiter(cells):
        return cells is not None and len(cells) == len(expected_headers) and all(
            re.fullmatch(r":?-+:?", cell) for cell in cells
        )

    lines = text.splitlines()
    header_positions = []
    for index in range(len(lines) - 1):
        cells = table_cells(lines[index])
        normalized = [cell.casefold() for cell in cells] if cells else None
        if normalized == [header.casefold() for header in expected_headers] and is_delimiter(
            table_cells(lines[index + 1])
        ):
            header_positions.append(index)

    if len(header_positions) != 1:
        raise SystemExit(
            "stable 2.0+ release publication requires one hardware matrix table "
            f"with columns: {' | '.join(expected_headers)}"
        )

    header_index = header_positions[0]
    rows = []
    row_index = header_index + 2
    while row_index < len(lines):
        cells = table_cells(lines[row_index])
        if cells is None:
            break
        if len(cells) != len(expected_headers):
            raise SystemExit(
                "stable 2.0+ release publication rejects inconsistent hardware "
                f"matrix row width at line {row_index + 1}"
            )
        rows.append((row_index + 1, cells))
        row_index += 1

    required_set = set(required_ids)
    row_map = {}
    for line_number, cells in rows:
        identifier = cells[0]
        if identifier not in required_set:
            raise SystemExit(
                "stable 2.0+ release publication permits only hardware matrix "
                f"IDs HW-01..HW-05; unexpected ID at line {line_number}: "
                f"{identifier or '<empty>'}"
            )
        row_map.setdefault(identifier, []).append((line_number, cells))

    missing = [identifier for identifier in required_ids if identifier not in row_map]
    if missing:
        raise SystemExit(
            "stable 2.0+ release publication requires every hardware matrix ID "
            f"exactly once; missing required IDs: {', '.join(missing)}"
        )
    duplicate = [
        f"{identifier} ({len(row_map[identifier])} rows)"
        for identifier in required_ids
        if len(row_map[identifier]) != 1
    ]
    if duplicate:
        raise SystemExit(
            "stable 2.0+ release publication requires every hardware matrix ID "
            f"exactly once; duplicate required IDs: {'; '.join(duplicate)}"
        )

    placeholder = re.compile(
        r"^(?:pending|blocked|fail(?:ed)?|tbd|todo|n/?a|none|unknown|xxx|"
        r"unverified|not[ \t]+(?:run|tested|executed|verified)|"
        r"no[ \t]+(?:capture|evidence|test(?:ing)?))\b",
        re.IGNORECASE,
    )

    def recorded(value):
        value = value.strip()
        return bool(value) and not placeholder.match(value) and not re.fullmatch(
            r"(?:\.\.\.|…|-+|–+|—+)", value
        )

    for identifier in required_ids:
        line_number, cells = row_map[identifier][0]
        if cells[2] != "Pass":
            raise SystemExit(
                "stable 2.0+ release publication requires every hardware matrix "
                f"status to be exactly Pass; {identifier} at line {line_number} "
                f"is {cells[2] or '<empty>'}"
            )
        for label, value in (
            ("Required scenario", cells[1]),
            ("Firmware / bench", cells[3]),
            ("Transcript / evidence", cells[4]),
        ):
            if not recorded(value):
                raise SystemExit(
                    "stable 2.0+ release publication requires every Pass row to "
                    f"record non-placeholder {label}; {identifier} at line "
                    f"{line_number} is {value or '<empty>'}"
                )

    def record_values(label):
        pattern = re.compile(rf"^{re.escape(label)}:[ \t]*(.*)$", re.MULTILINE)
        return [match.group(1).strip() for match in pattern.finditer(text)]

    tested_commits = record_values("Hardware-tested commit")
    if len(tested_commits) != 1 or re.fullmatch(
        r"[0-9a-f]{40}", tested_commits[0] if tested_commits else ""
    ) is None:
        raise SystemExit(
            "stable 2.0+ release publication requires Hardware-tested commit: "
            "to contain one full lowercase 40-hex SHA"
        )

    operator_dates = record_values("Hardware operator/date")
    operator_date_pattern = re.compile(
        r"^(?P<operator>.+\S)[ \t]+(?:—|-)[ \t]+"
        r"(?P<date>\d{4}-\d{2}-\d{2})(?:[ \t]+UTC)?$"
    )
    if len(operator_dates) != 1:
        raise SystemExit(
            "stable 2.0+ release publication requires one Hardware operator/date "
            "operator/date record"
        )
    operator_date_match = operator_date_pattern.fullmatch(operator_dates[0])
    if operator_date_match is None:
        raise SystemExit(
            "Hardware operator/date must be OPERATOR — YYYY-MM-DD UTC"
        )
    if not recorded(operator_date_match.group("operator")):
        raise SystemExit(
            "Hardware operator/date must name a non-placeholder operator"
        )
    try:
        date.fromisoformat(operator_date_match.group("date"))
    except ValueError:
        raise SystemExit(
            "Hardware operator/date must contain a valid date"
        )

    for label in ("Final sign-off", "Evidence index"):
        values = record_values(label)
        if len(values) != 1 or not recorded(values[0]):
            raise SystemExit(
                "stable 2.0+ release publication requires non-placeholder "
                f"'{label}:' record in {checklist}"
            )
PY

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    echo "version=${release_version}" >> "${GITHUB_OUTPUT}"
fi

echo "release metadata is consistent for ${release_tag}"
