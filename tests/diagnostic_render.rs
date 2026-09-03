//! What a diagnostic actually looks like, for every program in the repository
//! that produces one.
//!
//! # Why this exists
//!
//! M3 unifies five diagnostic types behind one model and separates the data
//! from its rendering. That is a refactor, and the thing a refactor of
//! diagnostics can most easily break is the diagnostics — silently, because the
//! conformance suite's 149 error fixtures assert only that *some* `❌` line
//! appeared and that the exit code was non-zero. A reworded message, a moved
//! column, a dropped note, a changed prefix, a diagnostic that stops being
//! printed at all: every one of those passes today.
//!
//! So this pins the bytes. For each fixture it runs the real `sz` binary and
//! hashes the complete stderr together with the exit code, into a committed
//! manifest. It is the M3 analogue of `tests/parser_snapshot.rs`, which pins
//! the frontend's *structured* diagnostics; between them, the data and the
//! rendering are both held.
//!
//! # What is pinned, and what is deliberately not
//!
//! Pinned: the full stderr byte stream, and the process exit code. Both are
//! user-visible contracts — `spec/cli.md` states the exit codes and
//! `spec/errors.md` states the rendered shape.
//!
//! Not pinned: stdout. A program that prints before failing is exercised by the
//! conformance suite's golden `.expected` files, and duplicating that here would
//! mean two places to update for one change.
//!
//! # Normalisation
//!
//! The absolute path of the fixture appears inside parser diagnostics, so it is
//! replaced with the bare file name — otherwise the manifest would only be
//! valid in one checkout directory. Line endings are normalised for the same
//! reason `tests/parser_snapshot.rs` normalises them.
//!
//! # Regenerating
//!
//! `SEREZ_SNAPSHOT_UPDATE=1 cargo test --test diagnostic_render`
//!
//! A difference here is a change to what a user sees. Read it, decide it was
//! intended, and say so in the commit — never regenerate to clear a red run.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Where the committed hashes live.
const MANIFEST: &str = "tests/snapshots/diagnostic_render.manifest";

/// Bumped when the manifest's *format* changes.
const SCHEMA: &str = "serez-diagnostic-render/1";

/// FNV-1a, 64-bit — same reasoning as `tests/parser_snapshot.rs`: fixed forever,
/// where `DefaultHasher` is explicitly not stable across Rust releases.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every fixture whose whole purpose is to fail.
///
/// `err_*` and `sec_*` are the conformance suite's two error categories: the
/// first covers ordinary failures, the second security gates and resource
/// ceilings. Both are defined by the runners as "must exit non-zero and print a
/// `❌`", which is exactly the assertion this file exists to strengthen.
fn fixtures() -> Vec<(String, PathBuf)> {
    let tests = crate_root().join("tests");
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&tests) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if path.extension().and_then(|e| e.to_str()) != Some("sz") {
            continue;
        }
        if name.starts_with("err_") || name.starts_with("sec_") {
            out.push((name.to_string(), path));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Run one fixture and return its exit code and normalised stderr.
fn run(name: &str, path: &Path) -> (i32, String) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_sz"))
        .arg(path)
        .current_dir(crate_root())
        .output()
        .unwrap_or_else(|e| panic!("could not run sz on {name}: {e}"));

    let stderr = String::from_utf8_lossy(&output.stderr)
        .replace("\r\n", "\n")
        // Parser diagnostics carry the path they were given, which is absolute
        // here and different in every checkout.
        .replace(&path.to_string_lossy().to_string(), name)
        .replace(&path.to_string_lossy().replace('\\', "/"), name);

    (output.status.code().unwrap_or(-1), stderr)
}

#[derive(PartialEq, Eq)]
struct Row {
    exit_code: i32,
    stderr_bytes: usize,
    stderr_hash: u64,
}

impl Row {
    fn render(&self, name: &str) -> String {
        format!(
            "{}\t{}\t{}\t{:016x}",
            name, self.exit_code, self.stderr_bytes, self.stderr_hash
        )
    }

    fn from_line(line: &str) -> Option<(String, Row)> {
        let mut fields = line.split('\t');
        let name = fields.next()?.to_string();
        let row = Row {
            exit_code: fields.next()?.parse().ok()?,
            stderr_bytes: fields.next()?.parse().ok()?,
            stderr_hash: u64::from_str_radix(fields.next()?, 16).ok()?,
        };
        if fields.next().is_some() {
            return None;
        }
        Some((name, row))
    }
}

fn measure() -> BTreeMap<String, (Row, String)> {
    let mut measured = BTreeMap::new();
    for (name, path) in fixtures() {
        let (exit_code, stderr) = run(&name, &path);
        let row = Row {
            exit_code,
            stderr_bytes: stderr.len(),
            stderr_hash: fnv1a64(stderr.as_bytes()),
        };
        measured.insert(name, (row, stderr));
    }
    measured
}

