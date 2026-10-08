#!/usr/bin/env bash

# Omarchy Tidal Music Player Plugin Installer
# Installs the Tidal streaming service and top bar widget.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_PLUGIN_DIR="$HOME/.config/omarchy/plugins/jaj.tidal"

echo "==> Installing Tidal Omarchy Plugin..."

# The tracked bundle makes installation independent of Cargo and build caches.
"$SCRIPT_DIR/scripts/verify-size.sh"

# 1. Stage a standalone plugin; Omarchy rejects symlinked plugin roots.
if [[ "$SCRIPT_DIR" != "$(realpath -m "$TARGET_PLUGIN_DIR")" || -L "$TARGET_PLUGIN_DIR" ]]; then
  STAGING_DIR="$(mktemp -d)"
  trap 'rm -rf "$STAGING_DIR"' EXIT
  tar -C "$SCRIPT_DIR" --exclude='./.git' --exclude='./backend/target' -cf - . \
    | tar -C "$STAGING_DIR" -xf -
  if command -v omarchy >/dev/null 2>&1; then
    omarchy plugin validate "$STAGING_DIR"
  fi
  if [[ -L "$TARGET_PLUGIN_DIR" ]]; then
    rm "$TARGET_PLUGIN_DIR"
  fi
  mkdir -p "$TARGET_PLUGIN_DIR"
  cp -a "$STAGING_DIR/." "$TARGET_PLUGIN_DIR/"
fi
echo "  [✓] Installed standalone plugin to $TARGET_PLUGIN_DIR"
cmp "$SCRIPT_DIR/bin/tidal-daemon" "$TARGET_PLUGIN_DIR/bin/tidal-daemon"
echo "  [✓] Bundled daemon installed; no Rust toolchain or build step required"

# 2. Validate Plugin
if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$TARGET_PLUGIN_DIR"
  echo "  [✓] Plugin validation passed"
else
  echo "  [i] Omarchy CLI unavailable; plugin validation skipped"
fi

# 3. Check shell.json layout
SHELL_CONFIG="$HOME/.config/omarchy/shell.json"
if [[ -f "$SHELL_CONFIG" ]]; then
  if ! grep -q "jaj.tidal" "$SHELL_CONFIG"; then
    echo "  [i] Note: Add \"jaj.tidal\" to your bar layout in $SHELL_CONFIG"
    echo "      e.g. In \"right\": [ ..., { \"id\": \"jaj.tidal\" } ]"
    echo "      Or run: omarchy plugin enable jaj.tidal"
  else
    echo "  [✓] Plugin already present in $SHELL_CONFIG"
  fi
fi

# 4. Trigger rescan
if command -v omarchy-shell >/dev/null 2>&1; then
  omarchy-shell shell rescanPlugins
  echo "  [✓] Triggered shell plugin rescan"
fi

echo "==> Installation complete! After source changes or a backend rebuild, run ./install.sh again."
