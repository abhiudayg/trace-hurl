#!/usr/bin/env bash
set -euo pipefail

echo "==> [Doctor] Checking toolchain requirements..."

TOOLS=("cargo" "rustc" "node" "git")
MISSING=0

for tool in "${TOOLS[@]}"; do
  if command -v "$tool" >/dev/null 2>&1; then
    VERSION=$("$tool" --version 2>&1 | head -n 1)
    echo "  [OK] $tool: $VERSION"
  else
    echo "  [FAIL] Missing required tool: $tool"
    MISSING=1
  fi
done

if [ "$MISSING" -ne 0 ]; then
  echo "==> [Doctor] Environment verification failed. Please install missing tools."
  exit 2
fi

echo "==> [Doctor] All toolchain prerequisites satisfied."
exit 0
