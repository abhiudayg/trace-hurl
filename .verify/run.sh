#!/usr/bin/env bash
set -euo pipefail

echo "========================================================"
echo "⚡ Running TraceHurl Comprehensive Verification Suite"
echo "========================================================"

echo "1. Checking code formatting..."
cargo fmt -- --check

echo "2. Running Clippy linting (-D warnings)..."
cargo clippy -- -D warnings

echo "3. Running Cargo unit & integration tests..."
cargo test --verbose

echo "4. Building release binary..."
cargo build --release

echo "5. Generating report on sample run fixture..."
./target/release/tracehurl report .verify/fixtures/sample_run

echo "6. Running functional report verification..."
node tests/verify_report_functional.js

echo "7. Running isolated environment verification..."
node tests/test_isolated_run.js

echo "8. Validating manifest cryptographic integrity..."
./target/release/tracehurl validate .verify/fixtures/sample_run

echo "========================================================"
echo "✅ All TraceHurl verification checks passed successfully!"
echo "========================================================"
