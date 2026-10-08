#!/usr/bin/env bash

# Omarchy Official Plugin Compliance Verification
# Validates all security, schema, layout, and style requirements enforced by Omarchy.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="$SCRIPT_DIR/manifest.json"

FAILED=0

pass() {
  echo "    [PASS] $*"
}

fail() {
  echo "    [FAIL] $*" >&2
  FAILED=1
}

warn() {
  echo "    [WARN] $*" >&2
}

echo "==> Running Omarchy Official Plugin Compliance Audit..."

# -----------------------------------------------------------------------------
# 1. Manifest Existence and JSON Parsing
# -----------------------------------------------------------------------------
echo "--> 1. Checking manifest.json existence and JSON syntax..."
if [[ ! -f "$MANIFEST" ]]; then
  fail "manifest.json missing at $MANIFEST"
  exit 1
fi

if ! jq -e . "$MANIFEST" >/dev/null 2>&1; then
  fail "manifest.json contains invalid JSON syntax"
  exit 1
fi
pass "manifest.json exists and is valid JSON"

# -----------------------------------------------------------------------------
# 2. Schema Version Enforced by Omarchy PluginRegistry
# -----------------------------------------------------------------------------
echo "--> 2. Checking schemaVersion..."
if ! jq -e '.schemaVersion == 1' "$MANIFEST" >/dev/null 2>&1; then
  fail "schemaVersion must be exactly integer 1 (found: $(jq -r '.schemaVersion // "missing"' "$MANIFEST"))"
else
  pass "schemaVersion is 1 (integer)"
fi

# -----------------------------------------------------------------------------
# 3. Mandatory Fields Enforced by Shell PluginRegistry
# -----------------------------------------------------------------------------
echo "--> 3. Checking required manifest fields..."
for field in id name version kinds entryPoints; do
  if ! jq -e --arg f "$field" 'has($f)' "$MANIFEST" >/dev/null 2>&1; then
    fail "Manifest missing required field: '$field'"
  fi
done
pass "All required manifest fields present (id, name, version, kinds, entryPoints)"

# -----------------------------------------------------------------------------
# 4. Plugin ID Namespace and Character Set Rules
# -----------------------------------------------------------------------------
echo "--> 4. Checking plugin ID rules..."
ID=$(jq -r '.id // ""' "$MANIFEST")
if [[ -z "$ID" ]]; then
  fail "Plugin ID is empty"
elif [[ "$ID" == omarchy.* ]]; then
  fail "Third-party plugin ID '$ID' cannot use reserved 'omarchy.*' namespace"
elif [[ "$ID" == *".."* ]]; then
  fail "Plugin ID '$ID' contains invalid path traversal '..'"
