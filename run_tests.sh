#!/usr/bin/env bash
# ── Serez-Code Test Runner ────────────────────────────────────────────────────
# Usage:
#   ./run_tests.sh                     # run all tests (including security)
#   ./run_tests.sh --filter switch     # run tests whose name contains "switch"
#   ./run_tests.sh --generate          # regenerate .expected golden files
#   ./run_tests.sh --unit              # only unit_*.sz tests (using framework)
#   ./run_tests.sh --e2e               # only E2E tests (numbered NN_*.sz)
#   ./run_tests.sh --security          # only security tests
#   ./run_tests.sh --cli               # only CLI, --eval, REPL and --check tests
#   ./run_tests.sh --ai                # only AI/ML training tests (ai_*.sz)
#
# This runner and run_tests.ps1 must execute the same logical suite. Any
# deliberate difference belongs in TESTS.md with the reason; a platform that
# passes because it ran fewer tests is not a quality gate.
#
# Test types:
#   tests/NN_*.sz     E2E — run and compare stdout vs NN_*.expected
#   tests/unit_*.sz   Unit — PASS = exit 0, Results: summary and no [FAIL] line
#                      A matching .expected makes it a legacy golden test.
#   tests/err_*.sz    Error — PASS = non-zero exit and at least one ❌ on stderr
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
ONLY_CLI=0
ONLY_AI=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --filter|-f)   FILTER="$2"; shift 2 ;;
        --generate|-g) GENERATE=1; shift ;;
        --unit|-u)     ONLY_UNIT=1; shift ;;
        --e2e|-e)      ONLY_E2E=1; shift ;;
        --security|-s) ONLY_SECURITY=1; shift ;;
        --cli|-c)      ONLY_CLI=1; shift ;;
        --ai)          ONLY_AI=1; shift ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TESTS_DIR="$ROOT/tests"
FRAMEWORK="$TESTS_DIR/framework.sz"
BINARY="$ROOT/target/release/sz"
TEMP_SZ="$TESTS_DIR/~unit_temp_$$.sz"
TEMP_OUT="/tmp/sz_test_$$_out.txt"
TEMP_ERR="/tmp/sz_test_$$_err.txt"
export SEREZ_HOME="$ROOT"
export SEREZ_PACKAGES="$ROOT/tests/packages"

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
if ! cargo build --release --manifest-path "$ROOT/Cargo.toml" 2>&1; then
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

    local stdout_out stderr_out exit_code
    "$BINARY" "$run_file" >"$TEMP_OUT" 2>"$TEMP_ERR"
    exit_code=$?
    stdout_out=$(cat "$TEMP_OUT")
    stderr_out=$(cat "$TEMP_ERR")

    # ── Error / security test ──────────────────────────────────────────────────
    if [[ "$is_err" == "1" ]]; then
        if [[ "$exit_code" -ne 0 ]] && echo "$stderr_out" | grep -q "❌"; then
            echo "${GREEN}[PASS]${RESET} $label"
            PASS=$((PASS + 1))
        else
            echo "${RED}[FAIL]${RESET} $label — expected non-zero exit and an error diagnostic (exit $exit_code)"
            FAIL=$((FAIL + 1))
        fi
        return
    fi

    # ── Unit test ──────────────────────────────────────────────────────────────
    if [[ "$is_unit" == "1" ]]; then
        local failures summary
        failures=$(echo "$stdout_out" | grep -F "[FAIL]" || true)
        summary=$(echo "$stdout_out" | grep "^Results:" | tail -1 || true)
        if [[ "$exit_code" -eq 0 && -z "$failures" && -n "$summary" ]]; then
            echo "${GREEN}[PASS]${RESET} $label"
            echo "${GRAY}       $summary${RESET}"
            PASS=$((PASS + 1))
        else
            local reason="framework reported failures"
            [[ "$exit_code" -ne 0 ]] && reason="process exited with code $exit_code"
            [[ "$exit_code" -eq 0 && -z "$failures" && -z "$summary" ]] && reason="missing Results: summary"
            echo "${RED}[FAIL]${RESET} $label — $reason"
            while IFS= read -r line; do
                echo "${YELLOW}       $line${RESET}"
            done <<< "$failures"
            FAIL=$((FAIL + 1))
        fi
        return
    fi

    if [[ "$exit_code" -ne 0 ]]; then
        echo "${RED}[FAIL]${RESET} $label — process exited with code $exit_code"
        head -n 3 "$TEMP_ERR" | while IFS= read -r line; do
            echo "${YELLOW}       $line${RESET}"
        done
        FAIL=$((FAIL + 1))
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

