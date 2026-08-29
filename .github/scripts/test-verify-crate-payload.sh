#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
helper="${script_directory}/verify-crate-payload.sh"
fixture_directory="$(mktemp -d)"
trap 'rm -rf "${fixture_directory}"' EXIT

crate_name="grafton-visca"
crate_version="2.0.0-rc.1"
local_payload="${fixture_directory}/${crate_name}-${crate_version}.crate"
registry_directory="${fixture_directory}/registry/${crate_name}/${crate_version}"
mkdir -p "${registry_directory}"
printf 'fixture crate payload\n' > "${local_payload}"
cp -- "${local_payload}" "${registry_directory}/download"

"${helper}" \
    "${crate_name}" \
    "${crate_version}" \
    "${local_payload}" \
    "file://${fixture_directory}/registry"

printf 'different fixture crate payload\n' > "${registry_directory}/download"
if "${helper}" \
    "${crate_name}" \
    "${crate_version}" \
    "${local_payload}" \
    "file://${fixture_directory}/registry"; then
    echo "expected a mismatched registry payload to fail" >&2
    exit 1
fi

echo "verify-crate-payload self-test passed"
