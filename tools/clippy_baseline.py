#!/usr/bin/env python3
"""Clippy as a gate that debt cannot grow through, or move through.

DEC-M10-002. CI ran `cargo clippy --all-targets` **without** `-D warnings`, so
its warnings failed nothing and a new one was invisible. This roadmap worked
around that by comparing the per-site list by hand at every milestone boundary
(ROADMAP_STATE.md §5.26) — a discipline that existed because the gate did not.

    tools/clippy_baseline.py --check       # CI: fail if any warning is new
    tools/clippy_baseline.py --write       # refresh after fixing warnings
    tools/clippy_baseline.py --self-test   # check the gate itself, no clippy run

WHAT A WARNING IS IDENTIFIED BY, AND WHY IT CHANGED
---------------------------------------------------
The first version keyed on `(lint, file)` and a **count**. That is strictly
better than a total, and it still had a hole: fix one warning of a lint and
introduce a different one of the same lint in the same file, and the count is
unchanged. Measured, on this tree, with `clippy::needless_return` in
`src/evaluator/ops.rs` (59 sites):

    1. baseline, untouched                         PASS
    2. one new warning added                       FAIL   59 -> 60
    3. one fixed AND a different one added         PASS   <- the hole
    4. reverted                                    PASS

Step 3 is a genuinely new warning entering the tree while the gate reports
"180 sites in 61 pairs", unchanged. §5.49.

So a warning is now identified by **what it is**, not by how many of its kind
live in a file: the lint, the file, and a fingerprint over the offending source
text together with the normalised message. A new warning has a fingerprint the
baseline does not contain, whatever else was fixed alongside it.

WHY NOT LINE NUMBERS
--------------------
They move whenever anything above them is edited, so a baseline keyed on them
would need refreshing after every unrelated change and would be ignored within a
week. The fingerprint is over the *text* of the offending span, so moving a
warning down a file is invisible to the gate — which is the intent. Editing the
offending expression is not invisible, and should not be: that is a different
warning, and the tree is being asked to notice.

Two identical sites — the same expression written twice in one file — share a
fingerprint and are counted, so adding a third still fails.

WHY NOT `-D warnings`
---------------------
Because that fails until all of the existing warnings are fixed, which is a large
mechanical change touching files no milestone has otherwise needed to open. The
decision is explicit that a mass cleanup is not the goal: the goal is that the
debt cannot grow. Reducing it is always allowed — `--check` says so and points at
`--write`.

DETERMINISM
-----------
`--all-targets` compiles the same file into several targets, so the same warning
is reported more than once. Sites are deduplicated by (lint, file, line, column)
before being fingerprinted, paths are normalised to forward slashes, and the
fingerprint input has its whitespace collapsed, so the baseline is identical on
Linux, macOS and Windows.
"""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path

BASELINE = Path(__file__).resolve().parent.parent / "clippy-baseline.txt"

FORMAT = "serez-clippy-baseline/2"

HEADER = [
    f"# {FORMAT}",
    "# Known Clippy debt, as `<lint>\\t<file>\\t<fingerprint>\\t<count>\\t<what>`,",
    "# sorted. The last column is for the reader and is not compared.",
    "#",
    "# The gate is `current <= baseline` per **warning**, not per file: the",
    "# fingerprint is over the offending source text and the normalised message,",
    "# so a warning that is new fails even when another of the same lint in the",
    "# same file was fixed in the same change. Line numbers are deliberately not",
    "# part of it, so moving code does not touch this file.",
    "#",
    "# Fixing warnings is always allowed and is what `--write` is for.",
    "#",
    "# Sites are deduplicated by (lint, file, line, column) before being",
    "# fingerprinted and paths use forward slashes, so this file is identical on",
    "# all three runners.",
]

_WHITESPACE = re.compile(r"\s+")


def normalise(text: str) -> str:
    """Collapse whitespace so indentation and line wrapping do not matter."""
    return _WHITESPACE.sub(" ", text).strip()