#[test]
fn every_failing_fixture_still_fails_the_same_way() {
    let measured = measure();
    assert!(
        measured.len() > 100,
        "only {} error fixtures found — a corpus that collapsed would pass this \
         file vacuously",
        measured.len()
    );

    let manifest_path = crate_root().join(MANIFEST);

    if std::env::var_os("SEREZ_SNAPSHOT_UPDATE").is_some() {
        let mut out = String::new();
        let _ = writeln!(out, "# {SCHEMA}");
        let _ = writeln!(
            out,
            "# stderr and exit code of every err_*.sz / sec_*.sz fixture."
        );
        let _ = writeln!(out, "# name<TAB>exit<TAB>stderr-bytes<TAB>fnv1a64(stderr)");
        let _ = writeln!(out, "# fixtures: {}", measured.len());
        for (name, (row, _)) in &measured {
            let _ = writeln!(out, "{}", row.render(name));
        }
        if let Some(parent) = manifest_path.parent() {
            std::fs::create_dir_all(parent).expect("cannot create the snapshot directory");
        }
        std::fs::write(&manifest_path, out).expect("cannot write the manifest");
        eprintln!("regenerated {MANIFEST} from {} fixtures", measured.len());
        return;
    }

    let existing = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!(
            "{MANIFEST} is missing ({e}). Create it with \
             SEREZ_SNAPSHOT_UPDATE=1 cargo test --test diagnostic_render"
        )
    });
    assert!(
        existing.starts_with(&format!("# {SCHEMA}")),
        "{MANIFEST} was written in a different manifest format than {SCHEMA}"
    );
    let expected: BTreeMap<String, Row> = existing
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(Row::from_line)
        .collect();

    let mut changed = Vec::new();
    for (name, (row, stderr)) in &measured {
        match expected.get(name) {
            None => changed.push(format!("  + {name} is not in the manifest")),
            Some(before) if before == row => {}
            Some(before) => {
                let mut what = Vec::new();
                if before.exit_code != row.exit_code {
                    what.push(format!(
                        "**exit code {} -> {}**",
                        before.exit_code, row.exit_code
                    ));
                }
                if before.stderr_hash != row.stderr_hash {
                    what.push(format!(
                        "stderr {} -> {} bytes",
                        before.stderr_bytes, row.stderr_bytes
                    ));
                }
                changed.push(format!(
                    "  ! {name}: {}\n{}",
                    what.join("; "),
                    indent(stderr)
                ));
            }
        }
    }
    for name in expected.keys() {
        if !measured.contains_key(name) {
            changed.push(format!("  - {name} is in the manifest but not on disk"));
        }
    }

    assert!(
        changed.is_empty(),
        "{} of {} error fixtures now produce different output.\n\n{}\n\n\
         Each of these is something a user sees. A refactor must not reach here. \
         If the change is intended, say so in the commit, then regenerate with:\n  \
         SEREZ_SNAPSHOT_UPDATE=1 cargo test --test diagnostic_render\n",
        changed.len(),
        measured.len(),
        changed.join("\n")
    );
}

/// The new stderr, indented, so a failure shows what it now says rather than
/// only that a hash moved.
fn indent(text: &str) -> String {
    text.lines()
        .map(|l| format!("      {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// No error fixture may be written in the unit-test framework's style.
///
/// `err_*.sz` and `sec_*.sz` are run **standalone** — the conformance runner
/// prepends `tests/framework.sz` only to `unit_*.sz` and `ai_*.sz`. So a fixture
/// in either of these categories that calls `test(…)` dies on its first line with
/// `Variable not found: test`, which satisfies the error-test contract — non-zero
/// exit, a `❌` line — and passes without running a single one of its assertions.
///
/// That is not hypothetical. `sec_crypto.sz`, `sec_crypto_ed25519.sz` and
/// `sec_tensor.sz` were all written this way, passed every run, and never
/// executed their 23 assertions; `docs/maturity/ROADMAP_STATE.md` §5.34 has the
/// evidence. The manifest beside this file even recorded all three with the same
/// 46-byte stderr and the same hash, and nobody read it as the signal it was.
///
/// The three were renamed to `unit_sec_*`, which is the category for
/// framework-based safety tests and which does receive the framework. This guard
/// is the part that stops it recurring: the fix repairs three files, the guard
/// covers every file added later.
#[test]
fn no_error_fixture_is_written_in_the_frameworks_style() {
    let offenders: Vec<String> = fixtures()
        .into_iter()
        .filter_map(|(name, path)| {
            let source = std::fs::read_to_string(&path).ok()?;
            source
                .lines()
                .any(|l| l.trim_start().starts_with("test("))
                .then_some(name)
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "{} error fixture(s) call test(…), which is only defined in \
         tests/framework.sz — and the runner does not prepend it to err_*/sec_*:\n  {}\n\n\
         Each of these passes by failing to find `test`, not by testing anything. \
         Rename to unit_sec_*.sz (framework-based) or rewrite without the framework.",
        offenders.len(),
        offenders.join("\n  ")
    );
}
