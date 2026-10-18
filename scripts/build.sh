#!/usr/bin/env bash

# Build script for tidal-daemon with size minimization
set -euo pipefail

[[ -d "$HOME/.cargo/bin" ]] && export PATH="$HOME/.cargo/bin:$PATH"
[[ -x "$HOME/.local/bin/upx" ]] && export PATH="$HOME/.local/bin:$PATH"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKEND_DIR="$SCRIPT_DIR/backend"

echo "==> Building tidal-daemon with size optimizations..."

if ! command -v cargo >/dev/null 2>&1; then
  echo "Cargo not found. Install Rust or use the tracked bin/tidal-daemon bundle for installation."
  exit 1
fi

cd "$BACKEND_DIR"
cargo build --release

TARGET_BIN="$BACKEND_DIR/target/release/tidal-daemon"
BUNDLED_BIN="$SCRIPT_DIR/bin/tidal-daemon"

echo "==> Stripping binary..."
# The release profile uses panic=abort, so unwind tables are not needed at runtime.
if file -b "$TARGET_BIN" | grep -q 'UPX compressed'; then
  upx -d "$TARGET_BIN"
fi
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

echo "==> Synchronizing release checksums and Service.qml pins..."
NEW_HASH=$(sha256sum "$BUNDLED_BIN" | awk '{ print $1 }')
SIZE_BYTES=$(stat -c%s "$BUNDLED_BIN")
SIZE_FORMATTED=$(printf "%'d bytes" "$SIZE_BYTES" 2>/dev/null || echo "$SIZE_BYTES bytes")

# 1. Update SHA256SUMS
sed -i -E "s/^[a-f0-9]{64}(  tidal-daemon-x86_64)$/${NEW_HASH}\1/" "$SCRIPT_DIR/SHA256SUMS"

# 2. Update Service.qml pins
sed -i -E "s/(x86_64\|amd64\) expected=\")[a-f0-9]{64}(\")/\1${NEW_HASH}\2/" "$SCRIPT_DIR/Service.qml"
sed -i -E "s/(asset=tidal-daemon-x86_64; expected=)[a-f0-9]{64}/\1${NEW_HASH}/" "$SCRIPT_DIR/Service.qml"

# 3. Update docs/PROVENANCE.md
if [[ -f "$SCRIPT_DIR/docs/PROVENANCE.md" ]]; then
  sed -i -E "s/(\|[[:space:]]*\`bin\/tidal-daemon\`[[:space:]]*\/[[:space:]]*\`tidal-daemon-x86_64\`[[:space:]]*\|[[:space:]]*)[^|]+(\|[[:space:]]*\`)[a-f0-9]{64}(\`[[:space:]]*\|)/\1${SIZE_FORMATTED} \2${NEW_HASH}\3/" "$SCRIPT_DIR/docs/PROVENANCE.md"
fi

"$SCRIPT_DIR/scripts/verify-size.sh"

SIZE=$(du -h "$BUNDLED_BIN" | cut -f1)
echo "==> Build successful! Bundled binary size: $SIZE ($BUNDLED_BIN)"
