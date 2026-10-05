#!/usr/bin/env bash

# Canonical 2.0 feature-matrix script.
#
# `FEATURE_LEGS` below is the one list of supported feature shapes. Modes:
#
# - `test` (default): the full local/release run — the rejection checks, then
#   `cargo test` on every leg, then the checks that are not a feature shape.
# - `clippy`: `cargo clippy ... -- -D warnings` on every leg and the
#   all-features workspace.
# - `doc`: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` on every leg's
#   documented crate (see `doc_args`) and the all-features workspace.
# - `rejections`: removed and facade-less feature selections must fail to
#   compile, for the right reason.
# - `list-json`: the legs as `[{"name", "command", "doc", "doc_command"}]`
#   (`doc` is false for a leg with no doc build); CI builds
#   its per-leg test/clippy/doc matrix from it, so CI and local runs share the
#   one list.
#
# Usage: test-all-features.sh [test|clippy|doc|rejections|list-json]

set -euo pipefail

readonly MODE="${1:-test}"
case "$MODE" in
test | clippy | doc | rejections | list-json) ;;
*)
    echo "usage: $0 [test|clippy|doc|rejections|list-json]" >&2
    exit 2
    ;;
esac

# One supported feature shape per entry: `name|cargo arguments`.
readonly FEATURE_LEGS=(
    "no-default pure engine/domain + smoke|--no-default-features --lib --test no_default_smoke"
    "no-default all targets|--no-default-features --all-targets"
    "default blocking|--workspace --all-targets"
    "blocking-only|--no-default-features --features blocking --all-targets"
    "blocking + dyn-api|--no-default-features --features blocking,dyn-api --all-targets"
    "default + serde|--features serde --all-targets"
    "blocking + test-utils|--features blocking,test-utils --all-targets"
    "runtime-neutral async|--no-default-features --features async --all-targets"
    "Tokio runtime|--no-default-features --features runtime-tokio --all-targets"
    "smol runtime|--no-default-features --features runtime-smol --all-targets"
    "Tokio + smol runtimes|--no-default-features --features runtime-tokio,runtime-smol --all-targets"
    "blocking + Tokio|--no-default-features --features blocking,runtime-tokio --all-targets"
    "blocking + smol|--no-default-features --features blocking,runtime-smol --all-targets"
    "blocking + Tokio + smol runtimes|--no-default-features --features blocking,runtime-tokio,runtime-smol --all-targets"
    "Tokio + dyn-api|--no-default-features --features runtime-tokio,dyn-api --all-targets"
    "smol + dyn-api|--no-default-features --features runtime-smol,dyn-api --all-targets"
    "blocking + async + dyn-api|--no-default-features --features blocking,async,dyn-api --all-targets"
    "blocking serial transport|--no-default-features --features transport-serial --all-targets"
    "Tokio serial transport|--no-default-features --features runtime-tokio,transport-serial-tokio --all-targets"
    "smol + Tokio serial API contract|--no-default-features --features runtime-smol,transport-serial-tokio --test api_stability_test"
    "Tokio + blocking serial transport|--no-default-features --features runtime-tokio,transport-serial --all-targets"
    "serde + schemars + ts-rs|--no-default-features --features serde,schemars,ts-rs --all-targets"
    # `test-utils` alone proves the toolkit compiles with no facade selected; it
    # cannot run it, because `ScriptedBlockingTransport` needs `blocking` and
    # `ScriptedTransport`/`DeterministicExecutor` need `async`. The union below is
    # the entry that actually executes the shipped toolkit and the tests built on
    # it; without it the 53 test-utils tests (11 issue-566 scripted, 5 inquiry
    # simulator, 4 timeout, 15 deterministic-executor, 11 scripted-transport,
    # and 7 `testing::frame_tests`) run in no CI job at all.
    "test-utils|--no-default-features --features test-utils --all-targets"
    "test-utils + blocking + Tokio|--no-default-features --features test-utils,blocking,runtime-tokio --all-targets"
    "macro derives|-p grafton-visca-macros"
    # The workspace/default leg leaves this fixture's optional serde, schemars,
    # and ts-rs oracle cfg'd out; the all-features leg is only a check. Run
    # this fixture explicitly so its
    # `renamed_dependency_derive_and_profile_contract_runs` behavioral
    # contract is not merely compile-checked.
    "renamed range helper derives|-p phase4-renamed-dependency --features range-helper-derives --all-targets"
)

