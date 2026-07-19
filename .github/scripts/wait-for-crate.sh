#!/usr/bin/env bash
set -euo pipefail

crate_name="${1:?crate name is required}"
crate_version="${2:?crate version is required}"
timeout_seconds="${3:-300}"
deadline=$((SECONDS + timeout_seconds))

while (( SECONDS < deadline )); do
    if cargo info --registry crates-io "${crate_name}@${crate_version}" >/dev/null 2>&1; then
        echo "${crate_name}@${crate_version} is available from the crates.io index"
        exit 0
    fi
    echo "waiting for ${crate_name}@${crate_version} to reach the crates.io index..."
    sleep 10
done

echo "timed out waiting for ${crate_name}@${crate_version} after ${timeout_seconds}s" >&2
exit 1
