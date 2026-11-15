#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repository_root="${ERROR_REACHABILITY_REPOSITORY_ROOT:-$(cd "${script_directory}/../.." && pwd)}"
temporary_directory="$(mktemp -d)"
trap 'rm -rf -- "${temporary_directory}"' EXIT

# This standalone binary shares the same scanner as the closed-inventory tests;
# it needs no registry access or Cargo build of grafton-visca.
rustc +1.98.0 --edition=2021 --deny warnings \
    --crate-name error_variant_reachability \
    "${script_directory}/error-variant-reachability.rs" \
    -o "${temporary_directory}/check"
"${temporary_directory}/check" "${repository_root}"
