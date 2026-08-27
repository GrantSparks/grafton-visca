#!/usr/bin/env bash
set -euo pipefail

release_tag="${1:-${GITHUB_REF_NAME:-}}"

# Build metadata is refused outright rather than stripped and re-gated.
# `2.0.0+meta` has exactly the same semver precedence as `2.0.0`, so accepting
# both would let one release be published under two different tag strings, and
# every downstream comparison -- manifest versions, the changelog heading, the
# crates.io version, and the stable-release evidence gate below -- would then
# have to agree on the same normalisation to stay honest. Refusing keeps one
# canonical tag per release and removes that whole class of bypass.
if [[ "${release_tag}" == *+* ]]; then
    echo "release tag must not carry semver build metadata (received: ${release_tag}); +metadata has the same release precedence as the base version, so tag the canonical vX.Y.Z instead" >&2
    exit 1
fi

# Anchored on purpose: no surrounding whitespace, no uppercase `V`, no
# `refs/tags/` prefix, and no leading zeroes in the numeric identifiers.
if [[ ! "${release_tag}" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]]; then
    echo "release tag must have the form vX.Y.Z or vX.Y.Z-prerelease (received: ${release_tag:-<empty>})" >&2
    exit 1
fi

release_version="${release_tag#v}"

python3 - "${release_version}" <<'PY'
import json
import pathlib
import re
import os
import subprocess
import sys
import tomllib

expected = sys.argv[1]
root = pathlib.Path.cwd()

# The version is re-parsed here instead of trusting the shell shape check
# above: this predicate decides whether the hardware-evidence gate runs at all,
# so it has to fail closed on anything it does not fully understand.
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
    raise SystemExit(
        f"tag version {expected} carries semver build metadata; build metadata "
        "does not change release precedence, so it is not an accepted release "
        "identity"
    )

# Every stable release from 2.0.0 onward carries the hardware claim, so the
# evidence gate keys off "major >= 2 and not a pre-release" rather than the
# literal major version 2. Candidate tags (-rc.N and friends) stay exempt:
# they are publishable while their hardware rows are still marked pending.
requires_hardware_evidence = (
    int(version_match.group("major")) >= 2
    and version_match.group("prerelease") is None
)

metadata = subprocess.run(
    # This is intentionally lockfile-independent.  grafton-visca is a library
    # workspace and does not track Cargo.lock; the publication workflow creates
    # an ignored lockfile later, immediately before locked packaging.
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

# Stable publication from 2.0.0 onward requires physical-camera evidence in
# addition to the software gates above.
if requires_hardware_evidence:
    checklist = root / (
        os.environ.get("HARDWARE_CHECKLIST") or "docs/hardware_release_checklist.md"
    )
    if not checklist.is_file():
        raise SystemExit(f"missing hardware release checklist: {checklist}")

    # Markdown emphasis and letter case must not be able to hide a `Status`
    # header or a placeholder value, so every cell is normalised first.
    def normalise(cell):
        return re.sub(r"[*_`]", "", cell).strip()

    separator = re.compile(r"^:?-{2,}:?$")
    unevidenced = re.compile(
        r"^(pending|blocked|fail(ed)?|tbd|todo)\b", re.IGNORECASE
    )

    incomplete = []
    nonpassing = []
    status_column = None
    status_rows = 0
    checklist_text = checklist.read_text()
    for line_number, line in enumerate(checklist_text.splitlines(), 1):
        if "|" not in line:
            status_column = None
            continue
        cells = [normalise(cell) for cell in line.split("|")]
        folded = [cell.casefold() for cell in cells]
        if "status" in folded:
            status_column = folded.index("status")
            continue
        for cell in cells:
            if unevidenced.match(cell):
                incomplete.append(f"line {line_number}: {cell}")
        if status_column is not None and status_column < len(cells):
            status = cells[status_column]
            if separator.match(status):
                continue
            status_rows += 1
            if status.casefold() != "pass":
                nonpassing.append(f"line {line_number}: {status or '<empty>'}")
    if incomplete:
        sample = "; ".join(incomplete[:5])
        suffix = "" if len(incomplete) <= 5 else f"; ... ({len(incomplete)} total)"
        raise SystemExit(
            "stable 2.0+ publication requires every hardware/checklist row to "
            f"have evidence; incomplete rows: {sample}{suffix}"
        )
    if nonpassing:
        sample = "; ".join(nonpassing[:5])
        suffix = "" if len(nonpassing) <= 5 else f"; ... ({len(nonpassing)} total)"
        raise SystemExit(
            "stable 2.0+ publication requires every hardware/checklist status "
            f"to be Pass; non-Pass status cells: {sample}{suffix}"
        )
    # A checklist with no recognisable `Status` table proves nothing, so an
    # emptied or restructured file must fail rather than pass by omission.
    if status_rows == 0:
        raise SystemExit(
            "stable 2.0+ publication requires hardware evidence rows; "
            f"{checklist} has no table row under a 'Status' column"
        )

    placeholder = re.compile(
        r"^(pending|tbd|todo|n/?a|none|unknown|xxx)\b", re.IGNORECASE
    )

    def recorded(label):
        pattern = re.compile(rf"^{re.escape(label)}:[ \t]*(.*)$", re.MULTILINE)
        for match in pattern.finditer(checklist_text):
            value = match.group(1).strip()
            if len(value) >= 2 and not placeholder.match(value):
                return value
        return None

    if recorded("Final sign-off") is None or recorded("Evidence index") is None:
        raise SystemExit(
            "stable 2.0+ publication requires non-placeholder 'Final sign-off:' "
            f"and 'Evidence index:' records in {checklist}"
        )
PY

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    echo "version=${release_version}" >> "${GITHUB_OUTPUT}"
fi

echo "release metadata is consistent for ${release_tag}"
