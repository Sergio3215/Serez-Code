#!/usr/bin/env bash
# ── Serez-Code Benchmark Suite (Unix) ─────────────────────────────────────────
# Usage:
#   ./run_benchmarks.sh                      # build + run all, 5 iterations
#   ./run_benchmarks.sh --no-build           # binary must already exist
#   ./run_benchmarks.sh -n 10                # iterations per benchmark
#   ./run_benchmarks.sh --filter oop         # only names containing "oop"
#   ./run_benchmarks.sh --json bench.json    # also write a machine-readable run
#   ./run_benchmarks.sh --baseline old.json  # compare against a recorded run
#
# The counterpart to run_benchmarks.ps1. Until this existed the suite could not
# be run on Linux or macOS at all — the same platform gap the conformance
# runners had, where a missing half means nobody outside one OS can reproduce a
# number, let alone a regression.
#
# **The reported statistic is the minimum, not the mean.** A process is only
# ever slowed down by its neighbours, never sped up, so the fastest of N runs is
# the least-contaminated estimate of the work itself. The mean and max are
# recorded too, because their spread is the honest measure of how much to trust
# the number: on an idle desktop `00_startup` was observed between 35 ms and
# 69 ms across two runs.
#
# Exit code: 0 = every benchmark exited 0. A slow benchmark is not a failure
# here; see --baseline for regression checking, and TESTS.md for why that is
# deliberately not a CI gate.
# ─────────────────────────────────────────────────────────────────────────────

NO_BUILD=0
N=5
FILTER=""
JSON_OUT=""
BASELINE=""
# A run has to be this much slower than its baseline to be called a regression.
# 25% is wide on purpose: see the spread above. A threshold tighter than the
# machine's own noise reports noise.
THRESHOLD_PCT=25

while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-build)     NO_BUILD=1; shift ;;
        -n|--n)         N="$2"; shift 2 ;;
        --filter|-f)    FILTER="$2"; shift 2 ;;
        --json|-j)      JSON_OUT="$2"; shift 2 ;;
        --baseline|-b)  BASELINE="$2"; shift 2 ;;
        --threshold)    THRESHOLD_PCT="$2"; shift 2 ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY="$ROOT/target/release/sz"
BENCH_DIR="$ROOT/benchmarks"
export SEREZ_HOME="$ROOT"
export SEREZ_PACKAGES="$ROOT/tests/packages"

if [[ -t 1 ]]; then
    RED=$'\033[31m' GREEN=$'\033[32m' YELLOW=$'\033[33m'
    CYAN=$'\033[36m' GRAY=$'\033[90m' RESET=$'\033[0m'
else
    RED="" GREEN="" YELLOW="" CYAN="" GRAY="" RESET=""
fi

if [[ "$NO_BUILD" == "0" ]]; then
    echo "${CYAN}Building (release)...${RESET}"
    if ! (cd "$ROOT" && cargo build --release >/dev/null 2>&1); then
        echo "${RED}Build failed.${RESET}" >&2
        exit 1
    fi
fi

if [[ ! -x "$BINARY" ]]; then
    echo "${RED}No release binary at $BINARY — run without --no-build.${RESET}" >&2
    exit 1
fi

CORE_VERSION=$("$BINARY" --version 2>/dev/null | head -1 | tr -d '\r')
STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
PLATFORM="$(uname -s)"

echo "════════════════════════════════════════════════════════════════════════"
echo "  Serez-Code  ·  Benchmark Suite"
echo "  Core : $CORE_VERSION"
echo "  Host : $PLATFORM $(uname -m)"
echo "  Mode : release binary  |  iterations per benchmark: $N"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# `time` with a 3-decimal real format is the one high-resolution clock available
# in bash on both Linux and macOS; `date +%s%N` is a GNU extension macOS lacks.
run_once() {
    local path="$1"
    local out
    TIMEFORMAT='%3R'
    out=$( { time "$BINARY" "$path" >/dev/null 2>/tmp/sz_bench_err_$$; } 2>&1 )
    BENCH_EXIT=$?
    # seconds with three decimals -> integer milliseconds, without bc
    echo "${out}" | awk '{ printf "%d", ($1 * 1000) + 0.5 }'
}

RESULTS_TSV="$(mktemp)"
FAILED=0
PASSED=0
TOTAL=0

