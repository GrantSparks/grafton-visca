#!/usr/bin/env bash

# Canonical 2.0 feature-matrix script.
#
# `FEATURE_LEGS` below is the one list of supported feature shapes. Modes:
#
# - `test` (default): the full local/release run — the rejection checks, then
#   `cargo test` on every leg (see `test-leg`), then the checks that are not a
#   feature shape.
# - `test-leg NAME`: `cargo test` on one leg, as CI's per-leg job runs it. The
#   engine model tests (`engine-model-tests.sh`) cannot differ by feature
#   configuration, so they run in `ENGINE_MODEL_TEST_LEG` only and are skipped
#   by exact name everywhere else; the leg fails unless each ran exactly where
#   it is designated.
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
# - `slowest [N] [cargo test arguments]`: the N (default 15) slowest tests of
#   one run, from libtest's `--report-time` JSON. That reporter is unstable, so
#   this mode uses the pinned nightly and reports timings only: the
#   compile-contract fixtures pin stable 1.98 diagnostics and may fail under
#   nightly, so pass/fail stays the stable legs' verdict.
#
# `--jobs N` (before the mode; `test`, `clippy` and `doc` only) runs up to N
# jobs at once. Each job builds in its own target directory under
# `MATRIX_TARGET_DIR` (default `<target>/matrix`), so concurrent legs never
# share or wait on a build lock, and writes its output to its own log under
# `MATRIX_LOG_DIR` (default `<MATRIX_TARGET_DIR>/logs`); a summary with each
# job's verdict and wall time ends the run, which fails if any job failed.
# Without `--jobs` the jobs run one after another in the caller's target
# directory, printing to the terminal.
#
# Usage: test-all-features.sh [--jobs N] [test|clippy|doc|rejections|list-json]
#        test-all-features.sh test-leg NAME
#        test-all-features.sh slowest [N] [cargo test arguments]

set -euo pipefail

usage() {
    sed -n "s/^# Usage: /usage: /p; s/^#        /       /p" "${BASH_SOURCE[0]}" >&2
    exit 2
}

JOBS=0
if [[ "${1:-}" == --jobs ]]; then
    [[ "${2:-}" =~ ^[1-9][0-9]*$ ]] || usage
    JOBS="$2"
    shift 2
fi
readonly JOBS
readonly MODE="${1:-test}"
case "$MODE" in
test | clippy | doc) ;;
rejections | list-json | test-leg | slowest)
    ((JOBS == 0)) || usage
    ;;
*) usage ;;
esac

# shellcheck source=engine-model-tests.sh
source "$(dirname "${BASH_SOURCE[0]}")/engine-model-tests.sh"