# How much of a warning's source text its identity is taken from.
#
# A whole span is too much for an item-level lint. `derivable_impls` points at an
# entire `impl Default for RunOpts { ... }`, so **adding a field to the struct**
# changed the fingerprint and the gate reported a years-old warning as new. That
# is the noise this design set out not to create, and it was caught by the gate
# firing on the very next commit.
#
# A span's first line is enough to tell warnings apart and stable against edits
# inside the item: `impl Default for RunOpts {` survives a new field and still
# differs from `impl Default for Anything Else`. For the single-line spans most
# lints produce — `return Ok(self.false_ref)` — it is the whole thing, so the
# discrimination the gate exists for is untouched.
FINGERPRINT_CHARS = 120


def highlighted(span: dict) -> str:
    """The source the warning points at, from clippy's own rendering data.

    A span carries the lines it covers with the columns highlighted in each.
    Taking the highlighted part rather than the whole line keeps a warning's
    identity from depending on an unrelated edit elsewhere on the same line, and
    taking only the first line keeps it from depending on the item's body.
    """
    for line in span.get("text") or []:
        text = line.get("text", "")
        start = max(int(line.get("highlight_start", 1)) - 1, 0)
        end = int(line.get("highlight_end", len(text) + 1)) - 1
        piece = normalise(text[start:end])
        if piece:
            return piece[:FINGERPRINT_CHARS]
    return ""


def fingerprint(lint: str, path: str, snippet: str, message: str) -> str:
    """A short, stable id for one warning.

    Short on purpose: the baseline is read by people during review, and 12 hex
    characters over four fields is far past any collision this repository could
    produce while still fitting a column.
    """
    material = "\x00".join((lint, path, snippet, normalise(message)))
    return hashlib.sha256(material.encode("utf-8")).hexdigest()[:12]


def sites_from_messages(messages) -> tuple[Counter, dict]:
    """Fold clippy's JSON stream into counted identities plus their excerpts.

    Split out from `collect` so `--self-test` can drive it with synthetic
    messages and check the gate without a four-minute clippy run per step.
    """
    seen: set[tuple[str, str, int, int]] = set()
    counted: Counter = Counter()
    excerpts: dict[tuple[str, str, str], str] = {}

    for message in messages:
        if not message or message.get("level") != "warning":
            continue
        code = (message.get("code") or {}).get("code")
        if not code:
            # Warnings with no lint name are rustc's summary lines
            # ("N warnings emitted"), not sites.
            continue
        primary = [s for s in message.get("spans", []) if s.get("is_primary")]
        if not primary:
            continue
        span = primary[0]
        path = span["file_name"].replace("\\", "/")
        dedup = (code, path, span["line_start"], span["column_start"])
        if dedup in seen:
            continue
        seen.add(dedup)

        snippet = highlighted(span)
        text = message.get("message", "")
        key = (code, path, fingerprint(code, path, snippet, text))
        counted[key] += 1
        excerpts.setdefault(key, snippet or normalise(text))

    return counted, excerpts