# The runner itself must reject a unit program that aborts before summary().
echo "${CYAN}═══ Test Runner Integrity ════════════════════${RESET}"
{ cat "$FRAMEWORK"; printf '\n'; cat "$TESTS_DIR/runner_fixtures/unit_abort_before_summary.sz"; } > "$TEMP_SZ"
"$BINARY" "$TEMP_SZ" >"$TEMP_OUT" 2>"$TEMP_ERR"
runner_exit=$?
runner_summary=$(grep "^Results:" "$TEMP_OUT" | tail -1 || true)
if [[ "$runner_exit" -ne 0 && -z "$runner_summary" ]]; then
    echo "${GREEN}[PASS]${RESET} runner rejects abort before summary"
    PASS=$((PASS + 1))
else
    echo "${RED}[FAIL]${RESET} runner accepted abort before summary"
    FAIL=$((FAIL + 1))
fi
echo ""

RUN_ALL=0
[[ "$ONLY_UNIT" == "0" && "$ONLY_E2E" == "0" && "$ONLY_SECURITY" == "0" \
   && "$ONLY_CLI" == "0" && "$ONLY_AI" == "0" ]] && RUN_ALL=1

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
        if [[ -f "$TESTS_DIR/$base.expected" ]]; then
            run_test "$base" "$f" "$TESTS_DIR/$base.expected" 0 0
        else
            run_test "$base" "$f" "" 1 0
        fi
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

# ── AI Tests ──────────────────────────────────────────────────────────────────
# ai_*.sz — framework-based tests for AI/ML training loops and autodiff behavior
echo ""
echo "${CYAN}═══ AI Tests ═════════════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_AI" == "1" ]]; then
    for f in "$TESTS_DIR"/ai_*.sz; do
        [[ -f "$f" ]] || continue
        base=$(basename "$f" .sz)
        run_test "$base" "$f" "" 1 0
    done
fi

