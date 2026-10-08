#!/usr/bin/env bash

# CLI Status Dashboard for Omarchy Tidal Music Player
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROGRESS_FILE="$SCRIPT_DIR/PROGRESS.md"

CYAN='\033[0;36m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
GRAY='\033[0;90m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo -e "${BOLD}${CYAN}==========================================================${NC}"
echo -e "${BOLD}   Omarchy Tidal Music Player - Status Dashboard${NC}"
echo -e "${BOLD}${CYAN}==========================================================${NC}"

if [[ -f "$PROGRESS_FILE" ]]; then
  CURRENT_STATUS=$(grep "^Current Status:" "$PROGRESS_FILE" | cut -d':' -f2- | sed 's/^[ *]*//;s/[ *]*$//')
  echo -e "${BOLD}Current Status:${NC} ${YELLOW}${CURRENT_STATUS}${NC}"
  echo ""
  echo -e "${BOLD}Milestone Overview:${NC}"
  # Print the markdown table lines
  grep -E "^\| \*\*[M0-7]" "$PROGRESS_FILE" | while IFS= read -r line; do
    echo -e "  ${line}"
  done
  echo ""
  echo -e "${BOLD}Next Immediate Tasks (Milestone 1):${NC}"
  grep -A 8 "### Milestone 1:" "$PROGRESS_FILE" | grep "^- \[ \]" | head -n 3 | while IFS= read -r task; do
    echo -e "  ${YELLOW}${task}${NC}"
  done
else
  echo "PROGRESS.md not found."
fi

echo ""
echo -e "${GRAY}Run ./scripts/verify.sh to execute all compliance and unit tests.${NC}"
echo -e "${BOLD}${CYAN}==========================================================${NC}"
