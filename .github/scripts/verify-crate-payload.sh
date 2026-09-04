#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 CRATE_NAME VERSION LOCAL_CRATE [REGISTRY_API_BASE_URL]" >&2
    exit 2
}

if (( $# < 3 || $# > 4 )); then
    usage
fi

crate_name="$1"
crate_version="$2"
local_payload="$3"
registry_base_url="${4:-https://crates.io/api/v1/crates}"

# These are the same release-version constraints enforced before publication;
# keeping the helper strict prevents a name or version from changing the URL
# path it downloads.
if [[ ! "${crate_name}" =~ ^[A-Za-z0-9][A-Za-z0-9_-]*$ ]]; then
    echo "invalid crate name: ${crate_name}" >&2
    exit 1
fi
if [[ ! "${crate_version}" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]]; then
    echo "invalid crate version: ${crate_version}" >&2
    exit 1
fi
if [[ "${registry_base_url}" != "https://crates.io/api/v1/crates" && "${registry_base_url}" != file://* ]]; then
    echo "registry API base must be crates.io or a file:// fixture URL" >&2
    exit 1
fi
if [[ ! -f "${local_payload}" ]]; then
    echo "local crate payload does not exist: ${local_payload}" >&2
    exit 1
fi

download_url="${registry_base_url%/}/${crate_name}/${crate_version}/download"
temporary_directory="$(mktemp -d)"
trap 'rm -rf "${temporary_directory}"' EXIT
published_payload="${temporary_directory}/${crate_name}-${crate_version}.crate"

curl --fail --location --silent --show-error --retry 3 \
    --output "${published_payload}" "${download_url}"

local_sha256="$(sha256sum -- "${local_payload}" | awk '{print $1}')"
published_sha256="$(sha256sum -- "${published_payload}" | awk '{print $1}')"
if [[ "${local_sha256}" != "${published_sha256}" ]]; then
    echo "${crate_name}@${crate_version} registry payload SHA-256 mismatch" >&2
    echo "local:     ${local_sha256}" >&2
    echo "published: ${published_sha256}" >&2
    exit 1
fi
if ! cmp -- "${local_payload}" "${published_payload}"; then
    echo "${crate_name}@${crate_version} registry payload differs byte-for-byte" >&2
    exit 1
fi

echo "verified ${crate_name}@${crate_version} registry payload matches local SHA-256 ${local_sha256}"
