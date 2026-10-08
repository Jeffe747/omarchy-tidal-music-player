#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SHELL_DIR="/usr/share/omarchy/shell"

if ! command -v quickshell >/dev/null 2>&1 || [[ ! -d "$SHELL_DIR/Ui" ]] \
    || [[ -z "${WAYLAND_DISPLAY:-}" ]]; then
  echo "    [SKIP] QML runtime check requires Quickshell, Omarchy, and a Wayland session."
  exit 0
fi

TEST_DIR="$(mktemp -d)"
trap 'rm -rf "$TEST_DIR"' EXIT
cp "$SCRIPT_DIR/tests/qml/shell.qml" "$TEST_DIR/shell.qml"
ln -s "$SHELL_DIR/Ui" "$TEST_DIR/Ui"
ln -s "$SHELL_DIR/Commons" "$TEST_DIR/Commons"
mkdir -p "$TEST_DIR/plugin/bin" "$TEST_DIR/plugin/backend/target/release" "$TEST_DIR/runtime"
chmod 700 "$TEST_DIR/runtime"
cp "$SCRIPT_DIR/Service.qml" "$TEST_DIR/plugin/Service.qml"
cp "$SCRIPT_DIR/tests/qml/fake-daemon.py" "$TEST_DIR/plugin/backend/target/release/tidal-daemon"
chmod +x "$TEST_DIR/plugin/backend/target/release/tidal-daemon"

DISPLAY_PATH="$WAYLAND_DISPLAY"
if [[ "$DISPLAY_PATH" != /* ]]; then
  DISPLAY_PATH="$XDG_RUNTIME_DIR/$DISPLAY_PATH"
fi

for mode in bundled fallback nonexecutable missing; do
  expected_binary="$TEST_DIR/plugin/backend/target/release/tidal-daemon"
  case "$mode" in
    bundled)
      cp "$SCRIPT_DIR/tests/qml/fake-daemon.py" "$TEST_DIR/plugin/bin/tidal-daemon"
      chmod +x "$TEST_DIR/plugin/bin/tidal-daemon"
      expected_binary="$TEST_DIR/plugin/bin/tidal-daemon"
      ;;
    fallback)
      rm "$TEST_DIR/plugin/bin/tidal-daemon"
      ;;
    nonexecutable)
      cp "$SCRIPT_DIR/tests/qml/fake-daemon.py" "$TEST_DIR/plugin/bin/tidal-daemon"
      chmod -x "$TEST_DIR/plugin/bin/tidal-daemon"
      ;;
    missing)
      rm "$TEST_DIR/plugin/backend/target/release/tidal-daemon"
      expected_binary=""
      ;;
  esac
  rm -f "$TEST_DIR/runtime/tidal.sock"
  if ! QT_QPA_PLATFORM=wayland WAYLAND_DISPLAY="$DISPLAY_PATH" \
    XDG_RUNTIME_DIR="$TEST_DIR/runtime" TIDAL_PLUGIN_DIR="$SCRIPT_DIR" \
    TIDAL_TEST_SERVICE="$TEST_DIR/plugin/Service.qml" \
    TIDAL_TEST_EXPECTED_BINARY="$expected_binary" TIDAL_TEST_MODE="$mode" \
    timeout 15s quickshell --no-color -p "$TEST_DIR" >"$TEST_DIR/output.log" 2>&1; then
    cat "$TEST_DIR/output.log"
    exit 1
  fi

  if ! grep -q 'TIDAL_QML_TEST_PASS' "$TEST_DIR/output.log" \
    || grep -v 'QLocalSocket::ServerNotFoundError' "$TEST_DIR/output.log" \
      | grep -Eq 'WARN|ERROR|qml:.*Error|failed'; then
    cat "$TEST_DIR/output.log"
    exit 1
  fi

  echo "    [PASS] QML loading, bar sizing, shared service, and daemon resolution ($mode)."
done
