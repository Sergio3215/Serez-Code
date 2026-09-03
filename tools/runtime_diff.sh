#!/usr/bin/env bash
#
# Differential runtime harness — record exactly what every self-contained
# fixture prints, so a refactor that claims to preserve behaviour can be made to
# prove it.
#
# WHY THIS EXISTS
#
#   DEC-M6-001 in docs/maturity/ROADMAP_STATE.md calls 12,000 lines of
#   behaviour-preserving change against a suite that asserts what programs print
#   "the highest-risk work in the roadmap", and asks for a differential harness
#   *before* that work rather than after it. The conformance runners answer
#   pass/fail against golden files; this answers "is the output the same text",
#   which is a stronger question and a much shorter feedback loop.
#
#   It was written for §6/M6 — splitting EvalResult into ExecutionFlow and
#   RuntimeFailure — and left here because the next such refactor needs it too.
#
# USAGE
#
#   cargo build --release
#   bash tools/runtime_diff.sh /tmp/before.txt      # on the unchanged tree
#   ...make the change, cargo build --release...
#   bash tools/runtime_diff.sh /tmp/after.txt
#   diff /tmp/before.txt /tmp/after.txt             # must be empty
#
# WHAT IT COVERS, AND WHAT IT DOES NOT
#
#   Every tests/err_*.sz, tests/sec_*.sz and tests/NN_*.sz fixture: exit code,
#   stdout and stderr, verbatim. 251 programs at the time of writing.
#
#   Fixtures whose output depends on the host rather than on the interpreter are
#   skipped by name — network, sockets, websockets, GUI, media, GPU, process
#   spawning. Including them would put the machine's mood in the diff and train
#   the reader to ignore it. Those paths are covered by the conformance runners
#   and by the ecosystem canary instead.
set -u

OUT="${1:?usage: runtime_diff.sh <output-file>}"
SZ="${SZ:-target/release/sz.exe}"
[ -x "$SZ" ] || SZ="target/release/sz"
[ -x "$SZ" ] || { echo "no release binary; run cargo build --release" >&2; exit 1; }

: > "$OUT"
count=0
for f in tests/err_*.sz tests/sec_*.sz tests/[0-9][0-9]_*.sz; do
    [ -e "$f" ] || continue
    name=$(basename "$f")
    case "$name" in
        *fetch*|*socket*|*websocket*|*gui*|*media*|*gpu*|*spawn*|*os_exec*) continue;;
    esac
    out=$("$SZ" "$f" 2>/dev/null); code=$?
    err=$("$SZ" "$f" 2>&1 >/dev/null)
    printf '=== %s\nexit=%s\n--- stdout\n%s\n--- stderr\n%s\n' "$name" "$code" "$out" "$err" >> "$OUT"
    count=$((count + 1))
done
echo "$count fixtures -> $OUT"
