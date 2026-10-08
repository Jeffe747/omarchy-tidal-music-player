#!/usr/bin/env bash

# Binary size and symbol stripping verification
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="$SCRIPT_DIR/backend/target/release/tidal-daemon"
MAX_SIZE_KB=2560 # 2.5 MB maximum budget
TARGET_SIZE_KB=1800 # 1.8 MB target

echo "==> Verifying binary size & symbols for tidal-daemon..."

if [[ ! -f "$BIN_PATH" ]]; then
  echo "Error: Binary not found at $BIN_PATH. Run ./scripts/build.sh first."
  exit 1
fi

# Measure size in KB
SIZE_BYTES=$(stat -c%s "$BIN_PATH" 2>/dev/null || stat -f%z "$BIN_PATH")
SIZE_KB=$((SIZE_BYTES / 1024))
SIZE_HUMAN=$(du -h "$BIN_PATH" | cut -f1)

echo "  Current binary size: ${SIZE_HUMAN} (${SIZE_KB} KB)"

# Verify debug symbols are stripped
if file "$BIN_PATH" | grep -q "with debug_info"; then
  echo "  [FAIL] Binary still contains debug symbols!"
  exit 1
else
  echo "  [PASS] Debug symbols stripped."
fi

# Check against maximum budget
if (( SIZE_KB > MAX_SIZE_KB )); then
  echo "  [FAIL] Binary size (${SIZE_KB} KB) exceeds maximum budget (${MAX_SIZE_KB} KB)!"
  exit 1
elif (( SIZE_KB > TARGET_SIZE_KB )); then
  echo "  [WARN] Binary size (${SIZE_KB} KB) passes budget but exceeds ideal target (${TARGET_SIZE_KB} KB)."
else
  echo "  [PASS] Binary size within ideal budget (< ${TARGET_SIZE_KB} KB)."
fi

echo "==> Binary verification passed successfully."