for f in "$BENCH_DIR"/*.sz; do
    [[ -e "$f" ]] || continue
    name="$(basename "$f" .sz)"
    [[ -n "$FILTER" && "$name" != *"$FILTER"* ]] && continue
    TOTAL=$((TOTAL + 1))

    printf '  %-34s' "$name"
    min=-1; max=-1; sum=0; ok=1
    for ((i = 0; i < N; i++)); do
        ms=$(run_once "$f")
        [[ "$BENCH_EXIT" -ne 0 ]] && ok=0
        [[ "$min" -lt 0 || "$ms" -lt "$min" ]] && min="$ms"
        [[ "$ms" -gt "$max" ]] && max="$ms"
        sum=$((sum + ms))
        printf '.'
    done
    avg=$((sum / N))

    if [[ "$ok" == "1" ]]; then
        printf '  %6s ms min  (avg %s, max %s)\n' "$min" "$avg" "$max"
        PASSED=$((PASSED + 1))
        status="pass"
    else
        printf '  %6s ms min  (avg %s, max %s)  %s\n' "$min" "$avg" "$max" "${RED}← FAILED${RESET}"
        FAILED=$((FAILED + 1))
        status="fail"
    fi
    printf '%s\t%s\t%s\t%s\t%s\n' "$name" "$min" "$avg" "$max" "$status" >> "$RESULTS_TSV"
done
rm -f "/tmp/sz_bench_err_$$"

startup_min=$(awk -F'\t' '$1=="00_startup" { print $2 }' "$RESULTS_TSV")
[[ -z "$startup_min" ]] && startup_min=0

echo ""
echo "════════════════════════════════════════════════════════════════════════"
printf '  RESULTS — milliseconds, minimum of %s runs; startup ≈ %s ms\n' "$N" "$startup_min"
echo "════════════════════════════════════════════════════════════════════════"
printf '  %-34s %7s %7s %7s %8s\n' "Benchmark" "min" "avg" "max" "net min"
echo "  ──────────────────────────────────────────────────────────────────────"
while IFS=$'\t' read -r name min avg max status; do
    net=$((min - startup_min)); [[ "$net" -lt 0 ]] && net=0
    printf '  %-34s %7s %7s %7s %8s\n' "$name" "$min" "$avg" "$max" "$net"
done < "$RESULTS_TSV"
echo "  ──────────────────────────────────────────────────────────────────────"
printf '  Passed: %s/%s\n' "$PASSED" "$TOTAL"
echo ""

# ── Optional comparison against a recorded run ────────────────────────────────
REGRESSED=0
if [[ -n "$BASELINE" ]]; then
    if [[ ! -f "$BASELINE" ]]; then
        echo "${RED}Baseline '$BASELINE' not found.${RESET}" >&2
        rm -f "$RESULTS_TSV"
        exit 1
    fi
    echo "  Comparing against $BASELINE (threshold ${THRESHOLD_PCT}%)"
    echo "  ──────────────────────────────────────────────────────────────────────"
    while IFS=$'\t' read -r name min avg max status; do
        base=$(grep -o "\"name\": \"$name\", \"min\": [0-9]*" "$BASELINE" 2>/dev/null |
               grep -o '[0-9]*$' | head -1)
        [[ -z "$base" || "$base" -eq 0 ]] && continue
        delta=$(( (min - base) * 100 / base ))
        if [[ "$delta" -gt "$THRESHOLD_PCT" ]]; then
            printf '  %-34s %5s ms -> %5s ms  %s+%s%%%s\n' \
                   "$name" "$base" "$min" "$RED" "$delta" "$RESET"
            REGRESSED=$((REGRESSED + 1))
        elif [[ "$delta" -lt "-$THRESHOLD_PCT" ]]; then
            printf '  %-34s %5s ms -> %5s ms  %s%s%%%s\n' \
                   "$name" "$base" "$min" "$GREEN" "$delta" "$RESET"
        fi
    done < "$RESULTS_TSV"
    if [[ "$REGRESSED" -eq 0 ]]; then
        echo "  ${GREEN}No benchmark exceeded the threshold.${RESET}"
    else
        echo "  ${RED}$REGRESSED benchmark(s) slower than the threshold.${RESET}"
    fi
    echo ""
fi

# ── Optional machine-readable run ─────────────────────────────────────────────
if [[ -n "$JSON_OUT" ]]; then
    {
        printf '{\n'
        printf '  "schema": "serez-benchmarks/1",\n'
        printf '  "runner": "run_benchmarks.sh",\n'
        printf '  "platform": "%s",\n' "$PLATFORM"
        printf '  "core": "%s",\n' "$CORE_VERSION"
        printf '  "startedAt": "%s",\n' "$STARTED_AT"
        printf '  "iterations": %d,\n' "$N"
        printf '  "statistic": "min",\n'
        printf '  "benchmarks": ['
        first=1
        while IFS=$'\t' read -r name min avg max status; do
            [[ "$first" == "0" ]] && printf ','
            first=0
            printf '\n    { "name": "%s", "min": %s, "avg": %s, "max": %s, "status": "%s" }' \
                   "$name" "$min" "$avg" "$max" "$status"
        done < "$RESULTS_TSV"
        printf '\n  ]\n}\n'
    } > "$JSON_OUT"
    echo "  Report written to $JSON_OUT"
    echo ""
fi

rm -f "$RESULTS_TSV"
[[ "$FAILED" -gt 0 ]] && exit 1 || exit 0
