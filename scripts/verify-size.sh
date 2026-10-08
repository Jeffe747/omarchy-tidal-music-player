#!/usr/bin/env bash

# Binary size and symbol stripping verification
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="$SCRIPT_DIR/bin/tidal-daemon"
RELEASE_BIN="$SCRIPT_DIR/backend/target/release/tidal-daemon"
MAX_SIZE_BYTES=1800000 # 1.8 MB bundled binary limit (decimal bytes)

echo "==> Verifying binary size & symbols for tidal-daemon..."

verify_binary() {
  local path="$1" max_bytes="$2" size_bytes description

  if [[ ! -f "$path" || ! -x "$path" ]]; then
    echo "  [FAIL] Binary must exist and be executable: $path. Run ./scripts/build.sh." >&2
    return 1
  fi

  size_bytes=$(stat -c%s "$path")
  description=$(file -b "$path")
  echo "  $path: $size_bytes bytes"

  if [[ "$description" != ELF* || "$description" != *", stripped"* \
      || "$description" == *"not stripped"* || "$description" == *"with debug_info"* ]]; then
    echo "  [FAIL] Binary must be a stripped ELF executable: $description" >&2
    return 1
  fi

  if (( size_bytes > max_bytes )); then
    echo "  [FAIL] Binary size ($size_bytes bytes) exceeds budget ($max_bytes bytes)!" >&2
    return 1
  fi
  echo "  [PASS] Executable, stripped ELF within $max_bytes-byte budget."
}

verify_binary "$BIN_PATH" "$MAX_SIZE_BYTES"

# A fresh plugin clone needs no Cargo build artifacts.
if [[ -f "$RELEASE_BIN" ]]; then
  verify_binary "$RELEASE_BIN" "$((2560 * 1024))"
  if ! cmp -s "$BIN_PATH" "$RELEASE_BIN"; then
    echo "  [FAIL] Bundled daemon differs from the release build. Run ./scripts/build.sh." >&2
    exit 1
  fi
  echo "  [PASS] Bundled daemon matches the release build."
fi

echo "==> Binary verification passed successfully."
