#!/usr/bin/env bash

# The blocking facade is a native synchronous implementation. Keep its normal
# dependency graph free of async runtimes and executors; dev-dependencies are
# deliberately excluded because this checks what a downstream application
# links, not how this crate tests itself.

set -euo pipefail

readonly GRAFTON_VISCA_STABLE_TOOLCHAIN='1.98.0'
readonly forbidden=(
    tokio
    tokio-serial
    smol
    async-executor
    async-io
    async-lock
    futures-lite
    pollster
)

check_graph() {
    local features="$1"
    local graph
    graph="$({
        cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" tree \
            --no-default-features \
            --features "${features}" \
            --edges normal \
            --prefix none \
            --format '{p}'
    })"

    local dependency
    for dependency in "${forbidden[@]}"; do
        if grep -Eq "^${dependency} v[0-9]" <<<"${graph}"; then
            printf 'blocking feature set %q unexpectedly links async dependency %q\n' \
                "${features}" "${dependency}" >&2
            printf '%s\n' "${graph}" >&2
            return 1
        fi
    done
}

check_graph blocking
check_graph transport-serial

cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" check --lib --no-default-features --features blocking

printf 'Blocking dependency boundary passed: native blocking has no async runtime/executor dependencies.\n'
