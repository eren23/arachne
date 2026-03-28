#!/usr/bin/env bash
# frame_budget.sh -- Run benchmark tests and report results against budgets.
#
# Runs the benchmark test binaries via `cargo test`, captures timing output
# from stderr, and prints a results table with pass/fail status.
#
# Usage: ./tools/frame_budget.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

# Colors.
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BOLD='\033[1m'
NC='\033[0m'

echo ""
echo -e "${BOLD}Arachne Engine -- Frame Budget Test Report${NC}"
echo "==========================================="
echo ""

overall_pass=true
results=()

run_bench() {
    local test_name="$1"
    local label="$2"

    echo -e "${BOLD}Running: $label${NC}"

    # Run the test, capturing both stdout and stderr.
    local output
    if output=$(cargo test --test "$test_name" -- --nocapture 2>&1); then
        local status="${GREEN}PASS${NC}"
    else
        local status="${RED}FAIL${NC}"
        overall_pass=false
    fi

    results+=("$(printf "%-40s %s" "$label" "$status")")

    # Print the benchmark output lines (lines containing timing data).
    echo "$output" | grep -E '(ops/sec|entities/sec|ms|checks/sec|draw calls|moves/sec|reduction)' | while read -r line; do
        echo "  $line"
    done
    echo ""
}

# -- Math benchmarks --
run_bench "math_bench" "Math: Vec3 ops (>=200M ops/sec)"
run_bench "math_bench" "Math: Mat4 multiply (>=50M ops/sec)"
run_bench "math_bench" "Math: Quat slerp"

# -- ECS benchmarks --
run_bench "ecs_bench" "ECS: Spawn 3-component (>=500K/sec)"
run_bench "ecs_bench" "ECS: Query 1M (>=10M entities/sec)"
run_bench "ecs_bench" "ECS: Archetype move (>=200K/sec)"

# -- Physics benchmarks --
run_bench "physics_bench" "Physics: Broadphase (>=1M checks/sec)"
run_bench "physics_bench" "Physics: Narrowphase (>=500K/sec)"
run_bench "physics_bench" "Physics: Full step 1000 bodies (<4ms)"

# -- Render benchmarks --
run_bench "render_bench" "Render: Batch 100K sprites"
run_bench "render_bench" "Render: Draw call merging"

# -- Integration tests --
run_bench "full_app_lifecycle" "Integration: Full app lifecycle"
run_bench "ecs_stress" "Integration: ECS stress 100K"
run_bench "physics_determinism" "Integration: Physics determinism"
run_bench "scene_roundtrip" "Integration: Scene roundtrip"

echo ""
echo -e "${BOLD}Summary${NC}"
echo "======="
echo ""
printf "%-40s %s\n" "Test" "Status"
printf "%-40s %s\n" "----------------------------------------" "------"
for r in "${results[@]}"; do
    echo -e "$r"
done
echo ""

if $overall_pass; then
    echo -e "${GREEN}${BOLD}ALL TESTS PASSED${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}SOME TESTS FAILED${NC}"
    exit 1
fi
