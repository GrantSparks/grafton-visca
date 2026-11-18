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

# crates.io may reject the runner's anonymous/default curl identity. Keep this
# artifact verifier identifiable and give registry operators a stable contact.
registry_user_agent="grafton-visca-release-verifier/2.0.0 (+https://github.com/GrantSparks/grafton-visca)"
curl --fail --location --silent --show-error --retry 3 \
    --user-agent "${registry_user_agent}" \
    --output "${published_payload}" "${download_url}"

local_sha256="$(sha256sum -- "${local_payload}" | awk '{print $1}')"
published_sha256="$(sha256sum -- "${published_payload}" | awk '{print $1}')"
printf '%s\n' \
    "local SHA-256:     ${local_sha256}" \
    "published SHA-256: ${published_sha256}"

python3 - "${crate_name}-${crate_version}" "${temporary_directory}" \
    "${local_payload}" "${published_payload}" <<'PY'
import filecmp
from pathlib import Path
import shutil
import sys
import tarfile

expected_root, temporary, local, published = sys.argv[1:]


def unpack(archive_path, destination):
    files = set()
    seen = set()
    with tarfile.open(archive_path, "r:gz") as archive:
        for member in archive:
            parts = member.name.split("/")
            if (
                parts[0] != expected_root
                or any(part in ("", ".", "..") or "\\" in part or ":" in part for part in parts)
            ):
                raise ValueError(
                    f"entry {member.name!r} is outside package directory {expected_root!r}"
                )
            relative = "/".join(parts[1:])
            if relative in seen:
                raise ValueError(f"duplicate package entry: {member.name!r}")
            seen.add(relative)

            path = destination.joinpath(*parts[1:])
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            elif relative and member.isfile():
                path.parent.mkdir(parents=True, exist_ok=True)
                with archive.extractfile(member) as source, path.open("xb") as target:
                    shutil.copyfileobj(source, target)
                files.add(relative)
            else:
                raise ValueError(f"unsupported package entry: {member.name!r}")
    if not files:
        raise ValueError("package contains no files")
    return files


root = Path(temporary)
local_root = root / "local"
published_root = root / "published"
try:
    local_files = unpack(local, local_root)
    published_files = unpack(published, published_root)
except (OSError, EOFError, ValueError, tarfile.TarError) as error:
    raise SystemExit(f"invalid crate archive: {error}") from error

differences = []
for relative in sorted(local_files | published_files):
    if relative not in local_files:
        differences.append(f"unexpected registry file: {relative}")
    elif relative not in published_files:
        differences.append(f"missing registry file: {relative}")
    elif relative != "Cargo.lock" and not filecmp.cmp(
        local_root / relative, published_root / relative, shallow=False
    ):
        differences.append(f"registry file differs: {relative}")
if differences:
    raise SystemExit("\n".join(differences))
PY

echo "verified ${crate_name}@${crate_version} registry contents match apart from the embedded Cargo.lock"
