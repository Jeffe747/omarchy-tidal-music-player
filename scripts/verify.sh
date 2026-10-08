#!/usr/bin/env bash

# Global test and verification suite for Omarchy Tidal Music Player
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "=========================================================="
echo "  Omarchy Tidal Music Player - Verification Suite"
echo "=========================================================="

# 1. Omarchy Official Compliance Audit
echo ""
echo "--> 1. Running Omarchy Official Compliance Audit..."
"$SCRIPT_DIR/scripts/verify-omarchy-compliance.sh"

# 2. Rust Unit Tests
echo ""
echo "--> 2. Running Rust unit test suite..."
if command -v cargo >/dev/null 2>&1; then
  (cd "$SCRIPT_DIR/backend" && cargo test)
  echo "    [PASS] All unit tests passed."
else
  echo "    [SKIP] 'cargo' not found on PATH."
fi

# 3. Binary Size & Stripping Audit
echo ""
echo "--> 3. Checking binary size and symbols..."
if [[ -f "$SCRIPT_DIR/backend/target/release/tidal-daemon" ]]; then
  "$SCRIPT_DIR/scripts/verify-size.sh"
else
  echo "    [SKIP] Release binary not built yet. Run ./scripts/build.sh to build."
fi

echo ""
echo "=========================================================="
echo "  All verification checks passed!"
echo "=========================================================="
