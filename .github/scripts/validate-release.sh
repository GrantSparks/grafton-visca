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
import subprocess
import sys
import tomllib

expected = sys.argv[1]
root = pathlib.Path.cwd()

metadata = subprocess.run(
    ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
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

lock = tomllib.loads((root / "Cargo.lock").read_text())
locked = {
    package["name"]: package["version"]
    for package in lock["package"]
    if package["name"] in {"grafton-visca", "grafton-visca-macros"}
}
for name in ("grafton-visca", "grafton-visca-macros"):
    if locked.get(name) != expected:
        raise SystemExit(
            f"Cargo.lock {name} version {locked.get(name)!r} does not match {expected}"
        )

changelog = (root / "CHANGELOG.md").read_text()
heading = re.compile(
    rf"^## \[{re.escape(expected)}\] - \d{{4}}-\d{{2}}-\d{{2}}$", re.MULTILINE
)
if not heading.search(changelog):
    raise SystemExit(
        f"CHANGELOG.md must contain a dated '## [{expected}] - YYYY-MM-DD' heading"
    )
PY

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    echo "version=${release_version}" >> "${GITHUB_OUTPUT}"
fi

echo "release metadata is consistent for ${release_tag}"
