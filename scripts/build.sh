#!/usr/bin/env bash

# Build script for tidal-daemon with size minimization
set -euo pipefail

[[ -d "$HOME/.cargo/bin" ]] && export PATH="$HOME/.cargo/bin:$PATH"

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
BUNDLED_BIN="$SCRIPT_DIR/bin/tidal-daemon"

echo "==> Stripping binary..."
strip --strip-all --remove-section=.comment --remove-section='.note*' "$TARGET_BIN"

install -Dm755 "$TARGET_BIN" "$BUNDLED_BIN"
"$SCRIPT_DIR/scripts/verify-size.sh"

SIZE=$(du -h "$BUNDLED_BIN" | cut -f1)
echo "==> Build successful! Bundled binary size: $SIZE ($BUNDLED_BIN)"

if command -v upx >/dev/null 2>&1; then
  echo "  [i] UPX available. After optional compression, update bin/tidal-daemon and re-run verification."
fi
