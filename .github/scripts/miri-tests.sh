#!/usr/bin/env bash

# Miri checks for the runtime-neutral engine and applicable library surfaces.

set -euo pipefail

readonly YELLOW='\033[1;33m'
readonly GREEN='\033[0;32m'
readonly RED='\033[0;31m'
readonly NC='\033[0m'

# Every run below is filtered by module path. A libtest filter that matches
# nothing exits 0 — the binary prints "running 0 tests" and reports success —
# so renaming or moving a module would quietly turn this whole job vacuous
# instead of turning it red. Each run therefore asserts that its filter
# selected at least one test.
run_miri() {
    local description="$1"
    shift

    local log
    log="${TMPDIR:-/tmp}/grafton-miri-${description//[^[:alnum:]]/_}.log"

    printf '%bMiri testing: %s%b\n' "$YELLOW" "$description" "$NC"
    if ! cargo miri test "$@" 2>&1 | tee "$log"; then
        printf '%b✗ %s failed%b\n' "$RED" "$description" "$NC"
        return 1
    fi
    if ! grep -Eq '^running [1-9][0-9]* tests?$' "$log"; then
        printf '%b✗ %s matched zero tests%b\n' "$RED" "$description" "$NC"
        printf 'The name filter selected nothing, so this check was vacuous.\n'
        printf 'A renamed or moved module is the usual cause; update the filter.\n'
        return 1
    fi
    printf '%b✓ %s passed%b\n\n' "$GREEN" "$description" "$NC"
}

run_check() {
    local description="$1"
    shift

    printf '%bFeature compile checking: %s%b\n' "$YELLOW" "$description" "$NC"
    cargo check --lib "$@"
    printf '%b✓ %s passed%b\n\n' "$GREEN" "$description" "$NC"
}

echo "=========================================="
echo "Starting Miri safety checks"
echo "=========================================="

export MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance"
cargo miri setup

# Miri is applicable to the synchronous, I/O-free domain boundary.  A library
# test target still compiles every unit-test module once, so keep that expensive
# compilation to one no-default build and bound execution to pure modules.  The
# long deterministic trace and generated property test remain covered by the
# ordinary engine/property CI jobs, not by an unbounded Miri run.
run_miri "deterministic protocol engine" \
    --no-default-features --lib 'runtime::engine::tests::' -- \
    --test-threads=1 \
    --skip arbitrary_stale_and_reordered_inputs_preserve_invariants \
    --skip arbitrary_ordered_and_stale_inputs_preserve_invariants_property
run_miri "prepared request domain" --no-default-features --lib 'prepared::tests::' -- --test-threads=1
run_miri "raw request contracts" --no-default-features --lib 'raw::tests::' -- --test-threads=1
run_miri "VISCA frame parsing" \
    --no-default-features --lib 'protocol::framer::tests::' -- --test-threads=1
run_miri "VISCA response decoding" \
    --no-default-features --lib 'protocol::response::tests::' -- --test-threads=1
run_miri "Sony envelope parsing" \
    --no-default-features --lib 'protocol::sony::tests::' -- --test-threads=1

# Runtime adapters, transports, serialization, and test utilities are not
# meaningful Miri executions here (they either require OS I/O or third-party
# runtime internals), but every supported feature combination still receives a
# library compile check in this job.
run_check "no-default pure library" --no-default-features
run_check "blocking library" --no-default-features --features blocking
run_check "runtime-neutral async library" --no-default-features --features async
run_check "Tokio library" --no-default-features --features runtime-tokio
run_check "smol library" --no-default-features --features runtime-smol
run_check "Tokio + dyn-api library" \
    --no-default-features --features runtime-tokio,dyn-api
run_check "smol + dyn-api library" \
    --no-default-features --features runtime-smol,dyn-api
run_check "blocking + async library" --no-default-features --features blocking,async
run_check "test-utils library" --no-default-features --features test-utils
run_check "serde + schemars + ts-rs library" \
    --no-default-features --features serde,schemars,ts-rs

echo "=========================================="
printf '%bAll bounded Miri and feature checks passed%b\n' "$GREEN" "$NC"
echo "=========================================="
