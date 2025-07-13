#!/bin/bash
set -e

echo "Running CI checks locally..."
echo

# Set environment variables from workflow
export CARGO_TERM_COLOR=always
export RUSTFLAGS="-D warnings"
export RUST_BACKTRACE=1

# 1. Format check
echo "=== Format Check ==="
cargo fmt --all -- --check
echo "✓ Format check passed"
echo

# 2. Test with default features
echo "=== Test with default features ==="
cargo test --verbose
echo "✓ Tests with default features passed"
echo

# 3. Test with no default features  
echo "=== Test with no default features ==="
cargo test --no-default-features --verbose
echo "✓ Tests with no default features passed"
echo

# 4. Test with all features
echo "=== Test with all features ==="
cargo test --all-features --verbose
echo "✓ Tests with all features passed"
echo

# 5. Test async feature only
echo "=== Test async feature only ==="
cargo test --no-default-features --features async --verbose
echo "✓ Tests with async feature passed"
echo

# 6. Test blocking mode
echo "=== Test blocking mode ==="
cargo test --no-default-features --verbose --tests
echo "✓ Blocking mode tests passed"
echo

# 7. Build examples with default features
echo "=== Build examples with default features ==="
cargo build --examples
echo "✓ Examples with default features built"
echo

# 8. Build examples with all features
echo "=== Build examples with all features ==="
cargo build --examples --all-features
echo "✓ Examples with all features built"
echo

# 9. Build examples with async only
echo "=== Build examples with async only ==="
cargo build --examples --no-default-features --features async
echo "✓ Examples with async only built"
echo

# 10. Clippy default features
echo "=== Clippy with default features ==="
cargo clippy --all-targets -- -D warnings
echo "✓ Clippy with default features passed"
echo

# 11. Clippy all features
echo "=== Clippy with all features ==="
cargo clippy --all-targets --all-features -- -D warnings
echo "✓ Clippy with all features passed"
echo

# 12. Clippy no default features
echo "=== Clippy with no default features ==="
cargo clippy --all-targets --no-default-features -- -D warnings
echo "✓ Clippy with no default features passed"
echo

# 13. Clippy examples
echo "=== Clippy examples ==="
cargo clippy --examples --all-features -- -D warnings
echo "✓ Clippy examples passed"
echo

# 14. Check documentation
echo "=== Check documentation ==="
cargo doc --no-deps --all-features
cargo doc --document-private-items --no-deps --all-features
echo "✓ Documentation check passed"
echo

# 15. Check doc links
echo "=== Check doc links ==="
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
echo "✓ Doc links check passed"
echo

echo "All CI checks passed!"