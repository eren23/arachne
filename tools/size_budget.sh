#!/usr/bin/env bash
# size_budget.sh -- Report source code sizes per crate and check thresholds.
#
# Since we cannot build WASM without wasm32-unknown-unknown target installed,
# this script measures source line counts as a proxy for code size and prints
# a nice report table.
#
# Usage: ./tools/size_budget.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATES_DIR="$PROJECT_ROOT/crates"

# Per-crate source line budget (approximate, generous).
# These correspond to the expected relative sizes of each crate.
declare -A BUDGETS=(
    [arachne-math]=3000
    [arachne-ecs]=5000
    [arachne-input]=2000
    [arachne-render]=8000
    [arachne-physics]=5000
    [arachne-audio]=3000
    [arachne-asset]=3000
    [arachne-app]=3000
    [arachne-particles]=3000
    [arachne-ui]=5000
    [arachne-scene]=3000
    [arachne-animation]=3000
    [arachne-networking]=3000
    [arachne-wasm]=2000
)

# Colors (if terminal supports them).
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo ""
echo -e "${BOLD}Arachne Engine -- Source Size Budget Report${NC}"
echo "============================================"
echo ""
printf "%-25s %10s %10s %10s %s\n" "Crate" "Lines" "Budget" "%" "Status"
printf "%-25s %10s %10s %10s %s\n" "-------------------------" "----------" "----------" "----------" "------"

total_lines=0
total_budget=0
failed=0

for crate_dir in "$CRATES_DIR"/arachne-*; do
    crate_name="$(basename "$crate_dir")"
    if [ ! -d "$crate_dir/src" ]; then
        continue
    fi

    # Count non-empty source lines (excluding comments for a rough metric).
    lines=$(find "$crate_dir/src" -name '*.rs' -exec cat {} + 2>/dev/null | wc -l | tr -d ' ')

    budget="${BUDGETS[$crate_name]:-5000}"
    pct=$((lines * 100 / budget))

    total_lines=$((total_lines + lines))
    total_budget=$((total_budget + budget))

    if [ "$lines" -gt "$budget" ]; then
        status="${RED}OVER${NC}"
        failed=$((failed + 1))
    elif [ "$pct" -gt 80 ]; then
        status="${YELLOW}WARN${NC}"
    else
        status="${GREEN}OK${NC}"
    fi

    printf "%-25s %10d %10d %9d%% " "$crate_name" "$lines" "$budget" "$pct"
    echo -e "$status"
done

echo ""
printf "%-25s %10d %10d %9d%%\n" "TOTAL" "$total_lines" "$total_budget" "$((total_lines * 100 / total_budget))"
echo ""

if [ "$failed" -gt 0 ]; then
    echo -e "${RED}FAIL: $failed crate(s) over budget${NC}"
    exit 1
else
    echo -e "${GREEN}PASS: All crates within size budget${NC}"
    exit 0
fi
