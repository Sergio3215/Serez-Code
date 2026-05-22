#!/usr/bin/env bash
# ── Serez-Code Test Runner ────────────────────────────────────────────────────
# Usage:
#   ./run_tests.sh                     # run all tests (including security)
#   ./run_tests.sh --filter switch     # run tests whose name contains "switch"
#   ./run_tests.sh --generate          # regenerate .expected golden files
#   ./run_tests.sh --unit              # only unit_*.sz tests (using framework)
#   ./run_tests.sh --e2e               # only E2E tests (numbered NN_*.sz)
#   ./run_tests.sh --security          # only security tests
#
# Test types:
#   tests/NN_*.sz     E2E — run and compare stdout vs NN_*.expected
#   tests/unit_*.sz   Unit — framework prepended; PASS = no [FAIL] line in stdout
#   tests/err_*.sz    Error — PASS = at least one ❌ on stderr
#   tests/sec_*.sz    Security error tests (same as err)
#   tests/unit_sec_*  Security unit tests (same as unit)
#
# Exit code: 0 = all passed, 1 = failures found
# ──────────────────────────────────────────────────────────────────────────────

FILTER=""
GENERATE=0
ONLY_UNIT=0
ONLY_E2E=0
ONLY_SECURITY=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --filter|-f)   FILTER="$2"; shift 2 ;;
        --generate|-g) GENERATE=1; shift ;;
        --unit|-u)     ONLY_UNIT=1; shift ;;
        --e2e|-e)      ONLY_E2E=1; shift ;;
        --security|-s) ONLY_SECURITY=1; shift ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTS_DIR="$ROOT/tests"
FRAMEWORK="$TESTS_DIR/framework.sz"
BINARY="$ROOT/target/debug/sz"
TEMP_SZ="/tmp/sz_test_$$_run.sz"
TEMP_ERR="/tmp/sz_test_$$_err.txt"

# Colors — disabled if stdout is not a terminal
if [[ -t 1 ]]; then
    RED=$'\033[31m' GREEN=$'\033[32m' YELLOW=$'\033[33m'
    CYAN=$'\033[36m' GRAY=$'\033[90m' RESET=$'\033[0m'
else
    RED="" GREEN="" YELLOW="" CYAN="" GRAY="" RESET=""
fi

PASS=0 FAIL=0 SKIP=0

# ── Build ──────────────────────────────────────────────────────────────────────
echo "${CYAN}Building...${RESET}"
if ! cargo build --manifest-path "$ROOT/Cargo.toml" 2>&1; then
    echo "${RED}BUILD FAILED${RESET}"
    exit 1
fi
echo "${GREEN}Build OK${RESET}"
echo ""

