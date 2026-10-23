#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-all}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${ROOT_DIR}"

mkdir -p .verify/artifacts

echo "========================================================"
echo "⚡ [agstack] Running TraceHurl Verification Suite (Target: ${TARGET})"
echo "========================================================"

# Phase 1: Doctor
echo "--> Phase 1: Doctor environment preflight..."
"${SCRIPT_DIR}/doctor.sh"

# Phase 2: Static Checks
echo "--> Phase 2: Static analysis (Format & Clippy)..."
cargo fmt -- --check
cargo clippy -- -D warnings

# Phase 3: Unit Checks
echo "--> Phase 3: Unit & integration test suites..."
cargo test --verbose

# Phase 4: Functional Verification
echo "--> Phase 4: Functional report & artifact validation..."
cargo build --release

echo "  [1/4] Generating report to .verify/artifacts/report..."
./target/release/tracehurl report verify/fixtures/sample_run --output .verify/artifacts/report

echo "  [2/4] Running functional DOM & JSON assertions..."
node tests/verify_report_functional.js .verify/artifacts/report/index.html

echo "  [3/4] Running isolated environment verification..."
node tests/test_isolated_run.js

echo "  [4/4] Validating manifest cryptographic integrity..."
./target/release/tracehurl validate verify/fixtures/sample_run

echo "========================================================"
echo "✅ All TraceHurl verification checks passed successfully!"
echo "========================================================"