# ── Rust Unit Tests ───────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ Rust Unit Tests ══════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_UNIT" == "1" ]]; then
    while IFS='|' read -r mod_filter mod_label; do
        [[ -n "$FILTER" && "$mod_label" != *"$FILTER"* ]] && continue
        # `cargo test <filter>` exits 0 when the filter matches nothing, so a
        # renamed or misspelled module would report PASS while asserting
        # nothing. Require that tests actually ran.
        cargo_out=$(cargo test "$mod_filter" --manifest-path "$ROOT/Cargo.toml" 2>&1)
        cargo_ok=$?
        ran=$(echo "$cargo_out" | grep -oE 'test result: ok\. [0-9]+ passed' \
              | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' \
              | awk '{ s += $1 } END { print s + 0 }')
        if [[ "$cargo_ok" -eq 0 && "$ran" -gt 0 ]]; then
            echo "${GREEN}[PASS]${RESET} $mod_label ($ran tests)"
            PASS=$((PASS + 1))
        elif [[ "$cargo_ok" -eq 0 ]]; then
            echo "${RED}[FAIL]${RESET} $mod_label — filter '$mod_filter' matched no tests"
            FAIL=$((FAIL + 1))
        else
            echo "${RED}[FAIL]${RESET} $mod_label"
            FAIL=$((FAIL + 1))
        fi
    done <<'RUST_MODULES'
package_manager::tests|package_manager unit tests
evaluator::namespaces_gui::css::tests|css nativo: condiciones and/or/not + bloques @when/@else
RUST_MODULES
fi

# ── CLI / --eval / REPL / --check helper ──────────────────────────────────────
# run_cli_test <label> <expect_out> <expect_err> <stdin> <workdir> -- <argv...>
# An empty expectation is not checked. Mirrors Run-CLI-Test in run_tests.ps1.
run_cli_test() {
    local label="$1" expect_out="$2" expect_err="$3" stdin_content="$4" workdir="$5"
    shift 5
    [[ -n "$FILTER" && "$label" != *"$FILTER"* ]] && return
    [[ -z "$workdir" ]] && workdir="$ROOT"

    local out err ok=1 reason=""
    if [[ -n "$stdin_content" ]]; then
        out=$(cd "$workdir" && printf '%s' "$stdin_content" | "$BINARY" "$@" 2>"$TEMP_ERR" || true)
    else
        out=$(cd "$workdir" && "$BINARY" "$@" < /dev/null 2>"$TEMP_ERR" || true)
    fi
    err=$(cat "$TEMP_ERR")

    if [[ -n "$expect_out" && "$out" != *"$expect_out"* ]]; then
        ok=0; reason="stdout missing '$expect_out'"
    fi
    if [[ -n "$expect_err" && "$err" != *"$expect_err"* ]]; then
        ok=0; reason="stderr missing '$expect_err'"
    fi

    if [[ "$ok" == "1" ]]; then
        echo "${GREEN}[PASS]${RESET} $label"; PASS=$((PASS + 1))
    else
        echo "${RED}[FAIL]${RESET} $label — $reason"; FAIL=$((FAIL + 1))
    fi
}

# ── CLI Tests ─────────────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ CLI Tests ════════════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_CLI" == "1" ]]; then
    run_cli_test "cli: --version prints version" "Serez-Code v" "" "" "" --version
    run_cli_test "cli: unknown flag reports error" "" "Unknown flag" "" "" --unknown-flag
    run_cli_test "cli: non-.sz file rejected" "" ".sz extension" "" "" readme.txt
    run_cli_test "cli: missing .sz file reports error" "" "ERROR reading file" "" "" \
        "$TESTS_DIR/this_file_does_not_exist.sz"
    # Regression 2026-07-14: the message must reach stderr intact
    # (previously "Referencia inválida" in `out f()`, and silence in f(g())).
    run_cli_test "cli: uncaught throw en out f() conserva el mensaje" "" "boom out con local 7" "" "" \
        "$TESTS_DIR/err_throw_out_stmt.sz"
    run_cli_test "cli: throw en argumento anidado no muere en silencio" "" "desde inner" "" "" \
        "$TESTS_DIR/err_throw_nested_arg.sz"
fi

# ── --eval Tests ──────────────────────────────────────────────────────────────
# `sz --eval` runs a snippet with no file behind it: no serez.json, so no
# permissions, and lockdown on. Same pipeline as `sz file.sz` (see src/run.rs) —
# these cover the door, not the interpreter.
echo ""
echo "${CYAN}═══ --eval Tests ═════════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_CLI" == "1" ]]; then
    run_cli_test "eval: runs a snippet from argv" "5" "" "" "" --eval 'out 2+3;'
    run_cli_test "eval: reads the snippet from stdin" "100" "" $'let x = 10;\nout x * x;' "" --eval -
    run_cli_test "eval: -e is accepted as a short form" "7" "" "" "" -e 'out 7;'
    run_cli_test "eval: no snippet reports usage" "" "Usage: sz --eval" "" "" --eval
    run_cli_test "eval: parse errors still abort" "" "Aborted" 'let = ;' "" --eval -

    # ── Lockdown ──────────────────────────────────────────────────────────────
    # The permission set is a manifest, not a sandbox. Everything below reaches
    # the machine without any permission being declared, so lockdown closes it.
    run_cli_test "eval/lockdown: use permissions denied" "" "use permissions" \
        $'use permissions { OS };\nout 1;' "" --eval -
    run_cli_test "eval/lockdown: File denied" "" "File is not available" \
        'out File.read("Cargo.toml");' "" --eval -
    run_cli_test "eval/lockdown: import denied" "" "import is not available" \
        'import "std/math";' "" --eval -
    run_cli_test "eval/lockdown: URL import denied" "" "import is not available" \
        'import "https://example.invalid/x.sz";' "" --eval -
    run_cli_test "eval/lockdown: Autodiff weights denied" "" "Autodiff.saveWeights" \
        'Autodiff.saveWeights("w.szw", []);' "" --eval -
    run_cli_test "eval/lockdown: permission set is empty" "" "requires permission" \
        'unsafe { OS.exec("whoami"); }' "" --eval -
    # Deliberately NOT gated: in the wasm build `fetch` runs in the viewer's own
    # tab under the browser's origin rules. Reaching the arity error proves the
    # builtin is still live under lockdown (and needs no network to check).
    run_cli_test "eval/lockdown: fetch is NOT gated" "" "fetch(url," 'out fetch();' "" --eval -

    # Lockdown is only for `--eval`; running your own file keeps declaring inline.
    TMP_PERM="$(mktemp -d)"
    printf 'use permissions { Time };\nout DateTime.now() != null;' > "$TMP_PERM/perm.sz"
    run_cli_test "eval/lockdown: sz file.sz still grants inline" "true" "" "" "" "$TMP_PERM/perm.sz"
    rm -rf "$TMP_PERM"
