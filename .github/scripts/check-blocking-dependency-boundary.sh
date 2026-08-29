#!/usr/bin/env bash

# The blocking facade is a native synchronous implementation. Keep its normal
# dependency graph free of async runtimes and executors; dev-dependencies are
# deliberately excluded because this checks what a downstream application
# links, not how this crate tests itself.
#
# `test-utils` is included below: it is a publicly documented, downstream-
# enableable feature, so a blocking consumer that turns it on must still get a
# native synchronous graph. The async testkit and its executor dependencies are
# gated behind `async` and must not leak in here.

set -euo pipefail

readonly GRAFTON_VISCA_STABLE_TOOLCHAIN='1.98.0'
readonly forbidden=(
    tokio
    tokio-serial
    smol
    async-executor
    async-io
    async-lock
    async-std
    async-global-executor
    futures-lite
    futures-executor
    pollster
)

# A dependency that must appear in every blocking feature set checked here.
# It backs a positive control: if `cargo tree`'s output shape ever changes so
# the forbidden-dependency greps silently match nothing, this sentinel stops
# matching too and the gate fails loudly instead of passing green on a graph it
# never actually parsed.
readonly present_sentinel='bytes'

check_graph() {
    local features="$1"
    local graph
    # `--target all` includes target-conditional dependencies (e.g. a
    # `cfg(windows)`-only async crate) that would otherwise be invisible on the
    # Linux CI host and slip past this gate.
    graph="$({
        cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" tree \
            --no-default-features \
            --features "${features}" \
            --edges normal \
            --target all \
            --prefix none \
            --format '{p}'
    })"

    if ! grep -Eq "^${present_sentinel} v[0-9]" <<<"${graph}"; then
        printf 'dependency-boundary gate self-check failed for feature set %q: sentinel dependency %q not found; cargo tree output shape may have changed, so the forbidden-dependency checks below cannot be trusted\n' \
            "${features}" "${present_sentinel}" >&2
        printf '%s\n' "${graph}" >&2
        return 1
    fi

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
check_graph blocking,test-utils

cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" check --lib --no-default-features --features blocking
cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" check --lib --no-default-features --features blocking,test-utils

printf 'Blocking dependency boundary passed: native blocking (with and without test-utils) has no async runtime/executor dependencies.\n'
