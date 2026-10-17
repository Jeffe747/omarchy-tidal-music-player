#!/usr/bin/env bash

# Binary size and symbol stripping verification
set -euo pipefail

[[ -x "$HOME/.local/bin/upx" ]] && export PATH="$HOME/.local/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_PATH="$SCRIPT_DIR/bin/tidal-daemon"
RELEASE_BIN="$SCRIPT_DIR/backend/target/release/tidal-daemon"
MAX_SIZE_BYTES=$((1800 * 1024)) # 1.8 MB bundled binary limit (1800 KiB = 1,843,200 bytes)
UPX_MAX_SIZE_BYTES=800000

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

  if command -v upx >/dev/null 2>&1 && upx -t "$path" >/dev/null 2>&1; then
    if (( size_bytes >= UPX_MAX_SIZE_BYTES )); then
      echo "  [FAIL] UPX binary is not below ${UPX_MAX_SIZE_BYTES} bytes." >&2
      return 1
    fi
    echo "  [PASS] Valid UPX executable below ${UPX_MAX_SIZE_BYTES}-byte budget."
    return 0
  fi

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
    canonical_digest=$(awk '$2 == "tidal-daemon-x86_64" { print $1; exit }' "$SCRIPT_DIR/SHA256SUMS")
    bundled_digest=$(sha256sum "$BIN_PATH" | awk '{ print $1 }')
    if [[ -z "$canonical_digest" || "$bundled_digest" != "$canonical_digest" ]]; then
      echo "  [FAIL] Bundled daemon differs from the release build and canonical release digest. Run ./scripts/build.sh." >&2
      exit 1
    fi
    echo "  [PASS] Bundled daemon matches the canonical x86_64 release digest; local Cargo build differs."
  else
    echo "  [PASS] Bundled daemon matches the release build."
  fi
fi

echo "==> Binary verification passed successfully."
