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
  ACTIVE_M=$(grep -E "^\| \*\*M[0-9]\*\*.*🟡" "$PROGRESS_FILE" | head -n 1 | sed -E 's/.*(\*\*M[0-9]\*\*).*/\1/' | tr -d '*' || true)
  if [[ -z "$ACTIVE_M" ]]; then
    ACTIVE_M="Completed"
    echo -e "  ${GREEN}All planned milestone tasks currently completed.${NC}"
  else
    M_NUM="${ACTIVE_M#M}"
    echo -e "${BOLD}Next Immediate Tasks (Milestone ${M_NUM}):${NC}"
    grep -A 10 "### Milestone ${M_NUM}:" "$PROGRESS_FILE" | (grep "^- \[ \]" || true) | head -n 5 | while IFS= read -r task; do
      [[ -n "$task" ]] && echo -e "  ${YELLOW}${task}${NC}"
    done
  fi
else
  echo "PROGRESS.md not found."
fi

echo ""
echo -e "${GRAY}Run ./scripts/verify.sh to execute all compliance and unit tests.${NC}"
echo -e "${BOLD}${CYAN}==========================================================${NC}"
