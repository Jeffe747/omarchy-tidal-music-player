#!/usr/bin/env bash

# Build script for tidal-daemon with size minimization
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$SCRIPT_DIR/backend"

echo "==> Building tidal-daemon with size optimizations..."

if ! command -v cargo >/dev/null 2>&1; then
  echo "Cargo not found. Please install Rust via 'omarchy pkg add rust' or rustup."
  exit 1
fi

cd "$BACKEND_DIR"
cargo build --release

TARGET_BIN="$BACKEND_DIR/target/release/tidal-daemon"

if [[ -f "$TARGET_BIN" ]]; then
  echo "==> Stripping binary..."
  strip --strip-all --remove-section=.comment --remove-section=.note* "$TARGET_BIN" 2>/dev/null || true

  SIZE=$(du -h "$TARGET_BIN" | cut -f1)
  echo "==> Build successful! Binary size: $SIZE ($TARGET_BIN)"

  if command -v upx >/dev/null 2>&1; then
    echo "  [i] UPX available. Run 'upx --best --lzma $TARGET_BIN' for sub-megabyte compression."
  fi
fi
