#!/usr/bin/env bash
set -euo pipefail

if ! repository_root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
    echo "release validation requires a Git worktree" >&2
    exit 1
fi
cd "${repository_root}"

# Release metadata must be read from an immutable committed candidate, never
# from staged, unstaged, or nonignored-untracked working-tree contents. This
# deliberately leaves ignored build products (including this library
# workspace's generated Cargo.lock) outside the gate.
if ! git rev-parse --verify --quiet HEAD^{commit} >/dev/null; then
    echo "release validation requires HEAD to resolve to a committed candidate" >&2
    exit 1
fi
worktree_status="$(git status --porcelain=v1 --untracked-files=all)"
if [[ -n "${worktree_status}" ]]; then
    echo "release validation requires a clean candidate worktree (staged, unstaged, and nonignored untracked files are forbidden)" >&2
    printf '%s\n' "${worktree_status}" >&2
    exit 1
fi

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
import pathlib
import re
import subprocess
import sys
import tomllib

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

# Hardware smoke checks are optional and private for every release.
# Public device records and hardware sign-off are not release metadata (#1008).

PY

if [[ -n "${GITHUB_OUTPUT:-}" ]]; then
    echo "version=${release_version}" >> "${GITHUB_OUTPUT}"
fi

echo "release metadata is consistent for ${release_tag}"
