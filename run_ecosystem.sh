#!/usr/bin/env bash
# ── Serez-Code ecosystem canary ───────────────────────────────────────────────
#
# Runs the official packages' own test suites against the core you just built,
# and reports one table. The core's own suite proves the language still does what
# its tests say; this proves the ecosystem still does what it did.
#
# `serez-ui` is the canary that matters most: it is the only official package
# exercising classes, inheritance, constructors, closures, method references,
# modules, GUI, CSS, JSX/SZX, callbacks and receiver writeback all at once, so a
# core change that breaks value semantics shows up here before anywhere else.
#
# Usage:
#   ./run_ecosystem.sh                  # every package found next to this repo
#   ./run_ecosystem.sh --only serez-ui  # just one
#   ./run_ecosystem.sh --skip-build     # reuse target/release/sz as-is
#
# Packages are expected as sibling checkouts (../serez-ui, ../serez-http, …).
# Missing ones are reported as SKIP rather than failing the run, so this works on
# a machine that has only part of the ecosystem cloned.
#
# Exit code: 0 when every package present passed, 1 otherwise.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PARENT="$(dirname "$ROOT")"
BINARY="$ROOT/target/release/sz"

ONLY=""
SKIP_BUILD=0
while [ $# -gt 0 ]; do
    case "$1" in
        --only)       ONLY="${2:-}"; shift 2 ;;
        --skip-build) SKIP_BUILD=1; shift ;;
        *) echo "Unknown argument: $1" >&2; exit 2 ;;
    esac
done

# Order matters: dependents come after what they depend on, so the first failure
# is the one closest to the core rather than a downstream symptom of it.
PACKAGES=(serez-ui serez-http serez-ai serez-agentai serez-pack serez-apipack serez-dotenv serez-graph)
if [ -n "$ONLY" ]; then PACKAGES=("$ONLY"); fi

if [ "$SKIP_BUILD" -eq 0 ]; then
    echo "Building core (release)..."
    if ! cargo build --release; then
        echo "BUILD FAILED" >&2
        exit 1
    fi
fi

if [ ! -x "$BINARY" ]; then
    echo "No binary at $BINARY — build the core first." >&2
    exit 1
fi

echo
echo "Ecosystem canary against: $("$BINARY" --version 2>&1)"
echo "Core: $BINARY"
echo

names=(); statuses=(); details=()
n_pass=0; n_fail=0; n_skip=0

for pkg in "${PACKAGES[@]}"; do
    dir="$PARENT/$pkg"
    runner="$dir/run_tests.sh"

    if [ ! -d "$dir" ]; then
        names+=("$pkg"); statuses+=("SKIP"); details+=("not checked out next to this repo")
        n_skip=$((n_skip + 1)); continue
    fi
    if [ ! -f "$runner" ]; then
        names+=("$pkg"); statuses+=("SKIP"); details+=("no run_tests.sh")
        n_skip=$((n_skip + 1)); continue
    fi

    echo "── $pkg ──────────────────────────────────────────"
    output="$(cd "$dir" && bash "$runner" "$BINARY" 2>&1)"
    code=$?

    # Prefer the runner's own tally over its exit code: some report totals and
    # still exit 0, and a green exit with failures in the log is the worst
    # possible outcome for a canary.
    if tally="$(printf '%s' "$output" | grep -oE 'TOTAL:[[:space:]]*[0-9]+[[:space:]]*passed[[:space:]]+[0-9]+[[:space:]]*failed' | tail -1)" && [ -n "$tally" ]; then
        passed="$(printf '%s' "$tally" | grep -oE '[0-9]+' | sed -n 1p)"
        failed="$(printf '%s' "$tally" | grep -oE '[0-9]+' | sed -n 2p)"
        if [ "$failed" -eq 0 ]; then status="PASS"; else status="FAIL"; fi
        detail="$passed passed, $failed failed"
    else
        fails="$(printf '%s' "$output" | grep -cE '^\[FAIL\]' || true)"
        if [ "$code" -eq 0 ] && [ "$fails" -eq 0 ]; then status="PASS"; else status="FAIL"; fi
        if [ "$fails" -gt 0 ]; then detail="$fails failing test(s)"; else detail="exit code $code"; fi
    fi

    if [ "$status" = "FAIL" ]; then
        printf '%s\n' "$output" | grep -E '\[FAIL\]|ERROR|panicked' | head -15 | sed 's/^/    /'
    fi

    echo "  -> $status ($detail)"
    echo
    names+=("$pkg"); statuses+=("$status"); details+=("$detail")
    if [ "$status" = "PASS" ]; then n_pass=$((n_pass + 1)); else n_fail=$((n_fail + 1)); fi
done

echo "═══════════════════════════════════════════════"
echo "Ecosystem compatibility"
echo "═══════════════════════════════════════════════"
for i in "${!names[@]}"; do
    printf '  %-16s %-5s %s\n' "${names[$i]}" "${statuses[$i]}" "${details[$i]}"
done
echo
echo "TOTAL: $n_pass passed  $n_fail failed  $n_skip skipped"

[ "$n_fail" -eq 0 ]