elif ! [[ "$ID" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
  fail "Plugin ID '$ID' does not match allowed regex ^[A-Za-z0-9][A-Za-z0-9._-]*$"
else
  pass "Plugin ID '$ID' conforms to third-party namespace and naming conventions"
fi

# -----------------------------------------------------------------------------
# 5. Kinds & EntryPoints 1:1 Mapping Contract
# -----------------------------------------------------------------------------
echo "--> 5. Checking kinds and entryPoints mapping..."
if ! jq -e '(.kinds | type) == "array" and (.kinds | length) > 0' "$MANIFEST" >/dev/null 2>&1; then
  fail "'kinds' must be a non-empty array"
fi

if ! jq -e '(.entryPoints | type) == "object"' "$MANIFEST" >/dev/null 2>&1; then
  fail "'entryPoints' must be an object"
fi

declare -A KIND_TO_EP=(
  ["bar"]="bar"
  ["bar-widget"]="barWidget"
  ["menu"]="menu"
  ["overlay"]="overlay"
  ["panel"]="panel"
  ["service"]="service"
)

KINDS=$(jq -r '.kinds[]' "$MANIFEST")
for kind in $KINDS; do
  ep_key="${KIND_TO_EP[$kind]:-}"
  if [[ -z "$ep_key" ]]; then
    fail "Unknown kind declared: '$kind'"
    continue
  fi

  if ! jq -e --arg ep "$ep_key" '.entryPoints | has($ep)' "$MANIFEST" >/dev/null 2>&1; then
    fail "Kind '$kind' declared but missing matching 'entryPoints.$ep_key'"
  else
    ep_file=$(jq -r --arg ep "$ep_key" '.entryPoints[$ep]' "$MANIFEST")
    
    # Path safety
    if [[ "$ep_file" == /* ]]; then
      fail "Entry point '$ep_key' must be relative path, not absolute: '$ep_file'"
    elif [[ "$ep_file" == *".."* ]]; then
      fail "Entry point '$ep_key' cannot contain path traversal: '$ep_file'"
    elif [[ ! -f "$SCRIPT_DIR/$ep_file" ]]; then
      fail "Entry point file for '$ep_key' not found on disk: '$ep_file'"
    else
      pass "Kind '$kind' correctly mapped to existing file '$ep_file'"
    fi
  fi
done

# -----------------------------------------------------------------------------
# 6. Default Section Check
# -----------------------------------------------------------------------------
echo "--> 6. Checking barWidget defaultSection..."
if jq -e '.barWidget? | has("defaultSection")' "$MANIFEST" >/dev/null 2>&1; then
  DEFAULT_SEC=$(jq -r '.barWidget.defaultSection' "$MANIFEST")
  if [[ ! "$DEFAULT_SEC" =~ ^(left|center|right)$ ]]; then
    fail "barWidget.defaultSection must be 'left', 'center', or 'right' (got: '$DEFAULT_SEC')"
  else
    pass "barWidget.defaultSection is valid: '$DEFAULT_SEC'"
  fi
fi

# -----------------------------------------------------------------------------
# 7. Symlink Security Audit (Strictly Enforced by Omarchy)
# -----------------------------------------------------------------------------
echo "--> 7. Auditing for banned internal symlinks..."
# Omarchy refuses any symlinks inside a plugin directory (to prevent path traversal escapes)
SYMLINKS=$(find "$SCRIPT_DIR" -name .git -prune -o -type l -print 2>/dev/null || true)
if [[ -n "$SYMLINKS" ]]; then
  fail "Symlinks are strictly prohibited inside an official Omarchy plugin folder:
$SYMLINKS"
else
  pass "Zero internal symlinks detected (pure standalone tree)"
fi

# -----------------------------------------------------------------------------
# 8. Omarchy CLI Validation
# -----------------------------------------------------------------------------
echo "--> 8. Executing official omarchy-plugin-validate..."
if command -v omarchy-plugin-validate >/dev/null 2>&1; then
  if omarchy-plugin-validate "$SCRIPT_DIR"; then
    pass "Official 'omarchy-plugin-validate' passed"
  else
    fail "Official 'omarchy-plugin-validate' failed"
  fi
elif command -v omarchy >/dev/null 2>&1; then
  if omarchy plugin validate "$SCRIPT_DIR"; then
    pass "Official 'omarchy plugin validate' passed"
  else
    fail "Official 'omarchy plugin validate' failed"
  fi
else
  warn "'omarchy' CLI not found on system PATH; skipping runtime validator command"
fi

# -----------------------------------------------------------------------------
# 9. Omarchy Style & Dynamic Theming Compliance
# -----------------------------------------------------------------------------
echo "--> 9. Checking Omarchy UI styling and theming compliance..."
QML_FILES=$(find "$SCRIPT_DIR" -maxdepth 2 -name "*.qml")

# Must import qs.Commons and qs.Ui in bar/widget components
if grep -q "BarWidget" <<< "$QML_FILES"; then
  for qml in $QML_FILES; do
    if grep -q "BarIconButton\|Panel" "$qml" 2>/dev/null; then
      if ! grep -q "import qs.Commons" "$qml"; then
        fail "$qml uses Omarchy UI components but does not import qs.Commons"
      fi
      if ! grep -q "import qs.Ui" "$qml"; then
        fail "$qml uses Omarchy UI components but does not import qs.Ui"
      fi
    fi
  done
fi

# Prohibit hardcoded hex colors in QML surfaces
HEX_COLORS=$(grep -rnE '#[0-9a-fA-F]{3,8}' "$SCRIPT_DIR"/*.qml 2>/dev/null || true)
if [[ -n "$HEX_COLORS" ]]; then
  fail "Hardcoded hex colors detected in QML surfaces (must bind to qs.Commons.Color):
$HEX_COLORS"
else
  pass "All QML colors bind dynamically to qs.Commons.Color (zero hardcoded hex colors)"
fi

# Prohibit WidgetButton with onClicked in QML files (WidgetButton has pressed(int button); panel UI uses Button)
WIDGETBUTTON_ONCLICKED=$(python3 -c '
import sys, glob, os, re

script_dir = sys.argv[1]
found_errors = []
qml_files = glob.glob(os.path.join(script_dir, "**", "*.qml"), recursive=True)

for path in qml_files:
    try:
        with open(path, "r", encoding="utf-8") as f:
            content = f.read()
    except Exception:
        continue

    idx = 0
    while True:
        m = re.search(r"\bWidgetButton\b[^{]*\{", content[idx:])
        if not m:
            break
        start_pos = idx + m.start()
        brace_pos = idx + m.end() - 1
        depth = 1
        pos = brace_pos + 1
        while pos < len(content) and depth > 0:
            if content[pos] == "{":
                depth += 1
            elif content[pos] == "}":
                depth -= 1
            pos += 1
        block = content[brace_pos:pos]
        if "onClicked" in block:
            line_no = content[:start_pos].count("\n") + 1
            found_errors.append(f"{path}:{line_no}: WidgetButton cannot use onClicked signal (WidgetButton only has pressed(int button); use Button instead)")
        idx = pos

if found_errors:
    print("\n".join(found_errors))
    sys.exit(1)
' "$SCRIPT_DIR" 2>&1 || true)

if [[ -n "$WIDGETBUTTON_ONCLICKED" ]]; then
  fail "WidgetButton used with onClicked detected:
$WIDGETBUTTON_ONCLICKED"
else
  pass "No invalid WidgetButton onClicked usage in QML files"
fi

# -----------------------------------------------------------------------------
# 10. Bundled Daemon Deployment Contract
# -----------------------------------------------------------------------------
echo "--> 10. Checking bundled daemon (executable, stripped, <= 1.8 MB)..."
if "$SCRIPT_DIR/scripts/verify-size.sh"; then
  pass "bin/tidal-daemon is ready for installation without a Rust toolchain"
else
  fail "Bundled daemon verification failed"
fi

# -----------------------------------------------------------------------------
# 11. Summary
# -----------------------------------------------------------------------------
echo ""
if (( FAILED != 0 )); then
  echo "==> [FAILED] Omarchy compliance audit failed. Please address the errors above." >&2
  exit 1
else
  echo "==> [SUCCESS] Plugin satisfies 100% of official Omarchy plugin compliance rules!"
  exit 0
fi
