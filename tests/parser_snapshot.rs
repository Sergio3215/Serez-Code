//! Differential snapshot of what the parser produces, over the whole in-repo
//! Serez corpus.
//!
//! # Why this exists
//!
//! The conformance suite asserts what a *program prints*. That is the right
//! contract for the language and the wrong instrument for a parser refactor: a
//! node nested under the wrong parent, an argument list built in the wrong
//! order, or a diagnostic reworded can each leave every printed byte identical.
//! Sixty-three of the error tests assert only that some `❌` line appeared, so
//! most parser wording is not pinned by anything at all.
//!
//! So this pins the parser's two actual outputs — the tree and the diagnostic
//! list — for every `.sz` file the repository ships, and fails if either moves.
//! It is the safety net the M1 parser extraction runs behind; see
//! `docs/maturity/ROADMAP_STATE.md`.
//!
//! # What is pinned
//!
//! Per file: the `Debug` rendering of the `Program`, and every `ParseError`
//! rendered as `code|line|column|message`. Both are hashed into
//! `tests/snapshots/parser_ast.manifest`, which is committed.
//!
//! Hashes rather than whole trees, because the corpus renders to tens of
//! megabytes and a manifest nobody can read in review is not evidence. When a
//! hash does move, the failure writes that file's complete pretty-printed tree
//! and diagnostics under `target/parser_snapshot/` and names the path, so the
//! actual difference is one `diff` away.
//!
//! # Deliberately not a dependency
//!
//! The hash is FNV-1a, written out below in six lines. `DefaultHasher` is the
//! obvious alternative and the wrong one: its output is explicitly not stable
//! across Rust releases, so a committed manifest would start failing on a
//! toolchain upgrade rather than on a real change — on three operating systems
//! at once.
//!
//! # Regenerating
//!
//! `SEREZ_SNAPSHOT_UPDATE=1 cargo test --test parser_snapshot`
//!
//! Regenerate only after reading the reported difference and deciding the new
//! parse is the correct one. An unexplained regeneration silently discards the
//! contract this file exists to hold.

use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Directories walked for corpus files, relative to the crate root.
const CORPUS_ROOTS: &[&str] = &["tests", "benchmarks", "std", "apps"];

/// Where the committed hashes live.
const MANIFEST: &str = "tests/snapshots/parser_ast.manifest";

/// Bumped when the manifest's *format* changes, so a stale manifest fails with
/// an explanation instead of several hundred confusing hash mismatches.
const SCHEMA: &str = "serez-parser-snapshot/1";

/// Stack for the thread the measurement runs on.
///
/// The corpus deliberately contains source that reaches the parser's depth
/// ceiling — `tests/err_parse_depth_chain.sz` and
/// `tests/err_parse_depth_nesting.sz` exist precisely to prove a 512-level tree
/// produces `SZ2001` instead of a crash. Walking one of those trees costs far
/// more stack in a debug build than in the shipped release binary:
/// `parse_expression` is one enormous `match` whose unoptimized frame measures
/// around 8 KiB against roughly 256 bytes when optimized, and this file walks
/// each tree three times over — the parser builds it, `Debug` renders it, and
/// the drop glue tears it down, each recursing once per level.
///
/// `cargo test` gives its threads 2 MiB, which is not close to enough: without
/// this the run dies with `STATUS_STACK_OVERFLOW` on the first depth fixture.
/// `tests/frontend_robustness.rs` hit the same wall and answered it the same
/// way; this is that answer with headroom for the two extra passes.
///
/// None of this is a property of the language. The release binary parses both
/// fixtures in the conformance suite every run.
const MEASUREMENT_STACK: usize = 32 * 1024 * 1024;

/// FNV-1a, 64-bit. Chosen for being fixed forever rather than for being fast.
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

/// Is this a corpus file?
///
/// The two exclusions mirror `.gitignore` rather than inventing a second rule:
/// `tests/_*.sz` is local scratch (bug hunts, one-off probes) and
/// `tests/~unit_temp*.sz` is the per-run scratch file both conformance runners
/// write next to the tests. Neither is committed, both appear on a developer's
/// disk, and including either would make this test fail on whichever machine
/// had most recently run the suite.
///
/// The `~` rule also excludes exactly one file that *is* committed:
/// `tests/~tmp_test.sz`. It is not a test. It is a captured runner temp file
/// from 2026-06-19 — `framework.sz` with the body of what is now
/// `unit_dict_advanced.sz` appended — that no runner, script or document
/// references, and that two commits have "restored" after glob cleanups removed
/// it, on the assumption it was needed. Pinning the parse of an artifact nobody
/// runs would only mean a spurious failure the day somebody finally deletes it.
/// So the corpus is 490 files rather than the 491 `git ls-files '*.sz'` reports,
/// and the missing one is that.
fn is_corpus_file(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("sz") {
        return false;
    }
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    !name.starts_with('_') && !name.starts_with('~')
}

/// Repository-relative path with forward slashes, so the manifest is identical
/// on Windows and on Unix.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn collect(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let skip = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with('.') || n == "target");
            if !skip {
                collect(&path, root, out);
            }
        } else if is_corpus_file(&path) {
            out.push((relative(root, &path), path));
        }
    }
}

