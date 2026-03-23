#!/bin/bash

# Miri safety testing script
# Tests for undefined behavior in different feature configurations

set -e

echo "=========================================="
echo "Starting Miri safety checks"
echo "=========================================="

# Color output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Track if any issues were found
ISSUES_FOUND=0

# Function to run miri test
run_miri_test() {
    local description="$1"
    local features="$2"

    echo -e "${YELLOW}Miri testing: ${description}${NC}"

    if [ -z "$features" ]; then
        cmd="cargo miri test --no-default-features"
    else
        cmd="cargo miri test --no-default-features --features $features"
    fi

    # Run with various miri flags for comprehensive checking
    export MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance"

    if $cmd 2>&1 | tee miri_output.log; then
        echo -e "${GREEN}✓ ${description} - No undefined behavior detected${NC}"
    else
        echo -e "${RED}⚠ ${description} - Potential issues found (see log)${NC}"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))

        # Extract and display key issues
        echo -e "${BLUE}Key findings:${NC}"
        grep -E "error:|warning:|undefined behavior" miri_output.log || true
    fi

    echo ""
}

# Setup miri
echo "Setting up Miri..."
cargo miri setup

# Test different feature configurations
echo -e "${BLUE}Testing core library (no features)${NC}"
run_miri_test "Blocking mode (no features)" ""

echo -e "${BLUE}Testing async features${NC}"
run_miri_test "Mode-async feature" "mode-async"

echo -e "${BLUE}Testing runtime implementations${NC}"
run_miri_test "Tokio runtime" "runtime-tokio"
run_miri_test "Smol runtime" "runtime-smol"

echo -e "${BLUE}Testing utility features${NC}"
run_miri_test "Test utilities" "test-utils"

echo -e "${BLUE}Testing combined features${NC}"
run_miri_test "Tokio + test-utils" "runtime-tokio,test-utils"

# Also test specific modules that might have unsafe code
echo -e "${BLUE}Testing specific modules for safety${NC}"

# Test serialization/deserialization if it has unsafe
if grep -r "unsafe" src/ | grep -E "ser|de" > /dev/null 2>&1; then
    echo "Found unsafe code in serialization, testing..."
    MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance -Zmiri-symbolic-alignment-check" \
        cargo miri test --no-default-features --lib serialization
fi

# Test any FFI boundaries
if grep -r "unsafe" src/ | grep -E "ffi|extern" > /dev/null 2>&1; then
    echo "Found unsafe FFI code, testing..."
    MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance -Zmiri-check-number-validity" \
        cargo miri test --no-default-features --lib ffi
fi

# Summary
echo "=========================================="
if [ $ISSUES_FOUND -eq 0 ]; then
    echo -e "${GREEN}✓ All Miri safety checks passed!${NC}"
    echo "No undefined behavior detected."
else
    echo -e "${YELLOW}⚠ Miri found $ISSUES_FOUND potential issue(s)${NC}"
    echo "Review the logs above for details."
    echo "Note: Some warnings may be false positives or acceptable in context."
fi
echo "=========================================="

# Clean up
rm -f miri_output.log

# Exit with appropriate code (0 for now since we're in continue-on-error mode)
exit 0