def collect() -> tuple[Counter, dict]:
    """Every Clippy warning site in the workspace, by identity."""
    result = subprocess.run(
        ["cargo", "clippy", "--all-targets", "--message-format=json"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    # Clippy exits non-zero on a *compile* error; warnings alone leave it at 0.
    # A build that did not compile must not be read as "no warnings".
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(f"cargo clippy failed with exit {result.returncode}")

    messages = []
    for line in result.stdout.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            messages.append(json.loads(line).get("message"))
        except json.JSONDecodeError:
            continue
    return sites_from_messages(messages)


def read_baseline() -> Counter:
    if not BASELINE.exists():
        raise SystemExit(
            f"no baseline at {BASELINE}; run tools/clippy_baseline.py --write"
        )
    raw_text = BASELINE.read_text(encoding="utf-8")
    if FORMAT not in raw_text.splitlines()[0]:
        raise SystemExit(
            f"{BASELINE} is not {FORMAT}. It was re-keyed from (lint, file) counts\n"
            "to per-warning fingerprints — see §5.49. Regenerate it with\n"
            "`tools/clippy_baseline.py --write` and check the totals are unchanged."
        )
    counted: Counter = Counter()
    for raw in raw_text.splitlines():
        if not raw or raw.startswith("#"):
            continue
        fields = raw.split("\t")
        if len(fields) < 4:
            raise SystemExit(f"malformed baseline line: {raw!r}")
        lint, path, fp, count = fields[0], fields[1], fields[2], fields[3]
        counted[(lint, path, fp)] = int(count)
    return counted


def write(counted: Counter, excerpts: dict) -> None:
    lines = list(HEADER)
    lines.append(f"# total sites: {sum(counted.values())}")
    for key, count in sorted(counted.items()):
        lint, path, fp = key
        what = excerpts.get(key, "").replace("\t", " ")[:70]
        lines.append(f"{lint}\t{path}\t{fp}\t{count}\t{what}")
    BASELINE.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(
        f"wrote {BASELINE} — {sum(counted.values())} sites, "
        f"{len(counted)} distinct warnings"
    )


def check(current: Counter, baseline: Counter, excerpts: dict | None = None) -> int:
    excerpts = excerpts or {}
    grew = []
    new = []
    for key, count in sorted(current.items()):
        allowed = baseline.get(key, 0)
        if allowed == 0:
            new.append((key, count))
        elif count > allowed:
            grew.append((key, allowed, count))

    if new or grew:
        print("Clippy debt grew. The gate is `current <= baseline`, per warning.\n")
        for key, count in new:
            lint, path, fp = key
            print(f"  NEW   {lint}  {path}  ({count})")
            what = excerpts.get(key)
            if what:
                print(f"        {what[:70]}")
        for key, was, now in grew:
            lint, path, fp = key
            print(f"  GREW  {lint}  {path}  {was} -> {now}")
            what = excerpts.get(key)
            if what:
                print(f"        {what[:70]}")
        print(
            "\nA warning is NEW even when another of the same lint in the same file\n"
            "was fixed in the same change — that is what this gate is for. Fix it,\n"
            "or — if it is genuinely accepted debt — say so in the commit and\n"
            "refresh with `tools/clippy_baseline.py --write`. Refreshing to silence\n"
            "a real regression is the one thing this gate exists to stop."
        )
        return 1

    fixed = sum(
        allowed - current.get(key, 0)
        for key, allowed in baseline.items()
        if current.get(key, 0) < allowed
    )
    total = sum(current.values())
    print(f"Clippy debt within baseline: {total} sites, {len(current)} distinct.")
    if fixed:
        print(
            f"{fixed} site(s) fewer than the baseline — run --write to bank the "
            "reduction so it cannot come back."
        )
    return 0


# ── the gate's own test ──────────────────────────────────────────────────────


def _warning(lint: str, path: str, line: int, snippet, message: str) -> dict:
    """One clippy JSON warning, as the real `--message-format=json` emits it.

    `snippet` may be a string or a list of lines. A list models an item-level
    lint, whose span covers a whole `impl` or `fn` — the case that showed the
    fingerprint had to stop at the first line.
    """
    lines = [snippet] if isinstance(snippet, str) else list(snippet)
    return {
        "level": "warning",
        "code": {"code": lint},
        "message": message,
        "spans": [
            {
                "is_primary": True,
                "file_name": path,
                "line_start": line,
                "column_start": 13,
                "text": [
                    {
                        "text": " " * 12 + text,
                        "highlight_start": 13,
                        "highlight_end": 13 + len(text),
                    }
                    for text in lines
                ],
            }
        ],
    }


def self_test() -> int:
    """The four-step control, on synthetic warnings rather than a clippy run.

    The hole was in how warnings are keyed, so this drives that logic directly:
    four steps, each asserting a verdict, and each verdict differing from the
    one before it for a reason the step names. Synthetic because the real
    version — editing `src/evaluator/ops.rs` and running clippy four times —
    takes minutes and cannot run in CI on every push. §5.49 records that the
    real one was run, and what it printed.
    """
    LINT = "clippy::needless_return"
    FILE = "src/evaluator/ops.rs"
    MSG = "unneeded `return` statement"

    # Two warnings that differ only in the expression returned, which is
    # exactly the pair the old gate could not tell apart.
    old = _warning(LINT, FILE, 403, "return Ok(self.false_ref)", MSG)
    new = _warning(LINT, FILE, 900, "return Ok(self.probe_ref)", MSG)
    # And one that is a duplicate of `old` — same text, different line.
    twin = _warning(LINT, FILE, 412, "return Ok(self.false_ref)", MSG)

    baseline, _ = sites_from_messages([old, twin])

    failures = []

    def step(number: str, messages, want: int, why: str):
        current, excerpts = sites_from_messages(messages)
        got = check(current, baseline, excerpts)
        verdict = "FAIL" if got else "PASS"
        wanted = "FAIL" if want else "PASS"
        ok = got == want
        print(f"  {number:<44} {verdict}   (want {wanted})  {'ok' if ok else 'WRONG'}")
        print(f"     {why}\n")
        if not ok:
            failures.append(number)

    print("The four-step control for the baseline gate:\n")
    step("1. the baseline itself", [old, twin], 0, "an unchanged tree must pass")
    step(
        "2. one new warning added",
        [old, twin, new],
        1,
        "the obvious case, which the old gate also caught",
    )
    step(
        "3. one fixed AND a different one added",
        [twin, new],
        1,
        "the hole: the count for (lint, file) is unchanged at 2, and the new "
        "warning must still fail",
    )
    step("4. reverted", [old, twin], 0, "and the tree passes again")

    # A control on the control: moving a warning must NOT fail, or the gate
    # would be noise and would be turned off.
    moved = _warning(LINT, FILE, 2000, "return Ok(self.false_ref)", MSG)
    step(
        "5. the same warnings, moved down the file",
        [moved, _warning(LINT, FILE, 2100, "return Ok(self.false_ref)", MSG)],
        0,
        "line numbers are not part of the identity, so an unrelated edit above "
        "these lines is invisible",
    )

    # And a warning whose *item* changed while the warning did not. This is the
    # near-miss that shaped the fingerprint: `derivable_impls` spans a whole
    # `impl` block, so adding a field to the struct rewrote the span and the gate
    # called a long-standing warning new.
    ITEM = "clippy::derivable_impls"
    RUN = "src/run.rs"
    DERIVABLE = "this `impl` can be derived"
    before = _warning(
        ITEM,
        RUN,
        61,
        ["impl Default for RunOpts {", "fn default() -> Self {", "permissions: vec![],", "}", "}"],
        DERIVABLE,
    )
    after = _warning(
        ITEM,
        RUN,
        61,
        [
            "impl Default for RunOpts {",
            "fn default() -> Self {",
            "permissions: vec![],",
            "resolve_imports: false,",
            "}",
            "}",
        ],
        DERIVABLE,
    )
    item_baseline, _ = sites_from_messages([before])
    current, excerpts = sites_from_messages([after])
    got = check(current, item_baseline, excerpts)
    verdict = "FAIL" if got else "PASS"
    ok = got == 0
    print(f"  {'6. an item edited around an unchanged warning':<44} {verdict}   (want PASS)  {'ok' if ok else 'WRONG'}")
    print("     adding a struct field must not re-report the impl's own lint\n")
    if not ok:
        failures.append("6. an item edited around an unchanged warning")

    if failures:
        print(f"SELF-TEST FAILED at: {', '.join(failures)}")
        return 1
    print("Self-test passed: the gate fails a new warning even when one was fixed.")
    return 0


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "--check"
    if mode not in ("--check", "--write", "--self-test"):
        raise SystemExit(__doc__)
    if mode == "--self-test":
        return self_test()
    current, excerpts = collect()
    if mode == "--write":
        write(current, excerpts)
        return 0
    return check(current, read_baseline(), excerpts)


if __name__ == "__main__":
    raise SystemExit(main())
