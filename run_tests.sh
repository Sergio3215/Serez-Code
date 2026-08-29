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
#   ./run_tests.sh --json report.json  # also write a machine-readable report
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
JSON_OUT=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --filter|-f)   FILTER="$2"; shift 2 ;;
        --generate|-g) GENERATE=1; shift ;;
        --unit|-u)     ONLY_UNIT=1; shift ;;
        --e2e|-e)      ONLY_E2E=1; shift ;;
        --security|-s) ONLY_SECURITY=1; shift ;;
        --cli|-c)      ONLY_CLI=1; shift ;;
        --ai)          ONLY_AI=1; shift ;;
        --json|-j)     JSON_OUT="$2"; shift 2 ;;
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
STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Every counted outcome is also recorded, so the run can be read by something
# other than a human scrolling a terminal. `--json <path>` writes it out; the
# record is kept either way and the self-check at the end compares its line
# count against the counters, which is what keeps the recorder honest.
# A file rather than an array: some outcomes are counted inside a subshell,
# where an array assignment would be lost.
RESULTS_TSV="$(mktemp)"
CATEGORY="startup"

# record <pass|fail|skip> <label> [detail]
record() {
    case "$1" in
        pass) PASS=$((PASS + 1)) ;;
        fail) FAIL=$((FAIL + 1)) ;;
        skip) SKIP=$((SKIP + 1)) ;;
        *) echo "record: unknown status '$1'" >&2; exit 1 ;;
    esac
    printf '%s\t%s\t%s\t%s\n' "$1" "$CATEGORY" "$2" "${3-}" >> "$RESULTS_TSV"
}

# Minimal JSON string escaping: backslash, quote, tab and any stray control
# character. Labels and details are ours, so this is the whole surface.
json_str() {
    printf '%s' "$1" | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' -e 's/\t/ /g' -e 's/\r//g'
}

# ── Fixture preflight ──────────────────────────────────────────────────────────
# These trees are loaded by the import/export, package and runner-integrity
# tests. They were excluded by .gitignore for a long time, so a fresh clone had
# none of them and eight tests failed with "ModuleNotFound" — a message that
# points at the language, not at the missing checkout. Fail here instead.
REQUIRED_FIXTURES=(
    "tests/lib/greet.sz|unit_sec_import, unit_import, 46_import_e2e"
    "tests/lib/math_utils.sz|unit_import, unit_export, 47_export_e2e, sec_export"
    "tests/packages/serez.json|unit_packages, 55_packages_e2e (via SEREZ_PACKAGES)"
    "tests/runner_fixtures/unit_abort_before_summary.sz|runner integrity check"
    "std/result.sz|unit_stdlib_*, 48_stdlib_e2e (via SEREZ_HOME)"
    "std/iter.sz|unit_stdlib_iter, unit_generators, 50_generators_e2e"
)
missing_fixtures=()
for entry in "${REQUIRED_FIXTURES[@]}"; do
    path="${entry%%|*}"
    [[ -f "$ROOT/$path" ]] || missing_fixtures+=("$entry")
