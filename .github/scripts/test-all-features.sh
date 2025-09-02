#!/bin/bash

# Comprehensive feature testing script
# This script tests all feature combinations to ensure compatibility

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

# Test default features
run_test "default features" \
    "cargo test --verbose"

# Test no default features (blocking mode)
run_test "no default features (blocking mode)" \
    "cargo test --no-default-features --verbose"

# Test async feature (runtime-agnostic)
run_test "async feature (runtime-agnostic)" \
    "cargo test --no-default-features --features async --verbose"

# Test individual runtime features
run_test "rt-tokio runtime" \
    "cargo test --no-default-features --features rt-tokio --verbose"

run_test "rt-async-std runtime" \
    "cargo test --no-default-features --features rt-async-std --verbose"

run_test "rt-smol runtime" \
    "cargo test --no-default-features --features rt-smol --verbose"

# Test test-utils feature
run_test "test-utils feature" \
    "cargo test --no-default-features --features test-utils --verbose"

# Test runtime + test-utils combinations
run_test "rt-tokio + test-utils" \
    "cargo test --no-default-features --features rt-tokio,test-utils --verbose"

run_test "rt-async-std + test-utils" \
    "cargo test --no-default-features --features rt-async-std,test-utils --verbose"

run_test "rt-smol + test-utils" \
    "cargo test --no-default-features --features rt-smol,test-utils --verbose"

# Test legacy compatibility
run_test "rt-tokio + serialport (legacy)" \
    "cargo test --no-default-features --features rt-tokio,serialport --verbose"

# Test the macro crate
run_test "grafton-visca-macros crate" \
    "cargo test -p grafton-visca-macros --verbose"

# Build examples for different configurations
echo -e "${YELLOW}Building examples...${NC}"

run_test "blocking examples" \
    "cargo build --examples --no-default-features"

run_test "async examples (runtime-agnostic)" \
    "cargo build --examples --no-default-features --features async"

run_test "tokio examples" \
    "cargo build --examples --no-default-features --features rt-tokio"

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
echo "✓ Async (runtime-agnostic)"
echo "✓ Tokio runtime"
echo "✓ Async-std runtime"
echo "✓ Smol runtime"
echo "✓ Test utilities"
echo "✓ Runtime + test-utils combinations"
echo "✓ Legacy compatibility"
echo "✓ Macro crate"
echo "✓ Examples compilation"
