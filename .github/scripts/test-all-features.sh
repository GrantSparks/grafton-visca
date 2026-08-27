#!/usr/bin/env bash

# Canonical 2.0 feature-matrix test script.
#
# Keep this matrix aligned with the support contract in CI. Removed feature
# names are exercised only by the explicit rejection checks below.

set -euo pipefail

readonly GREEN='\033[0;32m'
readonly RED='\033[0;31m'
readonly YELLOW='\033[1;33m'
readonly NC='\033[0m'

run_test() {
    local description="$1"
    shift

    printf '%bTesting: %s%b\n' "$YELLOW" "$description" "$NC"
    if "$@"; then
        printf '%b✓ %s passed%b\n\n' "$GREEN" "$description" "$NC"
    else
        printf '%b✗ %s failed%b\n' "$RED" "$description" "$NC"
        return 1
    fi
}

# A libtest name filter that matches nothing exits 0: the binary prints
# "running 0 tests" and reports success. A renamed, moved or cfg-ed-out test
# would silently turn a gate into a no-op instead of turning it red, so every
# run that carries a filter asserts that it selected at least one test.
run_filtered_test() {
    local description="$1"
    shift

    local log
    log="${TMPDIR:-/tmp}/grafton-filtered-${description//[^[:alnum:]]/_}.log"

    printf '%bTesting: %s%b\n' "$YELLOW" "$description" "$NC"
    if ! "$@" 2>&1 | tee "$log"; then
        printf '%b✗ %s failed%b\n' "$RED" "$description" "$NC"
        return 1
    fi
    if ! grep -Eq '^running [1-9][0-9]* tests?$' "$log"; then
        printf '%b✗ %s matched zero tests%b\n' "$RED" "$description" "$NC"
        printf 'The name filter selected nothing, so this check was vacuous.\n'
        printf 'A renamed or moved test is the usual cause; update the filter.\n'
        return 1
    fi
    printf '%b✓ %s passed%b\n\n' "$GREEN" "$description" "$NC"
}

expect_unknown_feature() {
    local feature="$1"
    local output_file
    output_file="${TMPDIR:-/tmp}/grafton-removed-feature-${feature//[^[:alnum:]]/_}.log"

    printf '%bChecking removed feature rejection: %s%b\n' "$YELLOW" "$feature" "$NC"
    if cargo check --no-default-features --features "$feature" >"$output_file" 2>&1; then
        cat "$output_file"
        printf '%b✗ removed feature %s unexpectedly succeeded%b\n' "$RED" "$feature" "$NC"
        return 1
    fi
    if ! grep -Fq "does not contain this feature: ${feature}" "$output_file"; then
        cat "$output_file"
        printf '%b✗ removed feature %s failed for the wrong reason%b\n' "$RED" "$feature" "$NC"
        return 1
    fi
    printf '%b✓ removed feature %s is rejected by Cargo%b\n\n' "$GREEN" "$feature" "$NC"
}

echo "=========================================="
echo "Starting canonical 2.0 feature tests"
echo "=========================================="

# The compatibility names must remain unknown rather than silently selecting a
# second implementation. This is the only live-compiler proof of that; CI relies
# on `ecosystem_feature_inventory_matches_cargo_manifest` and
# `removed_1x_feature_aliases_are_absent_from_the_manifest` in
# `tests/issue_548_supported_surface_inventory.rs`, which pin the same fact
# against the manifest without paying for three extra `cargo check` runs.
expect_unknown_feature "mode-async"
expect_unknown_feature "async-core"
expect_unknown_feature "mode-blocking"

run_test "no-default pure engine/domain" \
    cargo test --no-default-features --lib
run_test "default blocking" \
    cargo test --workspace --all-targets
run_test "blocking-only" \
    cargo test --no-default-features --features blocking --all-targets
run_test "runtime-neutral async" \
    cargo test --no-default-features --features async --all-targets
run_test "Tokio runtime" \
    cargo test --no-default-features --features runtime-tokio --all-targets
run_test "smol runtime" \
    cargo test --no-default-features --features runtime-smol --all-targets
run_test "Tokio + smol runtimes" \
    cargo test --no-default-features --features runtime-tokio,runtime-smol --all-targets
run_test "blocking + Tokio" \
    cargo test --no-default-features --features blocking,runtime-tokio --all-targets
run_test "blocking + smol" \
    cargo test --no-default-features --features blocking,runtime-smol --all-targets
run_test "blocking + Tokio + smol runtimes" \
    cargo test --no-default-features --features blocking,runtime-tokio,runtime-smol --all-targets
run_test "Tokio + dyn-api" \
    cargo test --no-default-features --features runtime-tokio,dyn-api --all-targets
run_test "smol + dyn-api" \
    cargo test --no-default-features --features runtime-smol,dyn-api --all-targets
run_test "blocking + async + dyn-api" \
    cargo test --no-default-features --features blocking,async,dyn-api --all-targets
run_test "blocking serial transport" \
    cargo test --no-default-features --features transport-serial --all-targets
run_test "Tokio serial transport" \
    cargo test --no-default-features --features runtime-tokio,transport-serial-tokio --all-targets
run_test "Tokio + blocking serial transport" \
    cargo test --no-default-features --features runtime-tokio,transport-serial --all-targets
run_test "serde + schemars + ts-rs" \
    cargo test --no-default-features --features serde,schemars,ts-rs --all-targets
# `test-utils` alone proves the toolkit compiles with no facade selected; it
# cannot run it, because `ScriptedBlockingTransport` needs `blocking` and
# `ScriptedTransport`/`DeterministicExecutor` need `async`. The union below is
# the entry that actually executes the shipped toolkit and the tests built on
# it; without it those tests run in no CI job at all.
run_test "test-utils" \
    cargo test --no-default-features --features test-utils --all-targets
run_test "test-utils + blocking + Tokio" \
    cargo test --no-default-features --features test-utils,blocking,runtime-tokio --all-targets
run_test "macro derives" \
    cargo test -p grafton-visca-macros

run_test "all-features and all-targets check" \
    cargo check --workspace --all-features --all-targets

run_test "blocking examples" \
    cargo check --examples --no-default-features --features blocking
run_test "Tokio examples" \
    cargo check --examples --no-default-features --features runtime-tokio
run_test "smol examples" \
    cargo check --examples --no-default-features --features runtime-smol
run_test "all-feature examples" \
    cargo check --examples --all-features

run_test "no-default doctests" \
    cargo test --doc --no-default-features
run_test "async doctests" \
    cargo test --doc --no-default-features --features async
run_test "all-feature doctests" \
    cargo test --doc --all-features

run_filtered_test "generated engine invariant property" \
    cargo test --no-default-features --lib arbitrary_ordered_and_stale_inputs_preserve_invariants_property

echo "=========================================="
printf '%bCanonical 2.0 feature matrix passed%b\n' "$GREEN" "$NC"
echo "=========================================="
