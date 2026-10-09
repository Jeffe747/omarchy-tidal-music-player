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
# The release profile uses panic=abort, so unwind tables are not needed at runtime.
strip --strip-all --remove-section=.comment --remove-section='.note*' \
  --remove-section=.eh_frame --remove-section=.eh_frame_hdr "$TARGET_BIN"

# Compression is opt-in so local development and diagnostics keep a normal ELF.
# Set TIDAL_UPX=1 to package an UPX executable when UPX is installed.
if [[ "${TIDAL_UPX:-0}" == "1" ]]; then
  if ! command -v upx >/dev/null 2>&1; then
    echo "TIDAL_UPX=1 requires upx on PATH." >&2
    exit 1
  fi
  upx --best --lzma "$TARGET_BIN"
fi

install -Dm755 "$TARGET_BIN" "$BUNDLED_BIN"
"$SCRIPT_DIR/scripts/verify-size.sh"

SIZE=$(du -h "$BUNDLED_BIN" | cut -f1)
echo "==> Build successful! Bundled binary size: $SIZE ($BUNDLED_BIN)"