fi

# ── REPL Tests ────────────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ REPL Tests ═══════════════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_CLI" == "1" ]]; then
    run_cli_test "repl: arithmetic output" "5" "" 'out 2+3;' ""
    run_cli_test "repl: string output" "hello" "" 'out "hello";' ""
    run_cli_test "repl: variable persists across lines" "42" "" $'let x = 42;\nout x;' ""
    run_cli_test "repl: function defined and called" "12" "" \
        $'fn int add(int a, int b) { return a + b; }\nout add(5, 7);' ""
    run_cli_test "repl: error recovery continues" "survived" "❌" \
        $'out undefined_xyz_var;\nout "survived";' ""
fi

# ── --check Mode Tests ────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ --check Mode Tests ═══════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_CLI" == "1" ]]; then
    run_cli_test "check: Flash Scope Criticality header" "Flash Scope Criticality" "" "" "" \
        --check "$TESTS_DIR/01_basic.sz"
    run_cli_test "check: Estimated Global Memory line" "Estimated Global Memory" "" "" "" \
        --check "$TESTS_DIR/01_basic.sz"
    run_cli_test "check: missing file reports error" "" "ERROR reading file" "" "" \
        --check "$TESTS_DIR/no_such_check_file.sz"
fi

# ── Package Manager CLI Tests ─────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ Package Manager Tests ════════════════════${RESET}"
if [[ "$RUN_ALL" == "1" || "$ONLY_CLI" == "1" ]]; then
    TMP_PROJECT="$(mktemp -d)"
    printf '%s' '{"name":"test-project","version":"1.0.0","dependencies":{"test-pkg":"1.0.0"}}' \
        > "$TMP_PROJECT/serez.json"
    export SEREZ_REGISTRY="$ROOT/tests/registry"

    run_cli_test "cli: sz install reads serez.json" "Installed test-pkg" "" "" "$TMP_PROJECT" install
    run_cli_test "cli: sz install pkg@ver explicit" "Installed test-pkg" "" "" "$TMP_PROJECT" \
        install test-pkg@1.0.0

    # Filtered runs must not inherit state from a test the filter skipped.
    # Arrange the uninstall precondition without counting it as a separate test.
    UNINSTALL_LABEL="cli: sz uninstall removes package"
    if [[ -z "$FILTER" || "$UNINSTALL_LABEL" == *"$FILTER"* ]]; then
        (cd "$TMP_PROJECT" && "$BINARY" install test-pkg@1.0.0 >/dev/null 2>&1 || true)
    fi
    run_cli_test "cli: sz uninstall removes package" "Uninstalled test-pkg" "" "" "$TMP_PROJECT" \
        uninstall test-pkg
    run_cli_test "cli: sz uninstall nonexistent errors" "" "not installed" "" "$TMP_PROJECT" \
        uninstall test-pkg

    unset SEREZ_REGISTRY
    rm -rf "$TMP_PROJECT"

    # ── sz init ───────────────────────────────────────────────────────────────
    TMP_INIT="$(mktemp -d)"
    run_cli_test "cli: sz init --y creates serez.json" "Created serez.json" "" "" "$TMP_INIT" init --y

    INIT_LABEL="cli: sz init --y serez.json has name/scripts/dev"
    if [[ -z "$FILTER" || "$INIT_LABEL" == *"$FILTER"* ]]; then
        INIT_JSON=$(cat "$TMP_INIT/serez.json" 2>/dev/null || true)
        if [[ "$INIT_JSON" == *'"name"'* && "$INIT_JSON" == *'"scripts"'* && "$INIT_JSON" == *'"dev"'* ]]; then
            echo "${GREEN}[PASS]${RESET} $INIT_LABEL"; PASS=$((PASS + 1))
        else
            echo "${RED}[FAIL]${RESET} $INIT_LABEL"; FAIL=$((FAIL + 1))
        fi
    fi

    run_cli_test "cli: sz init --y overwrites existing serez.json" "Created serez.json" "" "" \
        "$TMP_INIT" init --y
    rm -rf "$TMP_INIT"

    # ── sz run ────────────────────────────────────────────────────────────────
    TMP_RUN="$(mktemp -d)"
    printf '%s' '{"name":"run-test","version":"1.0.0","scripts":{"hello":"echo hello-from-script"}}' \
        > "$TMP_RUN/serez.json"
    run_cli_test "cli: sz run executes script from serez.json" "hello-from-script" "" "" "$TMP_RUN" \
        run hello
    run_cli_test "cli: sz run nonexistent script reports error" "" "not found" "" "$TMP_RUN" \
        run nonexistent
    run_cli_test "cli: sz run no args reports usage error" "" "Usage: sz run" "" "" run
    rm -rf "$TMP_RUN"

    # local ./packages/ resolution — temp dir so nothing lands in repo root
    LP_LABEL="pkg: import resolves from ./packages/ (not SEREZ_PACKAGES)"
    if [[ -n "$FILTER" && "$LP_LABEL" != *"$FILTER"* ]]; then LP_SKIP=1; else LP_SKIP=0; fi
    TMP_LP="$(mktemp -d)"
    mkdir -p "$TMP_LP/packages/local-only"
    printf 'fn int localAdd(int a, int b) { return a + b; }\nlet LOCAL_VERSION = "local-only@1.0.0";' \
        > "$TMP_LP/packages/local-only/index.sz"
    printf 'import "local-only"; out localAdd(3, 4); out localAdd(-1, 5); out LOCAL_VERSION;' \
        > "$TMP_LP/test.sz"
    if [[ "$LP_SKIP" == "0" ]]; then
        out=$(cd "$TMP_LP" && "$BINARY" test.sz 2>"$TEMP_ERR" || true)
        if [[ "$out" == *"7"* && "$out" == *"4"* && "$out" == *"local-only"* ]]; then
            echo "${GREEN}[PASS]${RESET} $LP_LABEL"
            PASS=$((PASS + 1))
        else
            echo "${RED}[FAIL]${RESET} $LP_LABEL — out: $out"
            FAIL=$((FAIL + 1))
        fi
    fi
    rm -rf "$TMP_LP"
fi

# ── Cleanup & Summary ─────────────────────────────────────────────────────────
rm -f "$TEMP_SZ" "$TEMP_OUT" "$TEMP_ERR"

echo ""
echo "${CYAN}═══════════════════════════════════════════════${RESET}"
COLOR=$([[ "$FAIL" -gt 0 ]] && echo "$RED" || echo "$GREEN")
echo "${COLOR}TOTAL: $PASS passed  $FAIL failed  $SKIP skipped${RESET}"

[[ "$FAIL" -gt 0 ]] && exit 1 || exit 0
