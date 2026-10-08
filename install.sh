#!/usr/bin/env bash

# Omarchy Tidal Music Player Plugin Installer
# Installs the Tidal streaming service and top bar widget.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_PLUGIN_DIR="$HOME/.config/omarchy/plugins/jaj.tidal"

echo "==> Installing Tidal Omarchy Plugin..."

# 1. Install Plugin symlink
mkdir -p "$(dirname "$TARGET_PLUGIN_DIR")"
if [[ -L "$TARGET_PLUGIN_DIR" ]] || [[ -d "$TARGET_PLUGIN_DIR" ]]; then
  rm -rf "$TARGET_PLUGIN_DIR"
fi
ln -sf "$SCRIPT_DIR" "$TARGET_PLUGIN_DIR"
echo "  [✓] Linked plugin to $TARGET_PLUGIN_DIR"

# 2. Validate Plugin
if command -v omarchy >/dev/null 2>&1; then
  omarchy plugin validate "$TARGET_PLUGIN_DIR" 2>/dev/null || true
  echo "  [✓] Plugin validation checked"
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
  omarchy-shell shell rescanPlugins 2>/dev/null || true
  echo "  [✓] Triggered shell plugin rescan"
fi

echo "==> Installation complete! Build backend with ./scripts/build.sh when ready."
