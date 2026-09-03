#!/usr/bin/env python3
"""Clippy as a gate that debt cannot grow through.

DEC-M10-002. CI ran `cargo clippy --all-targets` **without** `-D warnings`, so
its warnings failed nothing and a new one was invisible. This roadmap worked
around that by comparing the per-site list by hand at every milestone boundary
(ROADMAP_STATE.md §5.26) — a discipline that existed because the gate did not.
It moved twice in eleven milestones, and both times a person caught it.

The policy is `current <= baseline`, per lint and per file:

    tools/clippy_baseline.py --check      # CI: fail if any pair grew or is new
    tools/clippy_baseline.py --write      # refresh after fixing warnings

WHY (lint, file) AND NOT A TOTAL
--------------------------------
A single number can stay still while one warning is fixed and a different one is
introduced, which is exactly the drift the gate exists to catch. Keying on the
lint *and* the file makes those two events visible as one decrease and one
increase.

Line numbers are deliberately **not** part of the key. They move whenever
anything above them is edited, so a baseline keyed on them would need refreshing
after every unrelated change and would be ignored within a week.

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
before being counted, and paths are normalised to forward slashes, so the
baseline is identical on Linux, macOS and Windows.
"""

from __future__ import annotations

import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

BASELINE = Path(__file__).resolve().parent.parent / "clippy-baseline.txt"

HEADER = [
    "# serez-clippy-baseline/1",
    "# Known Clippy debt, as `<lint>\\t<file>\\t<count>`, sorted.",
    "#",
    "# The gate is `current <= baseline` per line: a pair that grows, or a pair",
    "# that is not here at all, fails CI. Fixing warnings is always allowed and",
    "# is what `tools/clippy_baseline.py --write` is for.",
    "#",
    "# Sites are deduplicated by (lint, file, line, column) before counting, and",
    "# paths use forward slashes, so this file is identical on all three runners.",
]


def collect() -> Counter:
    """Every Clippy warning site, counted by (lint, file)."""
    result = subprocess.run(
        [
            "cargo",
            "clippy",
            "--all-targets",
            "--message-format=json",
        ],
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

    sites: set[tuple[str, str, int, int]] = set()
    for line in result.stdout.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError:
            continue
        message = payload.get("message")
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
        sites.add((code, path, span["line_start"], span["column_start"]))

    counted: Counter = Counter()
    for code, path, _, _ in sites:
        counted[(code, path)] += 1
    return counted


def read_baseline() -> Counter:
    if not BASELINE.exists():
        raise SystemExit(
            f"no baseline at {BASELINE}; run tools/clippy_baseline.py --write"
        )
    counted: Counter = Counter()
    for raw in BASELINE.read_text(encoding="utf-8").splitlines():
        if not raw or raw.startswith("#"):
            continue
        lint, path, count = raw.split("\t")
        counted[(lint, path)] = int(count)
    return counted


def write(counted: Counter) -> None:
    lines = list(HEADER)
    lines.append(f"# total sites: {sum(counted.values())}")
    for (lint, path), count in sorted(counted.items()):
        lines.append(f"{lint}\t{path}\t{count}")
    BASELINE.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {BASELINE} — {sum(counted.values())} sites in {len(counted)} pairs")


def check(current: Counter, baseline: Counter) -> int:
    grew = []
    new = []
    for key, count in sorted(current.items()):
        allowed = baseline.get(key, 0)
        if allowed == 0:
            new.append((key, count))
        elif count > allowed:
            grew.append((key, allowed, count))

    if new or grew:
        print("Clippy debt grew. The gate is `current <= baseline`, per lint and file.\n")
        for (lint, path), count in new:
            print(f"  NEW   {lint}  {path}  ({count})")
        for (lint, path), was, now in grew:
            print(f"  GREW  {lint}  {path}  {was} -> {now}")
        print(
            "\nFix the warning, or — if it is genuinely accepted debt — say so in the\n"
            "commit and refresh with `tools/clippy_baseline.py --write`. Refreshing\n"
            "to silence a real regression is the one thing this gate exists to stop."
        )
        return 1

    fixed = sum(
        allowed - current.get(key, 0)
        for key, allowed in baseline.items()
        if current.get(key, 0) < allowed
    )
    total = sum(current.values())
    print(f"Clippy debt within baseline: {total} sites in {len(current)} pairs.")
    if fixed:
        print(
            f"{fixed} site(s) fewer than the baseline — run --write to bank the "
            "reduction so it cannot come back."
        )
    return 0


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "--check"
    if mode not in ("--check", "--write"):
        raise SystemExit(__doc__)
    current = collect()
    if mode == "--write":
        write(current)
        return 0
    return check(current, read_baseline())


if __name__ == "__main__":
    raise SystemExit(main())