# Runs `cargo <subcommand>` over every feature leg.
run_feature_legs() {
    local subcommand="$1"
    shift
    local leg name args
    for leg in "${FEATURE_LEGS[@]}"; do
        name="${leg%%|*}"
        read -r -a args <<<"${leg#*|}"
        run_test "$name ($subcommand)" cargo_stable "$subcommand" "${args[@]}" "$@"
    done
}

# Prints the `cargo doc` arguments of one leg's command, and fails when the
# leg documents nothing. Rustdoc takes no target selectors, so `--all-targets`
# and `--test <name>` are dropped, and `--workspace` is dropped so every leg
# documents the crate it names (the root package unless `-p` selects one).
# The range-helper fixture is a test-only workspace crate whose only feature
# shape is already documented by the all-features workspace build, so its leg
# has no doc build of its own.
doc_args() {
    local -a args out=()
    local word skip_next=false
    read -r -a args <<<"$1"
    if [[ " ${args[*]} " == *" phase4-renamed-dependency "* ]]; then
        return 1
    fi
    for word in "${args[@]}"; do
        if $skip_next; then
            skip_next=false
        elif [[ "$word" == --test ]]; then
            skip_next=true
        elif [[ "$word" != --all-targets && "$word" != --workspace ]]; then
            out+=("$word")
        fi
    done
    # A leg that selects no crate target documents the root library.
    if [[ " ${out[*]} " != *" -p "* && " ${out[*]} " != *" --lib "* ]]; then
        out+=(--lib)
    fi
    echo "${out[*]}"
}

