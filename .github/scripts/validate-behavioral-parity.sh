#!/usr/bin/env bash
set -euo pipefail

# Resolve the validator from this script, then let it discover the repository
# root through Git.  No workspace-specific absolute path is required.
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(git -C "${script_dir}/../.." rev-parse --show-toplevel)"
cd "${repo_root}"
exec python3 "${repo_root}/.github/behavioral-parity-1x/validate.py" "$@"
