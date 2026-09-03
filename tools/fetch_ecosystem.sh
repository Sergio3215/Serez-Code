#!/usr/bin/env bash
# Clone the official packages next to this repository, for the ecosystem canary.
#
# `run_ecosystem.sh` expects them as sibling checkouts (`../serez-ui`, …), which
# is how a developer's machine is laid out and is why the canary has only ever
# run locally. This puts a CI runner in the same shape.
#
#   tools/fetch_ecosystem.sh            # at the commits in ecosystem-pins.txt
#   tools/fetch_ecosystem.sh --latest   # at each package's default branch
#
# The pinned form is the CI **gate** (DEC-M10-001): only a change in this
# repository can turn it red. The `--latest` form is the daily drift check, which
# answers a different question and cannot block a PR. `ecosystem-pins.txt` says
# why both exist.
#
# Exit code: 0 when every package was fetched, 1 otherwise. A package that cannot
# be cloned is a **failure**, not a skip — a canary that quietly tests seven of
# eight packages is worse than one that says it could not run.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PARENT="$(dirname "$ROOT")"
PINS="$ROOT/ecosystem-pins.txt"
REMOTE_BASE="${SEREZ_ECOSYSTEM_BASE:-https://github.com/Sergio3215}"

LATEST=0
[ "${1:-}" = "--latest" ] && LATEST=1

[ -f "$PINS" ] || { echo "missing $PINS" >&2; exit 1; }

failed=0
fetched=0
while IFS=$'\t' read -r package commit; do
    case "$package" in ''|\#*) continue ;; esac
    dest="$PARENT/$package"

    if [ -d "$dest/.git" ]; then
        echo "-- $package: already present at $dest"
        fetched=$((fetched + 1))
        continue
    fi

    echo "-- $package: cloning"
    if ! git clone --quiet "$REMOTE_BASE/$package.git" "$dest"; then
        echo "   FAILED to clone $package" >&2
        failed=$((failed + 1))
        continue
    fi

    if [ "$LATEST" -eq 0 ]; then
        if ! git -C "$dest" checkout --quiet "$commit"; then
            echo "   FAILED to check out $package at $commit" >&2
            echo "   The pin names a commit the remote does not have. Either the" >&2
            echo "   history was rewritten or the pin was never pushed." >&2
            failed=$((failed + 1))
            continue
        fi
        echo "   at $commit (pinned)"
    else
        echo "   at $(git -C "$dest" rev-parse --short HEAD) (default branch)"
    fi
    fetched=$((fetched + 1))
done < "$PINS"

echo
if [ "$failed" -gt 0 ]; then
    echo "$failed package(s) could not be fetched; the canary would test fewer" >&2
    echo "packages than it claims to. Failing instead." >&2
    exit 1
fi
echo "$fetched package(s) ready in $PARENT"