# ── run_test <label> <file> <expected> <is_unit:0|1> <is_err:0|1> ─────────────
run_test() {
    local label="$1" file="$2" expected="$3" is_unit="$4" is_err="$5"

    [[ -n "$FILTER" && "$label" != *"$FILTER"* ]] && return

    local run_file="$file"
    if [[ "$is_unit" == "1" ]]; then
        { cat "$FRAMEWORK"; printf '\n'; cat "$file"; } > "$TEMP_SZ"
        run_file="$TEMP_SZ"
    fi

    local stdout_out stderr_out
    stdout_out=$("$BINARY" "$run_file" 2>"$TEMP_ERR" || true)
    stderr_out=$(cat "$TEMP_ERR")

    # ── Error / security test ──────────────────────────────────────────────────
    if [[ "$is_err" == "1" ]]; then
        if echo "$stderr_out" | grep -q "❌"; then
            echo "${GREEN}[PASS]${RESET} $label"
            PASS=$((PASS + 1))
        else
            echo "${RED}[FAIL]${RESET} $label — expected an error but got none"
            FAIL=$((FAIL + 1))
        fi
        return
    fi

    # ── Unit test ──────────────────────────────────────────────────────────────
    if [[ "$is_unit" == "1" ]]; then
        local failures summary
        failures=$(echo "$stdout_out" | grep -F "[FAIL]" || true)
        summary=$(echo "$stdout_out" | grep "^Results:" | tail -1 || true)
        if [[ -z "$failures" ]]; then
            echo "${GREEN}[PASS]${RESET} $label"
            [[ -n "$summary" ]] && echo "${GRAY}       $summary${RESET}"
            PASS=$((PASS + 1))
        else
            echo "${RED}[FAIL]${RESET} $label"
            while IFS= read -r line; do
                echo "${YELLOW}       $line${RESET}"
            done <<< "$failures"
            FAIL=$((FAIL + 1))
        fi
        return
    fi

    # ── E2E golden file test ───────────────────────────────────────────────────
    if [[ "$GENERATE" == "1" ]]; then
        printf '%s' "$stdout_out" > "$expected"
        echo "${CYAN}[GEN]${RESET}  $label -> $expected"
        return
    fi

    if [[ ! -f "$expected" ]]; then
        echo "${YELLOW}[SKIP]${RESET} $label (no .expected — run with --generate to create)"
        SKIP=$((SKIP + 1))
        return
    fi

    local actual expected_content
    actual=$(printf '%s' "$stdout_out" | tr -d '\r')
    expected_content=$(tr -d '\r' < "$expected")

    if [[ "$actual" == "$expected_content" ]]; then
        echo "${GREEN}[PASS]${RESET} $label"
        PASS=$((PASS + 1))
    else
        echo "${RED}[FAIL]${RESET} $label"
        diff <(echo "$expected_content") <(echo "$actual") | grep "^[<>]" | \
        while IFS= read -r diffline; do
            if [[ "${diffline:0:1}" == "<" ]]; then
                echo "${YELLOW}       expected: ${diffline:2}${RESET}"
            else
                echo "${YELLOW}         actual: ${diffline:2}${RESET}"
            fi
        done
        FAIL=$((FAIL + 1))
    fi
}

RUN_ALL=0
[[ "$ONLY_UNIT" == "0" && "$ONLY_E2E" == "0" && "$ONLY_SECURITY" == "0" ]] && RUN_ALL=1

# ── E2E Tests ─────────────────────────────────────────────────────────────────
echo "${CYAN}═══ E2E Tests ════════════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_E2E" == "1" ]]; then
    for f in "$TESTS_DIR"/[0-9][0-9]_*.sz; do
        [[ -f "$f" ]] || continue
        base=$(basename "$f" .sz)
        run_test "$base" "$f" "$TESTS_DIR/$base.expected" 0 0
    done
fi

# ── Unit Tests ────────────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ Unit Tests ═══════════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_UNIT" == "1" ]]; then
    for f in "$TESTS_DIR"/unit_*.sz; do
        [[ -f "$f" ]] || continue
        base=$(basename "$f" .sz)
        [[ "$base" == unit_sec_* ]] && continue
        run_test "$base" "$f" "" 1 0
    done
fi

# ── Error Tests ───────────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ Error Tests ══════════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_E2E" == "1" ]]; then
    for f in "$TESTS_DIR"/err_*.sz; do
        [[ -f "$f" ]] || continue
        base=$(basename "$f" .sz)
        run_test "$base" "$f" "" 0 1
    done
fi

# ── Security Tests ────────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ Security Tests ═══════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_SECURITY" == "1" ]]; then
    for f in "$TESTS_DIR"/sec_*.sz; do
        [[ -f "$f" ]] || continue
        base=$(basename "$f" .sz)
        run_test "$base" "$f" "" 0 1
    done
    for f in "$TESTS_DIR"/unit_sec_*.sz; do
        [[ -f "$f" ]] || continue
        base=$(basename "$f" .sz)
        run_test "$base" "$f" "" 1 0
    done
fi

# ── Cleanup & Summary ─────────────────────────────────────────────────────────
rm -f "$TEMP_SZ" "$TEMP_ERR"

echo ""
echo "${CYAN}═══════════════════════════════════════════════${RESET}"
COLOR=$([[ "$FAIL" -gt 0 ]] && echo "$RED" || echo "$GREEN")
echo "${COLOR}TOTAL: $PASS passed  $FAIL failed  $SKIP skipped${RESET}"

[[ "$FAIL" -gt 0 ]] && exit 1 || exit 0
