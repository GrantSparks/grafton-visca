#!/bin/bash

# 1.x support-matrix testing script.
#
# Keep this list in sync with README.md and CONTRIBUTING.md. Entries below are
# part of the documented support contract unless they are explicitly labelled
# compatibility-only.

set -e

echo "=========================================="
echo "Starting comprehensive feature tests"
echo "=========================================="

# Color output for better readability
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to run a test with nice output
run_test() {
    local description="$1"
    local command="$2"

    echo -e "${YELLOW}Testing: ${description}${NC}"
    if eval "$command"; then
        echo -e "${GREEN}✓ ${description} passed${NC}"
    else
        echo -e "${RED}✗ ${description} failed${NC}"
        exit 1
    fi
    echo ""
}

run_expected_unknown_feature() {
    local description="$1"
    local command="$2"
    local output
    local status

    echo -e "${YELLOW}Testing expected failure: ${description}${NC}"
    set +e
    output=$(eval "$command" 2>&1)
    status=$?
    set -e

    if [ "$status" -eq 0 ]; then
        echo -e "${RED}✗ ${description} unexpectedly succeeded${NC}"
        exit 1
    fi

    case "$output" in
        *"does not contain this feature: mode-blocking"*)
            echo -e "${GREEN}✓ ${description} failed with Cargo's unknown-feature error${NC}"
            ;;
        *)
            echo "$output"
            echo -e "${RED}✗ ${description} failed for the wrong reason${NC}"
            exit 1
            ;;
    esac
    echo ""
}

clean_target_checkpoint() {
    local description="$1"

    if [ "${GITHUB_ACTIONS:-}" != "true" ]; then
        return 0
    fi

    echo -e "${YELLOW}Cleaning cargo target after ${description} to keep CI disk usage bounded${NC}"
    du -sh target 2>/dev/null || true
    cargo clean --quiet
    echo ""
}

# Test default features
run_test "default features" \
    "cargo test"

# Test no default features (blocking mode)
run_test "no default features (blocking mode)" \
    "cargo test --no-default-features"

# Assert the removed blocking marker feature stays removed. These should fail
# with Cargo's unknown-feature error rather than compiling any public surface.
run_expected_unknown_feature "mode-blocking feature is rejected" \
    "cargo check --no-default-features --features mode-blocking"

run_expected_unknown_feature "mode-async + mode-blocking is rejected" \
    "cargo check --no-default-features --features mode-async,mode-blocking"

run_expected_unknown_feature "runtime-tokio + mode-blocking is rejected" \
    "cargo check --no-default-features --features runtime-tokio,mode-blocking"

# Test mode-async feature (runtime-agnostic)
run_test "mode-async feature (runtime-agnostic)" \
    "cargo test --no-default-features --features mode-async"

# Test individual runtime features
run_test "runtime-tokio runtime" \
    "cargo test --no-default-features --features runtime-tokio"

run_test "runtime-smol runtime" \
    "cargo test --no-default-features --features runtime-smol"

# Test test-utils feature
run_test "test-utils feature" \
    "cargo test --no-default-features --features test-utils"

# Test runtime + test-utils combinations
run_test "runtime-tokio + test-utils" \
    "cargo test --no-default-features --features runtime-tokio,test-utils"

run_test "runtime-smol + test-utils" \
    "cargo test --no-default-features --features runtime-smol,test-utils"

# Operation-handle release gates (#539). Keep these explicit so a cfg change
# cannot silently turn a runtime's semantic suite into zero executed tests.
run_test "#539 blocking operation handles" \
    "cargo test --no-default-features --features test-utils --test issue_539_blocking_handle_test"

run_test "#539 runtime-agnostic async contracts" \
    "cargo test --no-default-features --features mode-async,test-utils --test issue_539_async_handle_test"

run_test "#539 Tokio operation handles" \
    "cargo test --no-default-features --features runtime-tokio,test-utils --test issue_539_async_handle_test"

run_test "#539 smol operation handles" \
    "cargo test --no-default-features --features runtime-smol,test-utils --test issue_539_async_handle_test"

clean_target_checkpoint "runtime/test-utils feature group"

# Test transport features
run_test "blocking serial transport" \
    "cargo test --no-default-features --features transport-serial"

run_test "runtime-tokio + transport-serial compatibility-only union" \
    "cargo test --no-default-features --features runtime-tokio,transport-serial"

run_test "runtime-tokio + transport-serial-tokio" \
    "cargo test --no-default-features --features runtime-tokio,transport-serial-tokio"

clean_target_checkpoint "transport feature group"

# Test optional public contract features
run_test "serde + schemars + ts-rs" \
    "cargo test --no-default-features --features serde,schemars,ts-rs"

run_test "runtime-tokio + dyn-api + test-utils" \
    "cargo test --no-default-features --features runtime-tokio,dyn-api,test-utils --test dyn_api_integration_test"

run_test "runtime-smol + dyn-api compatibility-only union" \
    "cargo test --no-default-features --features runtime-smol,dyn-api"

run_test "runtime-smol + dyn-api + test-utils integration" \
    "cargo test --no-default-features --features runtime-smol,dyn-api,test-utils --test dyn_api_smol_integration_test"

# Test runtime coexistence explicitly.
run_test "tokio + smol runtime coexistence" \
    "cargo check --no-default-features --features runtime-tokio,runtime-smol"

# Test the macro crate
run_test "grafton-visca-macros crate" \
    "cargo test -p grafton-visca-macros"

clean_target_checkpoint "optional feature group"

# Build examples for different configurations
echo -e "${YELLOW}Building examples...${NC}"

run_test "blocking examples" \
    "cargo build --examples --no-default-features"

run_test "mode-async examples (runtime-agnostic)" \
    "cargo build --examples --no-default-features --features mode-async"

run_test "tokio examples" \
    "cargo build --examples --no-default-features --features runtime-tokio"

# Summary
echo "=========================================="
echo -e "${GREEN}All feature tests passed successfully!${NC}"
echo "=========================================="

# Optional: Show feature matrix coverage
echo ""
echo "Feature Matrix Coverage:"
echo "------------------------"
echo "✓ Default features"
echo "✓ Blocking mode (no features)"
echo "✓ Removed mode-blocking feature rejected"
echo "✓ Mode-async (runtime-agnostic)"
echo "✓ Tokio runtime"
echo "✓ Smol runtime"
echo "✓ Test utilities"
echo "✓ Runtime + test-utils combinations"
echo "✓ Blocking and Tokio serial transport features"
echo "✓ Serialization/schema/type-generation features"
echo "✓ Dyn-api feature"
echo "✓ Tokio + smol runtime coexistence"
echo "✓ Compatibility-only feature unions"
echo "✓ Macro crate"
echo "✓ Examples compilation"