fn corpus() -> Vec<(String, PathBuf)> {
    let root = crate_root();
    let mut files = Vec::new();
    for dir in CORPUS_ROOTS {
        collect(&root.join(dir), &root, &mut files);
    }
    // read_dir order is filesystem order, which differs between machines.
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// What the parser produced for one file.
struct Parsed {
    /// Compact `Debug` of the `Program`. Compact rather than pretty because it
    /// is rendered for every corpus file on every run; the pretty form is
    /// produced only for the files that actually differ.
    tree: String,
    diagnostics: String,
    diagnostic_count: usize,
}

fn parse(relative_path: &str, source: String) -> Parsed {
    let lines: Vec<String> = source.lines().map(str::to_string).collect();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer);
    parser.set_source(lines);
    parser.set_source_name(relative_path);
    let program = parser.parse_program();

    let errors = parser.take_errors();
    let mut diagnostics = String::new();
    for error in &errors {
        let _ = writeln!(
            diagnostics,
            "{}|{}|{}|{}",
            error.code, error.line, error.column, error.message
        );
    }

    Parsed {
        tree: format!("{:?}", program),
        diagnostics,
        diagnostic_count: errors.len(),
    }
}

/// One manifest row: everything pinned about one file.
#[derive(PartialEq, Eq)]
struct Row {
    tree_bytes: usize,
    diagnostic_count: usize,
    tree_hash: u64,
    diagnostic_hash: u64,
}

impl Row {
    fn render(&self, path: &str) -> String {
        format!(
            "{}\t{}\t{}\t{:016x}\t{:016x}",
            path, self.tree_bytes, self.diagnostic_count, self.tree_hash, self.diagnostic_hash
        )
    }

    fn from_line(line: &str) -> Option<(String, Row)> {
        let mut fields = line.split('\t');
        let path = fields.next()?.to_string();
        let row = Row {
            tree_bytes: fields.next()?.parse().ok()?,
            diagnostic_count: fields.next()?.parse().ok()?,
            tree_hash: u64::from_str_radix(fields.next()?, 16).ok()?,
            diagnostic_hash: u64::from_str_radix(fields.next()?, 16).ok()?,
        };
        if fields.next().is_some() {
            return None;
        }
        Some((path, row))
    }
}

fn measure() -> BTreeMap<String, (Row, PathBuf)> {
    let mut measured = BTreeMap::new();
    for (path, absolute) in corpus() {
        let source = std::fs::read_to_string(&absolute)
            .unwrap_or_else(|e| panic!("corpus file {path} is unreadable: {e}"));
        let parsed = parse(&path, source);
        let row = Row {
            tree_bytes: parsed.tree.len(),
            diagnostic_count: parsed.diagnostic_count,
            tree_hash: fnv1a64(parsed.tree.as_bytes()),
            diagnostic_hash: fnv1a64(parsed.diagnostics.as_bytes()),
        };
        measured.insert(path, (row, absolute));
    }
    measured
}

fn render_manifest(measured: &BTreeMap<String, (Row, PathBuf)>) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {SCHEMA}");
    let _ = writeln!(
        out,
        "# Parser output for every .sz file under {}.",
        CORPUS_ROOTS.join(", ")
    );
    let _ = writeln!(
        out,
        "# path<TAB>tree-bytes<TAB>diagnostics<TAB>fnv1a64(tree)<TAB>fnv1a64(diagnostics)"
    );
    let _ = writeln!(out, "# files: {}", measured.len());
    for (path, (row, _)) in measured {
        let _ = writeln!(out, "{}", row.render(path));
    }
    out
}

fn read_manifest(text: &str) -> BTreeMap<String, Row> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.trim().is_empty())
        .filter_map(Row::from_line)
        .collect()
}

/// Write one file's complete parse result where a human can diff it.
fn dump(path: &str, absolute: &Path) -> PathBuf {
    let target = crate_root()
        .join("target")
        .join("parser_snapshot")
        .join(path.replace('/', "__"));
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let source = std::fs::read_to_string(absolute).unwrap_or_default();
    let lines: Vec<String> = source.lines().map(str::to_string).collect();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer);
    parser.set_source(lines);
    parser.set_source_name(path);
    let program = parser.parse_program();

    let mut text = format!("== {path} ==\n\n-- diagnostics --\n");
    for error in parser.take_errors() {
        let _ = writeln!(
            text,
            "{}|{}|{}|{}",
            error.code, error.line, error.column, error.message
        );
    }
    let _ = write!(text, "\n-- tree --\n{:#?}\n", program);
    let _ = std::fs::write(&target, text);
    target
}

#[test]
fn the_corpus_the_snapshot_walks_is_the_one_it_claims_to_walk() {
    let files = corpus();
    assert!(
        files.len() > 400,
        "the corpus collapsed to {} files — a walk that finds nothing would pass \
         every other assertion here vacuously",
        files.len()
    );
    for (path, _) in &files {
        assert!(
            !path.contains('\\') && !path.contains('\t'),
            "path {path:?} would corrupt the tab-separated manifest"
        );
    }
}

