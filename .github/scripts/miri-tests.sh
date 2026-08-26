#!/bin/bash

# Miri safety testing script
# Tests for undefined behavior in different feature configurations.
#
# This script is a real gate: any Miri failure makes it exit non-zero. Do not
# reintroduce an unconditional `exit 0` — a Miri job that cannot fail reports
# safety it has not checked.
#
# Scope: library tests only, matching the `miri` job in ci.yml. The integration
# tests intentionally exercise real socket I/O and OS resolution paths, which
# belong in the normal CI matrix rather than Miri's isolation model.

set -euo pipefail

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
FAILED_CONFIGS=()

LOG_DIR="$(mktemp -d)"
trap 'rm -rf "$LOG_DIR"' EXIT

# Function to run miri test
run_miri_test() {
    local description="$1"
    local features="$2"
    local log_file
    log_file="$LOG_DIR/$(echo "$description" | tr -cs '[:alnum:]' '-').log"

    echo -e "${YELLOW}Miri testing: ${description}${NC}"

    local -a cmd
    if [ -z "$features" ]; then
        cmd=(cargo miri test --lib --no-default-features)
    else
        cmd=(cargo miri test --lib --no-default-features --features "$features")
    fi

    # Run with miri flags for comprehensive checking.
    export MIRIFLAGS="-Zmiri-disable-isolation -Zmiri-strict-provenance"

    # `set -o pipefail` makes the pipeline report the cargo exit status rather
    # than tee's, so a Miri failure is not swallowed by the log pipe.
    if "${cmd[@]}" 2>&1 | tee "$log_file"; then
        echo -e "${GREEN}OK ${description} - no undefined behavior detected${NC}"
    else
        echo -e "${RED}FAIL ${description} - see findings below${NC}"
        ISSUES_FOUND=$((ISSUES_FOUND + 1))
        FAILED_CONFIGS+=("$description")

        echo -e "${BLUE}Key findings:${NC}"
        grep -E "error:|undefined behavior" "$log_file" || true
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

# Summary
echo "=========================================="
if [ "$ISSUES_FOUND" -eq 0 ]; then
    echo -e "${GREEN}All Miri safety checks passed.${NC}"
    echo "=========================================="
    exit 0
fi

echo -e "${RED}Miri failed in $ISSUES_FOUND configuration(s):${NC}"
for config in "${FAILED_CONFIGS[@]}"; do
    echo "  - $config"
done
echo "=========================================="
exit 1