# One supported feature shape per entry: `name|cargo arguments`.
readonly FEATURE_LEGS=(
    "no-default domain vocabulary + smoke|--no-default-features --lib --test no_default_smoke"
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

# The leg that runs `ENGINE_MODEL_TESTS`: the smallest facade leg. The engine
# compiles only with a facade, so the no-default leg (the domain vocabulary
# without any runtime) has no engine tests to run.
readonly ENGINE_MODEL_TEST_LEG="blocking-only"

# Prints the cargo arguments of the leg called `$1`, or fails if none is.
leg_args() {
    local leg
    for leg in "${FEATURE_LEGS[@]}"; do
        if [[ "${leg%%|*}" == "$1" ]]; then
            echo "${leg#*|}"
            return 0
        fi
    done
    echo "unknown feature leg: $1" >&2
    return 1
}

# A designated leg that names no leg would make every leg skip the engine
# model tests and pass, so a renamed or misspelled leg fails every mode,
# `list-json` included, before any leg starts.
if ! leg_args "$ENGINE_MODEL_TEST_LEG" >/dev/null 2>&1; then
    echo "ENGINE_MODEL_TEST_LEG names no feature leg: \"$ENGINE_MODEL_TEST_LEG\"" >&2
    exit 1
fi

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
readonly GRAFTON_VISCA_NIGHTLY_TOOLCHAIN='nightly-2026-08-26'

cargo_stable() {
    command cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" "$@"
}

if [[ "$MODE" == slowest ]]; then
    shift
    top=15
    if [[ "${1:-}" =~ ^[1-9][0-9]*$ ]]; then
        top="$1"
        shift
    fi
    report="$(mktemp "${TMPDIR:-/tmp}/grafton-report-time.XXXXXX")"
    trap 'rm -f "$report"' EXIT
    command cargo "+${GRAFTON_VISCA_NIGHTLY_TOOLCHAIN}" test "$@" -- \
        -Z unstable-options --report-time --format json >"$report" || true
    printf 'Slowest %s tests (%s, timings only):\n' "$top" "$*"
    jq -rR 'fromjson? | select(.type == "test" and .exec_time != null)
        | [.exec_time, .event, .name] | @tsv' "$report" |
        sort -t $'\t' -k1,1 -gr | head -n "$top" |
        awk -F '\t' '{ printf "%9.2fs  %-6s %s\n", $1, $2, $3 }'
    failed="$(jq -rR 'fromjson? | select(.type == "test" and .event == "failed") | .name' "$report")"
    if [[ -n "$failed" ]]; then
        printf 'note: failed under %s (judge pass/fail with the stable legs):\n%s\n' \
            "$GRAFTON_VISCA_NIGHTLY_TOOLCHAIN" "$failed"
    fi
    exit 0
fi

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

# Every engine model test must run, and pass, exactly once in
# `ENGINE_MODEL_TEST_LEG` and not at all in any other leg. Skipping by exact
# name means a renamed or moved test would otherwise run in every leg again
# (harmless) while its designated leg quietly stopped running it; this check
# turns that into a failure, the way a libtest filter that matches nothing
# would otherwise pass vacuously.
check_engine_model_tests() {
    local leg_name="$1" log="$2" model_test runs verdict=0
    for model_test in "${ENGINE_MODEL_TESTS[@]}"; do
        runs="$(grep -Fc -- "test ${model_test} ... " "$log" || true)"
        if [[ "$leg_name" == "$ENGINE_MODEL_TEST_LEG" ]]; then
            if [[ "$runs" != 1 ]] || ! grep -Fxq -- "test ${model_test} ... ok" "$log"; then
                printf '%b✗ engine model test %s must pass exactly once in this leg (ran %s times)%b\n' \
                    "$RED" "$model_test" "$runs" "$NC"
                verdict=1
            fi
        elif [[ "$runs" != 0 ]]; then
            printf '%b✗ engine model test %s ran here; it belongs to "%s" only%b\n' \
                "$RED" "$model_test" "$ENGINE_MODEL_TEST_LEG" "$NC"
            verdict=1
        fi
    done
    return "$verdict"
}

# `cargo test` on the leg called `$1`; see `test-leg` in the header.
test_leg() {
    local leg_name="$1" leg_command log model_test verdict=0
    local -a args libtest_args=(--color never)
    leg_command="$(leg_args "$leg_name")" || return 2
    read -r -a args <<<"$leg_command"
    if [[ "$leg_name" != "$ENGINE_MODEL_TEST_LEG" ]]; then
        libtest_args+=(--exact)
        for model_test in "${ENGINE_MODEL_TESTS[@]}"; do
            libtest_args+=(--skip "$model_test")
        done
    fi
    log="$(mktemp "${TMPDIR:-/tmp}/grafton-leg.XXXXXX")"
    cargo_stable test "${args[@]}" -- "${libtest_args[@]}" 2>&1 | tee "$log" || verdict=1
    if ((verdict == 0)); then
        check_engine_model_tests "$leg_name" "$log" || verdict=1
    fi
    rm -f "$log"
    return "$verdict"
}

if [[ "$MODE" == test-leg ]]; then
    [[ $# -eq 2 ]] || usage
    test_leg "$2"
    exit 0
fi

run_reexported_range_helper_test() (
    local export_dir
    export_dir="$(mktemp -d "${TMPDIR:-/tmp}/grafton-range-reexport.XXXXXX")"
    trap 'find "$export_dir" -depth -delete' EXIT

    TS_RS_EXPORT_DIR="$export_dir" cargo_stable test \
        --manifest-path tests/fixtures/range_type_reexport/Cargo.toml \
        -p range-macro-consumer --features range-helper-derives &&
        test -s "${export_dir}/ReexportedRange.ts" &&
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

# Removed feature names must remain unknown rather than silently selecting a
# second implementation, and `dyn-api` alone must be rejected rather than
# compile to an empty feature. These live-compiler checks run in `rejections`
# mode (CI runs it as its own step) and in the full `test` run; the manifest
# side of the removed names is also pinned by
# `ecosystem_feature_inventory_matches_cargo_manifest` in
# `tests/issue_548_supported_surface_inventory.rs`.
run_rejections() {
    expect_unknown_feature "mode-async" &&
        expect_unknown_feature "async-core" &&
        expect_unknown_feature "mode-blocking" &&
        expect_facade_required "dyn-api"
}

if [[ "$MODE" == rejections ]]; then
    run_rejections
    exit 0
fi

# The queued jobs of this run: a description and a shell-quoted command each.
JOB_NAMES=()
JOB_COMMANDS=()

add_job() {
    local name="$1"
    shift
    JOB_NAMES+=("$name")
    JOB_COMMANDS+=("$(printf '%q ' "$@")")
}

# One parallel job: its own target and temporary directories, its output in
# `<log dir>/<id>.log`, and its verdict and wall time in `<id>.status`.
run_job() {
    local id="$1" name="$2" command="$3" target_dir="$4" log_dir="$5"
    local started=$SECONDS verdict=PASS
    mkdir -p "$target_dir/tmp"
    if ! (
        export CARGO_TARGET_DIR="$target_dir" TMPDIR="$target_dir/tmp"
        eval "$command"
    ) >"$log_dir/$id.log" 2>&1; then
        verdict=FAIL
    fi
    printf '%s %5ds  %s\n' "$verdict" "$((SECONDS - started))" "$name" >"$log_dir/$id.status"
}

# Runs the queued jobs: one after another in the caller's target directory,
# or `JOBS` at a time (see `--jobs` in the header), then summarizes.
run_jobs() {
    local i
    if ((JOBS == 0)); then
        for i in "${!JOB_NAMES[@]}"; do
            eval "run_test \"\${JOB_NAMES[i]}\" ${JOB_COMMANDS[i]}"
        done
        return 0
    fi

    local target_base="${MATRIX_TARGET_DIR:-${CARGO_TARGET_DIR:-$PWD/target}/matrix}"
    local log_dir="${MATRIX_LOG_DIR:-$target_base/logs}/$MODE"
    local id running=0 started=$SECONDS
    mkdir -p "$log_dir"
    rm -f "$log_dir"/*.log "$log_dir"/*.status "$log_dir/summary.txt"
    printf 'Running %d %s jobs, %d at a time; logs in %s\n' \
        "${#JOB_NAMES[@]}" "$MODE" "$JOBS" "$log_dir"
    for i in "${!JOB_NAMES[@]}"; do
        if ((running >= JOBS)); then
            wait -n || true
            running=$((running - 1))
        fi
        id="$(printf '%02d-%s' "$i" "${JOB_NAMES[i]//[^[:alnum:]]/_}")"
        run_job "$id" "${JOB_NAMES[i]}" "${JOB_COMMANDS[i]}" \
            "$target_base/$id" "$log_dir" &
        running=$((running + 1))
    done
    wait

    local failed=0 status_file
    for i in "${!JOB_NAMES[@]}"; do
        id="$(printf '%02d-%s' "$i" "${JOB_NAMES[i]//[^[:alnum:]]/_}")"
        status_file="$log_dir/$id.status"
        if [[ -s "$status_file" ]]; then
            cat "$status_file"
        else
            printf 'FAIL     ?s  %s (no verdict recorded)\n' "${JOB_NAMES[i]}"
        fi
    done >"$log_dir/summary.txt"
    # libtest warns about any single test still running after 60 seconds;
    # list those so a budget regression is visible without failing a run
    # whose wall time depends on machine load.
    grep -H 'has been running for over 60 seconds' "$log_dir"/*.log \
        >>"$log_dir/summary.txt" || true
    printf 'TOTAL %5ds  %s (%d jobs, %d at a time)\n' \
        "$((SECONDS - started))" "$MODE" "${#JOB_NAMES[@]}" "$JOBS" >>"$log_dir/summary.txt"
    cat "$log_dir/summary.txt"
    failed="$(grep -c '^FAIL' "$log_dir/summary.txt" || true)"
    if ((failed > 0)); then
        printf '%b✗ %d %s job(s) failed; see %s%b\n' "$RED" "$failed" "$MODE" "$log_dir" "$NC"
        return 1
    fi
}

finish() {
    echo "=========================================="
    printf '%b%s%b\n' "$GREEN" "$1" "$NC"
    echo "=========================================="
}

case "$MODE" in
doc)
    for leg in "${FEATURE_LEGS[@]}"; do
        doc="$(doc_args "${leg#*|}")" || continue
        read -r -a args <<<"$doc"
        add_job "${leg%%|*} (doc)" env RUSTDOCFLAGS="-D warnings" \
            cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" doc --no-deps "${args[@]}"
    done
    add_job "all-features workspace (doc)" env RUSTDOCFLAGS="-D warnings" \
        cargo "+${GRAFTON_VISCA_STABLE_TOOLCHAIN}" doc --no-deps --workspace --all-features
    run_jobs
    finish "Canonical 2.0 feature-shape rustdoc passed"
    ;;
clippy)
    for leg in "${FEATURE_LEGS[@]}"; do
        read -r -a args <<<"${leg#*|}"
        add_job "${leg%%|*} (clippy)" cargo_stable clippy "${args[@]}" -- -D warnings
    done
    add_job "all-features workspace (clippy)" \
        cargo_stable clippy --workspace --all-features --all-targets -- -D warnings
    run_jobs
    finish "Canonical 2.0 feature-shape lint passed"
    ;;
test)
    echo "=========================================="
    echo "Starting canonical 2.0 feature tests"
    echo "=========================================="
    add_job "removed and facade-less feature rejections" run_rejections
    for leg in "${FEATURE_LEGS[@]}"; do
        add_job "${leg%%|*} (test)" test_leg "${leg%%|*}"
    done
    add_job "native blocking dependency boundary" \
        bash .github/scripts/check-blocking-dependency-boundary.sh
    add_job "re-exported range macro without helpers" \
        cargo_stable test --manifest-path tests/fixtures/range_type_reexport/Cargo.toml \
        -p range-macro-consumer
    add_job "re-exported range macro with helpers" run_reexported_range_helper_test
    add_job "all-features and all-targets check" \
        cargo_stable check --workspace --all-features --all-targets
    add_job "blocking examples" \
        cargo_stable check --examples --no-default-features --features blocking
    add_job "Tokio examples" \
        cargo_stable check --examples --no-default-features --features runtime-tokio
    add_job "smol examples" \
        cargo_stable check --examples --no-default-features --features runtime-smol
    add_job "all-feature examples" cargo_stable check --examples --all-features
    add_job "no-default doctests" cargo_stable test --doc --no-default-features
    add_job "async doctests" cargo_stable test --doc --no-default-features --features async
    add_job "all-feature doctests" cargo_stable test --doc --all-features
    run_jobs
    finish "Canonical 2.0 feature matrix passed"
    ;;
esac