/// Run `body` on a thread with [`MEASUREMENT_STACK`], propagating its result.
fn on_measurement_stack<T: Send + 'static>(body: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(MEASUREMENT_STACK)
        .spawn(body)
        .expect("spawn the snapshot thread")
        .join()
        .expect("the snapshot thread panicked")
}

#[test]
fn the_parser_produces_the_trees_and_diagnostics_it_produced_before() {
    on_measurement_stack(compare_against_the_manifest)
}

#[test]
fn the_manifest_does_not_depend_on_what_the_checkout_did_to_line_endings() {
    // The manifest is committed, and CI runs on three operating systems. The
    // GitHub Actions Windows runner checks out with `core.autocrlf=true`, so
    // every corpus file reaches the parser as CRLF there and as LF on Linux and
    // macOS. If a carriage return could survive into the tree — inside a string
    // literal spanning a line break, say — the hashes would be
    // platform-dependent and this suite would fail on exactly one of the three
    // runners, for a reason that would look nothing like line endings.
    //
    // The lexer skips '\r' as whitespace (`skip_whitespace`), so it should not.
    // "Should not" is not evidence, so this parses the whole corpus both ways
    // and compares. It runs on the same large stack as the snapshot: the depth
    // fixtures are in here too.
    on_measurement_stack(|| {
        let mut differing = Vec::new();
        for (path, absolute) in corpus() {
            let raw = std::fs::read_to_string(&absolute)
                .unwrap_or_else(|e| panic!("corpus file {path} is unreadable: {e}"));
            let lf = raw.replace("\r\n", "\n");
            let crlf = lf.replace('\n', "\r\n");

            let as_lf = parse(&path, lf);
            let as_crlf = parse(&path, crlf);
            if as_lf.tree != as_crlf.tree || as_lf.diagnostics != as_crlf.diagnostics {
                differing.push(path);
            }
        }
        assert!(
            differing.is_empty(),
            "{} corpus files parse differently under CRLF than under LF, so the \
             committed manifest is only valid on whichever platform generated \
             it:\n  {}",
            differing.len(),
            differing.join("\n  ")
        );
    })
}

fn compare_against_the_manifest() {
    let measured = measure();
    let manifest_path = crate_root().join(MANIFEST);

    if std::env::var_os("SEREZ_SNAPSHOT_UPDATE").is_some() {
        if let Some(parent) = manifest_path.parent() {
            std::fs::create_dir_all(parent).expect("cannot create the snapshot directory");
        }
        std::fs::write(&manifest_path, render_manifest(&measured))
            .expect("cannot write the manifest");
        eprintln!(
            "regenerated {MANIFEST} from {} corpus files",
            measured.len()
        );
        return;
    }

    let existing = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!(
            "{MANIFEST} is missing ({e}). Create it with \
             SEREZ_SNAPSHOT_UPDATE=1 cargo test --test parser_snapshot"
        )
    });
    assert!(
        existing.starts_with(&format!("# {SCHEMA}")),
        "{MANIFEST} was written in a different manifest format than {SCHEMA}"
    );
    let expected = read_manifest(&existing);

    let mut changed: Vec<String> = Vec::new();
    let mut dumped: Vec<PathBuf> = Vec::new();

    for (path, (row, absolute)) in &measured {
        match expected.get(path) {
            None => changed.push(format!(
                "  + {path} is not in the manifest (new corpus file)"
            )),
            Some(before) if before == row => {}
            Some(before) => {
                let mut what = Vec::new();
                if before.tree_hash != row.tree_hash {
                    what.push(format!(
                        "tree {:016x} -> {:016x} ({} -> {} bytes)",
                        before.tree_hash, row.tree_hash, before.tree_bytes, row.tree_bytes
                    ));
                }
                if before.diagnostic_hash != row.diagnostic_hash {
                    what.push(format!(
                        "diagnostics {:016x} -> {:016x} ({} -> {})",
                        before.diagnostic_hash,
                        row.diagnostic_hash,
                        before.diagnostic_count,
                        row.diagnostic_count
                    ));
                }
                changed.push(format!("  ! {path}: {}", what.join("; ")));
                dumped.push(dump(path, absolute));
            }
        }
    }
    for path in expected.keys() {
        if !measured.contains_key(path) {
            changed.push(format!("  - {path} is in the manifest but not on disk"));
        }
    }

    if !changed.is_empty() {
        let dumps = if dumped.is_empty() {
            String::from("  (nothing dumped: the corpus itself changed)")
        } else {
            dumped
                .iter()
                .map(|p| format!("  {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        };
        panic!(
            "the parser no longer produces the same output for {} of {} corpus files.\n\n\
             {}\n\n\
             Full trees and diagnostics for the changed files:\n{}\n\n\
             A refactor must not reach here. If the change is intended, read the \
             difference first, then regenerate with:\n  \
             SEREZ_SNAPSHOT_UPDATE=1 cargo test --test parser_snapshot\n",
            changed.len(),
            measured.len(),
            changed.join("\n"),
            dumps
        );
    }
}