if [[ "$MODE" == list-json ]]; then
    for leg in "${FEATURE_LEGS[@]}"; do
        doc=true
        doc_command="$(doc_args "${leg#*|}")" || doc=false
        jq -cn --arg name "${leg%%|*}" --arg command "${leg#*|}" \
            --argjson doc "$doc" --arg doc_command "$doc_command" \
            '{name: $name, command: $command, doc: $doc, doc_command: $doc_command}'
    done | jq -cs .
    exit 0
fi

readonly GREEN='\033[0;32m'
readonly RED='\033[0;31m'
readonly YELLOW='\033[1;33m'
readonly NC='\033[0m'
readonly GRAFTON_VISCA_STABLE_TOOLCHAIN='1.98.0'

cargo_stable() {
    command cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" "$@"
}

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

run_reexported_range_helper_test() (
    local export_dir
    export_dir="$(mktemp -d "${TMPDIR:-/tmp}/grafton-range-reexport.XXXXXX")"
    trap 'find "$export_dir" -depth -delete' EXIT

    TS_RS_EXPORT_DIR="$export_dir" cargo_stable test \
        --manifest-path tests/fixtures/range_type_reexport/Cargo.toml \
        -p range-macro-consumer --features range-helper-derives

    test -s "${export_dir}/ReexportedRange.ts"
    grep -Fq 'export type ReexportedRange = number;' \
        "${export_dir}/ReexportedRange.ts"
)

expect_unknown_feature() {
    local feature="$1"
    local output_file
    output_file="${TMPDIR:-/tmp}/grafton-removed-feature-${feature//[^[:alnum:]]/_}.log"

    printf '%bChecking removed feature rejection: %s%b\n' "$YELLOW" "$feature" "$NC"
    if cargo_stable check --no-default-features --features "$feature" >"$output_file" 2>&1; then
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

# `dyn-api` projects the enabled facades' camera views. On its own it must be
# rejected with its explanation, never compile to an empty feature (#828).
expect_facade_required() {
    local feature="$1"
    local output_file
    output_file="${TMPDIR:-/tmp}/grafton-facade-required-${feature//[^[:alnum:]]/_}.log"

    printf '%bChecking facade-less feature rejection: %s%b\n' "$YELLOW" "$feature" "$NC"
    if cargo_stable check --no-default-features --features "$feature" >"$output_file" 2>&1; then
        cat "$output_file"
        printf '%b✗ %s without a facade unexpectedly compiled%b\n' "$RED" "$feature" "$NC"
        return 1
    fi
    if ! grep -Fq "feature \`${feature}\` projects a session facade's camera views" "$output_file"; then
        cat "$output_file"
        printf '%b✗ %s without a facade failed for the wrong reason%b\n' "$RED" "$feature" "$NC"
        return 1
    fi
    printf '%b✓ %s without a facade is rejected with its explanation%b\n\n' "$GREEN" "$feature" "$NC"
}

# The compatibility names must remain unknown rather than silently selecting a
# second implementation, and `dyn-api` alone must be rejected rather than
# compile to an empty feature. These live-compiler checks run in `rejections`
# mode (CI runs it as its own step) and in the full `test` run; the manifest
# side of the removed names is also pinned by
# `ecosystem_feature_inventory_matches_cargo_manifest` and
# `removed_1x_feature_aliases_are_absent_from_the_manifest` in
# `tests/issue_548_supported_surface_inventory.rs`.
run_rejections() {
    expect_unknown_feature "mode-async"
    expect_unknown_feature "async-core"
    expect_unknown_feature "mode-blocking"
    expect_facade_required "dyn-api"
}

if [[ "$MODE" == rejections ]]; then
    run_rejections
    exit 0
fi

# Builds every leg's rustdoc with `-D warnings`; see `doc_args`.
run_doc_legs() {
    local leg name doc
    local -a args
    for leg in "${FEATURE_LEGS[@]}"; do
        name="${leg%%|*}"
        doc="$(doc_args "${leg#*|}")" || continue
        read -r -a args <<<"$doc"
        run_test "$name (doc)" env RUSTDOCFLAGS="-D warnings" \
            cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" doc --no-deps "${args[@]}"
    done
}

if [[ "$MODE" == doc ]]; then
    run_doc_legs
    run_test "all-features workspace (doc)" env RUSTDOCFLAGS="-D warnings" \
        cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" doc --no-deps --workspace --all-features
    echo "=========================================="
    printf '%bCanonical 2.0 feature-shape rustdoc passed%b\n' "$GREEN" "$NC"
    echo "=========================================="
    exit 0
fi

if [[ "$MODE" == clippy ]]; then
    run_feature_legs clippy -- -D warnings
    run_test "all-features workspace (clippy)" \
        cargo_stable clippy --workspace --all-features --all-targets -- -D warnings
    echo "=========================================="
    printf '%bCanonical 2.0 feature-shape lint passed%b\n' "$GREEN" "$NC"
    echo "=========================================="
    exit 0
fi

echo "=========================================="
echo "Starting canonical 2.0 feature tests"
echo "=========================================="

run_rejections
run_feature_legs test
run_test "native blocking dependency boundary" \
    bash .github/scripts/check-blocking-dependency-boundary.sh
run_test "re-exported range macro without helpers" \
    cargo_stable test --manifest-path tests/fixtures/range_type_reexport/Cargo.toml \
        -p range-macro-consumer
run_test "re-exported range macro with helpers" \
    run_reexported_range_helper_test

run_test "all-features and all-targets check" \
    cargo_stable check --workspace --all-features --all-targets

run_test "blocking examples" \
    cargo_stable check --examples --no-default-features --features blocking
run_test "Tokio examples" \
    cargo_stable check --examples --no-default-features --features runtime-tokio
run_test "smol examples" \
    cargo_stable check --examples --no-default-features --features runtime-smol
run_test "all-feature examples" \
    cargo_stable check --examples --all-features

run_test "no-default doctests" \
    cargo_stable test --doc --no-default-features
run_test "async doctests" \
    cargo_stable test --doc --no-default-features --features async
run_test "all-feature doctests" \
    cargo_stable test --doc --all-features

run_filtered_test "generated engine invariant property" \
    cargo_stable test --no-default-features --lib arbitrary_ordered_and_stale_inputs_preserve_invariants_property

echo "=========================================="
printf '%bCanonical 2.0 feature matrix passed%b\n' "$GREEN" "$NC"
echo "=========================================="