done
if (( ${#missing_fixtures[@]} > 0 )); then
    echo "${RED}Missing test fixtures — this checkout cannot produce a valid result:${RESET}"
    for entry in "${missing_fixtures[@]}"; do
        echo "${YELLOW}  ${entry%%|*}  (needed by ${entry#*|})${RESET}"
    done
    echo "${RED}These files must be tracked in git. Check .gitignore.${RESET}"
    exit 1
fi

# ── Build ──────────────────────────────────────────────────────────────────────
echo "${CYAN}Building...${RESET}"
if ! cargo build --release --manifest-path "$ROOT/Cargo.toml" 2>&1; then
    echo "${RED}BUILD FAILED${RESET}"
    exit 1
fi
echo "${GREEN}Build OK${RESET}"
echo ""

# ── Paths handed to the binary ────────────────────────────────────────────────
# MSYS2 / Git Bash rewrites POSIX paths into Windows paths when it calls a
# native executable, but it declines to rewrite an argument whose final
# component begins with `~`. The unit temp file is named `~unit_temp_$$.sz`
# precisely so the `unit_*.sz` glob skips it, so `sz` received a literal
# `/e/...` path it cannot open and every unit test on this platform died with
# "ERROR reading file" — 168 of 474 tests that never executed here while
# run_tests.ps1 reported 474 green. Convert explicitly instead of relying on
# the heuristic. `cygpath` is absent on Linux and macOS, where the path is
# already native.
to_native_path() {
    if command -v cygpath >/dev/null 2>&1; then
        cygpath -w "$1"
    else
        printf '%s' "$1"
    fi
}

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
    "$BINARY" "$(to_native_path "$run_file")" >"$TEMP_OUT" 2>"$TEMP_ERR"
    exit_code=$?
    stdout_out=$(cat "$TEMP_OUT")
    stderr_out=$(cat "$TEMP_ERR")

    # ── Error / security test ──────────────────────────────────────────────────
    if [[ "$is_err" == "1" ]]; then
        if [[ "$exit_code" -ne 0 ]] && echo "$stderr_out" | grep -q "❌"; then
            echo "${GREEN}[PASS]${RESET} $label"
            record pass "$label"
        else
            echo "${RED}[FAIL]${RESET} $label — expected non-zero exit and an error diagnostic (exit $exit_code)"
            record fail "$label" "expected non-zero exit and an error diagnostic (exit $exit_code)"
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
            record pass "$label" "$summary"
        else
            local reason="framework reported failures"
            [[ "$exit_code" -ne 0 ]] && reason="process exited with code $exit_code"
            [[ "$exit_code" -eq 0 && -z "$failures" && -z "$summary" ]] && reason="missing Results: summary"
            echo "${RED}[FAIL]${RESET} $label — $reason"
            while IFS= read -r line; do
                echo "${YELLOW}       $line${RESET}"
            done <<< "$failures"
            # A unit program that dies before summary() prints its diagnostic on
            # stderr, and $failures is empty — without this the whole report is
            # "process exited with code 1" and nothing else. The PowerShell
            # runner has always shown it here; this side had not.
            if [[ "$exit_code" -ne 0 ]]; then
                head -n 3 "$TEMP_ERR" | while IFS= read -r line; do
                    echo "${YELLOW}       $line${RESET}"
                done
            fi
            record fail "$label" "$reason"
        fi
        return
    fi

    if [[ "$exit_code" -ne 0 ]]; then
        echo "${RED}[FAIL]${RESET} $label — process exited with code $exit_code"
        head -n 3 "$TEMP_ERR" | while IFS= read -r line; do
            echo "${YELLOW}       $line${RESET}"
        done
        record fail "$label" "process exited with code $exit_code"
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
        record skip "$label" "no .expected file"
        return
    fi

    local actual expected_content
    actual=$(printf '%s' "$stdout_out" | tr -d '\r')
    expected_content=$(tr -d '\r' < "$expected")

    if [[ "$actual" == "$expected_content" ]]; then
        echo "${GREEN}[PASS]${RESET} $label"
        record pass "$label"
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
        record fail "$label" "stdout differs from the golden file"
    fi
}

# The runner itself must reject a unit program that aborts before summary().
echo "${CYAN}═══ Test Runner Integrity ════════════════════${RESET}"
CATEGORY="runner-integrity"
{ cat "$FRAMEWORK"; printf '\n'; cat "$TESTS_DIR/runner_fixtures/unit_abort_before_summary.sz"; } > "$TEMP_SZ"
"$BINARY" "$(to_native_path "$TEMP_SZ")" >"$TEMP_OUT" 2>"$TEMP_ERR"
runner_exit=$?
runner_summary=$(grep "^Results:" "$TEMP_OUT" | tail -1 || true)
# A non-zero exit with no summary is not enough on its own: a path the binary
# cannot open satisfies both, and that is exactly what happened here while the
# tilde in the temp name defeated MSYS2 path conversion — this guard reported
# PASS for a file-read error rather than for the abort it exists to detect.
# Require the fixture's own diagnostic, so the check can only pass by actually
# running the program.
runner_diag=$(grep -c "SZ4004" "$TEMP_ERR" || true)
if [[ "$runner_exit" -ne 0 && -z "$runner_summary" && "$runner_diag" -gt 0 ]]; then
    echo "${GREEN}[PASS]${RESET} runner rejects abort before summary"
    record pass "runner rejects abort before summary"
else
    reason="the runner accepted a suite that aborted before summary()"
    [[ "$runner_diag" -eq 0 ]] && reason="the fixture never reached the interpreter: $(head -n 1 "$TEMP_ERR")"
    echo "${RED}[FAIL]${RESET} runner rejects abort before summary — $reason"
    record fail "runner rejects abort before summary" "$reason"
fi
echo ""

RUN_ALL=0
[[ "$ONLY_UNIT" == "0" && "$ONLY_E2E" == "0" && "$ONLY_SECURITY" == "0" \
   && "$ONLY_CLI" == "0" && "$ONLY_AI" == "0" ]] && RUN_ALL=1

# ── E2E Tests ─────────────────────────────────────────────────────────────────
echo "${CYAN}═══ E2E Tests ════════════════════════════════${RESET}"
CATEGORY="e2e"
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
CATEGORY="unit"
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
CATEGORY="error"
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
CATEGORY="security"
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
CATEGORY="ai"
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
CATEGORY="rust"
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
            record pass "$mod_label" "$ran tests"
        elif [[ "$cargo_ok" -eq 0 ]]; then
            echo "${RED}[FAIL]${RESET} $mod_label — filter '$mod_filter' matched no tests"
            record fail "$mod_label" "filter '$mod_filter' matched no tests"
        else
            echo "${RED}[FAIL]${RESET} $mod_label"
            record fail "$mod_label" "cargo test reported failures"
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
        echo "${GREEN}[PASS]${RESET} $label"; record pass "$label"
    else
        echo "${RED}[FAIL]${RESET} $label — $reason"; record fail "$label" "$reason"
    fi
}


# -- run_repl_test <label> <expect_out> <forbid_out> <expect_err> <stdin_fixture>
# The REPL cases below can only be stated as an absence: proving that a line the
# parser rejected did NOT run, and that a line the process could not decode did
# NOT kill the session. `run_cli_test` asserts containment only, which is why
# neither defect was visible to the five REPL cases that already existed. The
# input comes from a fixture file because one of them is deliberately not UTF-8
# and cannot survive being carried as a shell or PowerShell string.
run_repl_test() {
    local label="$1" expect_out="$2" forbid_out="$3" expect_err="$4" stdin_fixture="$5"
    [[ -n "$FILTER" && "$label" != *"$FILTER"* ]] && return

    local out err ok=1 reason=""
    out=$("$BINARY" < "$TESTS_DIR/runner_fixtures/$stdin_fixture" 2>"$TEMP_ERR" || true)
    err=$(cat "$TEMP_ERR")

    if [[ -n "$expect_out" && "$out" != *"$expect_out"* ]]; then
        ok=0; reason="stdout missing '$expect_out'"
    fi
    if [[ -n "$forbid_out" && "$out" == *"$forbid_out"* ]]; then
        ok=0; reason="stdout contains '$forbid_out', which must not have run"
    fi
    if [[ -n "$expect_err" && "$err" != *"$expect_err"* ]]; then
        ok=0; reason="stderr missing '$expect_err'"
    fi

    if [[ "$ok" == "1" ]]; then
        echo "${GREEN}[PASS]${RESET} $label"; record pass "$label"
    else
        echo "${RED}[FAIL]${RESET} $label - $reason"; record fail "$label" "$reason"
    fi
}

# ── CLI Tests ─────────────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ CLI Tests ════════════════════════════════${RESET}"
CATEGORY="cli"
if [[ "$RUN_ALL" == "1" || "$ONLY_CLI" == "1" ]]; then
    run_cli_test "cli: --version prints version" "Serez-Code v" "" "" "" --version
    run_cli_test "cli: --help prints usage on stdout" "USAGE" "" "" "" --help
    run_cli_test "cli: -h is accepted" "USAGE" "" "" "" -h
    run_cli_test "cli: help subcommand is accepted" "USAGE" "" "" "" help
    run_cli_test "cli: --help documents the exit codes" "EXIT CODES" "" "" "" --help
    run_cli_test "cli: unknown flag reports error" "" "Unknown flag" "" "" --unknown-flag
    run_cli_test "cli: unknown flag points at --help" "" "sz --help" "" "" --unknown-flag
    run_cli_test "cli: no file argument points at --help" "" "sz --help" "" "" --check
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
CATEGORY="eval"
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
CATEGORY="repl"
if [[ "$RUN_ALL" == "1" || "$ONLY_CLI" == "1" ]]; then
    run_cli_test "repl: arithmetic output" "5" "" 'out 2+3;' ""
    run_cli_test "repl: string output" "hello" "" 'out "hello";' ""
    run_cli_test "repl: variable persists across lines" "42" "" $'let x = 42;\nout x;' ""
    run_cli_test "repl: function defined and called" "12" "" \
        $'fn int add(int a, int b) { return a + b; }\nout add(5, 7);' ""
    run_cli_test "repl: error recovery continues" "survived" "❌" \
        $'out undefined_xyz_var;\nout "survived";' ""
    run_repl_test "repl: a parse error does not run the line" \
        "the session continues" "SIDE_EFFECT_RAN" \
        "Aborted: fix the parse errors" "repl_parse_error.txt"
    run_repl_test "repl: a parse error shows the source and caret" \
        "the session continues" "" "let x = ;" "repl_parse_error.txt"
    run_repl_test "repl: a non-UTF-8 line is skipped, not fatal" \
        "after the bad line" "" "did not contain valid UTF-8" "repl_invalid_utf8.txt"
    run_cli_test "repl: without a grant a namespace is denied" "" "SZ6001" \
        'out DateTime.now();' ""
    run_cli_test "repl: a grant persists across lines" "true" "" \
        $'use permissions { Time }\nout DateTime.now() != null;\nout OS.platform();' ""
    run_cli_test "repl: a grant opens only what it names" "true" "requires permission 'OS'" \
        $'use permissions { Time }\nout DateTime.now() != null;\nout OS.platform();' ""
fi

# ── --check Mode Tests ────────────────────────────────────────────────────────
echo ""
echo "${CYAN}═══ --check Mode Tests ═══════════════════════${RESET}"
CATEGORY="check"
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
CATEGORY="package-manager"
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

    # ── runtime requirement ───────────────────────────────────────────────────
    # `serez-code` in "dependencies" declares the minimum runtime, not a package
    # to fetch. serez-ui declares it; before the key was reserved, `sz install`
    # in that project failed on the space and the '>' in ">= 9.17.0".
    TMP_FLOOR="$(mktemp -d)"

    printf '%s' '{"name":"floor-ok","version":"1.0.0","dependencies":{"serez-code":">= 0.1.0"}}' \
        > "$TMP_FLOOR/serez.json"
    run_cli_test "cli: satisfied runtime requirement installs cleanly" \
        "runtime requirement satisfied" "" "" "$TMP_FLOOR" install

    printf '%s' '{"name":"floor-bad","version":"1.0.0","dependencies":{"serez-code":">= 999.0.0"}}' \
        > "$TMP_FLOOR/serez.json"
    run_cli_test "cli: unsatisfiable runtime requirement is reported" \
        "" "requires Serez Code >= 999.0.0" "" "$TMP_FLOOR" install

    run_cli_test "cli: the runtime is not an installable package" \
        "" "is the runtime" "" "$TMP_FLOOR" install serez-code

    rm -rf "$TMP_FLOOR"

    # ── sz init ───────────────────────────────────────────────────────────────
    TMP_INIT="$(mktemp -d)"
    run_cli_test "cli: sz init --y creates serez.json" "Created serez.json" "" "" "$TMP_INIT" init --y

    INIT_LABEL="cli: sz init --y serez.json has name/scripts/dev"
    if [[ -z "$FILTER" || "$INIT_LABEL" == *"$FILTER"* ]]; then
        INIT_JSON=$(cat "$TMP_INIT/serez.json" 2>/dev/null || true)
        if [[ "$INIT_JSON" == *'"name"'* && "$INIT_JSON" == *'"scripts"'* && "$INIT_JSON" == *'"dev"'* ]]; then
            echo "${GREEN}[PASS]${RESET} $INIT_LABEL"; record pass "$INIT_LABEL"
        else
            echo "${RED}[FAIL]${RESET} $INIT_LABEL"; record fail "$INIT_LABEL" "serez.json is missing name/scripts/dev"
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
            record pass "$LP_LABEL"
        else
            echo "${RED}[FAIL]${RESET} $LP_LABEL — out: $out"
            record fail "$LP_LABEL" "out: $out"
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

# The record must agree with the counters. A site that increments without
# recording would silently produce a report missing tests that ran — the same
# class of defect as a suite that cannot find its fixtures.
RECORDED=$(wc -l < "$RESULTS_TSV" | tr -d '[:space:]')
COUNTED=$((PASS + FAIL + SKIP))
if [[ "$RECORDED" != "$COUNTED" ]]; then
    echo "${RED}Runner defect: $COUNTED outcomes counted but $RECORDED recorded.${RESET}"
    rm -f "$RESULTS_TSV"
    exit 1
fi

if [[ -n "$JSON_OUT" ]]; then
    CORE_VERSION=$("$BINARY" --version 2>/dev/null | head -1 | tr -d '\r')
    {
        printf '{\n'
        printf '  "schema": "serez-conformance/1",\n'
        printf '  "runner": "run_tests.sh",\n'
        printf '  "platform": "%s",\n' "$(json_str "$(uname -s)")"
        printf '  "core": "%s",\n' "$(json_str "$CORE_VERSION")"
        printf '  "startedAt": "%s",\n' "$STARTED_AT"
        printf '  "filter": "%s",\n' "$(json_str "$FILTER")"
        printf '  "totals": { "passed": %d, "failed": %d, "skipped": %d },\n' "$PASS" "$FAIL" "$SKIP"
        printf '  "categories": {'
        first=1
        while IFS= read -r cat; do
            [[ -z "$cat" ]] && continue
            cp=$(awk -F'\t' -v c="$cat" '$2==c && $1=="pass"' "$RESULTS_TSV" | wc -l | tr -d '[:space:]')
            cf=$(awk -F'\t' -v c="$cat" '$2==c && $1=="fail"' "$RESULTS_TSV" | wc -l | tr -d '[:space:]')
            cs=$(awk -F'\t' -v c="$cat" '$2==c && $1=="skip"' "$RESULTS_TSV" | wc -l | tr -d '[:space:]')
            [[ "$first" == "0" ]] && printf ','
            first=0
            printf '\n    "%s": { "passed": %d, "failed": %d, "skipped": %d }' \
                   "$(json_str "$cat")" "$cp" "$cf" "$cs"
        done < <(cut -f2 "$RESULTS_TSV" | sort -u)
        printf '\n  },\n'
        printf '  "tests": ['
        first=1
        while IFS=$'\t' read -r st cat name detail; do
            [[ -z "$st" ]] && continue
            [[ "$first" == "0" ]] && printf ','
            first=0
            printf '\n    { "name": "%s", "category": "%s", "status": "%s", "detail": "%s" }' \
                   "$(json_str "$name")" "$(json_str "$cat")" "$st" "$(json_str "$detail")"
        done < "$RESULTS_TSV"
        printf '\n  ]\n}\n'
    } > "$JSON_OUT"
    echo "${CYAN}Report written to $JSON_OUT${RESET}"
fi

rm -f "$RESULTS_TSV"

[[ "$FAIL" -gt 0 ]] && exit 1 || exit 0
