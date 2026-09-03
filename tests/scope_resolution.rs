//! How much real Serez code relies on names being resolved at run time.
//!
//! # The question this answers
//!
//! Serez has no static name resolution. `ScopeStack::lookup` walks a frame stack
//! that a function *call* pushes onto, so a body reading a name it does not
//! declare picks up whatever the caller holds. `docs/maturity/ROADMAP_STATE.md`
//! registers whether that should become a diagnostic as **DEC-M4-002**, and
//! records that the decision needs a number first: how much code actually
//! depends on it.
//!
//! `semantic::scopes::analyze` answers that per file, deliberately biased toward
//! *under*-reporting. This runs it over the whole in-repo corpus and turns the
//! per-file answer into the corpus-wide one.
//!
//! # What is asserted, and what is only reported
//!
//! The two are not the same and are treated differently, the way
//! `semantic_divergence` treats its two directions.
//!
//! **Asserted — these are correctness properties of the analysis itself:**
//!
//!   * It terminates and does not panic on any corpus file, including the ones
//!     that do not parse. An analysis that falls over on hostile input is not
//!     usable by a checker, and M9 would have to find this anyway.
//!   * Every span it reports points **inside the file it came from**. A
//!     diagnostic that names a position past the end of the source is the defect
//!     class M2 spent a milestone removing; reintroducing it in a new module
//!     would be a regression against that milestone.
//!   * The corpus does not silently collapse to nothing. A walk that finds no
//!     files would satisfy every other assertion here.
//!
//! **Reported, not asserted — this is the measurement:** how many files contain
//! a use that lexical structure cannot account for, split by where it sits. It
//! is printed rather than pinned because it moves whenever a fixture is added,
//! and pinning it would produce noise instead of signal. Run with
//! `cargo test --test scope_resolution -- --nocapture` to read it.
//!
//! # Why the number is a floor
//!
//! Files containing `import` are excluded, not counted: their names may
//! legitimately come from another module, and this analysis reads one file at a
//! time. Every other ambiguity in `semantic::scopes` also resolves toward
//! "bound". So the reported figure understates dynamic resolution, and a
//! decision taken on it is taken on a conservative estimate — which is the safe
//! direction for a measurement arguing that a change is affordable.

use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use serez_code::semantic::scopes::{self, UseKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Directories walked, relative to the crate root. The same set
/// `parser_snapshot` uses, so the two measure the same corpus.
const CORPUS_ROOTS: &[&str] = &["tests", "benchmarks", "std", "apps"];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn is_corpus_file(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("sz")
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
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((relative, path));
        }
    }
}

/// Whether the conformance runner prepends `tests/framework.sz` to this file
/// before running it.
///
/// This is not a detail — it decides whether the file is a *program* at all.
/// `run_tests.ps1:388` composes the framework, a newline and the source for
/// `tests/unit_*.sz` and `tests/ai_*.sz`, so those files call `test(...)` and
/// `summary()` without declaring them. Analysed alone they are fragments, and
/// the first run of this measurement duly reported `test` 2,003 times as an
/// unaccounted name — an artefact of the method, not a property of the language.
///
/// The exception mirrors the runner exactly: a `unit_*.sz` with a sibling
/// `.expected` is a golden test and runs standalone (`run_tests.ps1:633-637`).
fn needs_the_framework(relative: &str, path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    if !relative.starts_with("tests/") || relative.matches('/').count() != 1 {
        return false;
    }
    if !(name.starts_with("unit_") || name.starts_with("ai_")) {
        return false;
    }
    !path.with_extension("expected").exists()
}

fn corpus() -> Vec<(String, PathBuf)> {
    let root = crate_root();
    let mut files = Vec::new();
    for dir in CORPUS_ROOTS {
        collect(&root.join(dir), &root, &mut files);
    }
    // The official ecosystem packages are the strongest compatibility signal in
    // this repository and they live outside it, as sibling checkouts. Naming
    // them here would hard-code one machine's layout, so they are opt-in:
    //
    //   SEREZ_SCOPE_EXTRA_ROOTS="../serez-ui;../serez-http" cargo test --test scope_resolution -- --nocapture
    //
    // Absent the variable this measures the in-repo corpus alone, which is what
    // CI does and what the assertions below are calibrated for.
    if let Ok(extra) = std::env::var("SEREZ_SCOPE_EXTRA_ROOTS") {
        for entry in extra.split(';').filter(|e| !e.trim().is_empty()) {
            let dir = PathBuf::from(entry.trim());
            let base = dir.parent().unwrap_or(&dir).to_path_buf();
            collect(&dir, &base, &mut files);
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// What one file contributed to the measurement.
#[derive(Default)]
struct FileResult {
    /// Uses inside a named function, method or class — the caller-dependent
    /// case, and the one DEC-M4-002 is actually about.
    inside_a_function: usize,
    /// Uses at the file's top level, where nothing can be dynamically supplied.
    /// These would be a genuine `SZ4001` if the line executed.
    at_top_level: usize,
    by_kind: BTreeMap<&'static str, usize>,
}

fn kind_label(kind: UseKind) -> &'static str {
    match kind {
        UseKind::Read => "read",
        UseKind::Write => "write",
        UseKind::Call => "call",
        UseKind::Type => "type",
        UseKind::Parent => "parent",
    }
}

/// Stack for the measurement thread.
///
/// The corpus ships two depth-ceiling fixtures nested to the parser's 512-level
/// limit, and `scopes::analyze` descends the tree recursively like every other
/// frontend walk. `cargo test` gives its threads 2 MiB, which is not close to
/// enough: without this the run dies with `STATUS_STACK_OVERFLOW`.
///
/// `docs/maturity/ROADMAP_STATE.md` §5.15 records this exact wall, hit by
/// `frontend_robustness` and then by `parser_snapshot`, and was written so the
/// next frontend test would not rediscover it. This test rediscovered it anyway
/// on first run — which is worth noting rather than quietly fixing: a note in a
/// roadmap does not reach a person writing a new file. 16 MiB is enough here,
/// since this walks each tree once rather than three times.
///
/// **No product behaviour is involved.** The release binary parses and runs both
/// fixtures in every conformance run.
const MEASUREMENT_STACK: usize = 16 * 1024 * 1024;

#[test]
fn how_much_of_the_corpus_depends_on_dynamic_name_resolution() {
    std::thread::Builder::new()
        .stack_size(MEASUREMENT_STACK)
        .spawn(measure)
        .expect("spawn the measurement thread")
        .join()
        .expect("the measurement thread panicked")
}

fn measure() {
    let files = corpus();
    assert!(
        files.len() > 400,
        "the corpus collapsed to {} files — a walk that finds nothing would satisfy \
         every other assertion in this test",
        files.len()
    );

    let mut analysed = 0usize;
    let mut framework_composed = 0usize;
    let mut skipped_for_imports = 0usize;
    let mut unparsed = 0usize;
    let mut files_with_free_uses = 0usize;
    let mut totals = FileResult::default();
    let mut worst: Vec<(usize, String)> = Vec::new();
    let mut names: BTreeMap<String, usize> = BTreeMap::new();

    let framework = std::fs::read_to_string(crate_root().join("tests/framework.sz"))
        .expect("tests/framework.sz is the harness the runner prepends; it must exist");

    for (relative, path) in &files {
        let Ok(mut source) = std::fs::read_to_string(path) else {
            continue;
        };
        if needs_the_framework(relative, path) {
            framework_composed += 1;
            source = format!(
                "{framework}
{source}"
            );
        }
        let line_count = source.lines().count().max(1);
        let byte_len = source.len();

        let mut parser = Parser::new(Lexer::new(source));
        let program = parser.parse_program();

        // A file that does not parse still has to be walkable: the analysis runs
        // on whatever tree recovery produced. Not skipping these is the point —
        // it is where a panic would live.
        if !parser.take_errors().is_empty() {
            unparsed += 1;
        }

        let report = scopes::analyze(&program);

        for use_site in &report.free {
            assert!(
                use_site.span.line >= 1 && use_site.span.line <= line_count,
                "{relative}: reported '{}' at line {}, but the file has {line_count} lines",
                use_site.name,
                use_site.span.line
            );
            assert!(
                use_site.span.start <= byte_len,
                "{relative}: reported '{}' at byte offset {}, past the file's {byte_len} bytes",
                use_site.name,
                use_site.span.start
            );
        }

        if !report.is_conclusive() {
            skipped_for_imports += 1;
            continue;
        }
        analysed += 1;

        if report.free.is_empty() {
            continue;
        }
        files_with_free_uses += 1;

        let mut here = FileResult::default();
        for use_site in &report.free {
            if use_site.enclosing.is_some() {
                here.inside_a_function += 1;
                totals.inside_a_function += 1;
            } else {
                here.at_top_level += 1;
                totals.at_top_level += 1;
            }
            *here.by_kind.entry(kind_label(use_site.kind)).or_default() += 1;
            *totals.by_kind.entry(kind_label(use_site.kind)).or_default() += 1;
            *names.entry(use_site.name.clone()).or_default() += 1;
        }
        worst.push((here.inside_a_function + here.at_top_level, relative.clone()));
    }

    worst.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut ranked: Vec<(usize, String)> = names.into_iter().map(|(n, c)| (c, n)).collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));

    println!("\n── dynamic name resolution across the corpus ──");
    println!("corpus files                : {}", files.len());
    println!("  framework prepended       : {framework_composed} (as the runner does)");
    println!("  of which do not parse     : {unparsed} (still analysed — that is the point)");
    println!("  excluded, contain import  : {skipped_for_imports}");
    println!("conclusively analysed       : {analysed}");
    println!(
        "  with an unaccounted use   : {files_with_free_uses} ({:.1}%)",
        100.0 * files_with_free_uses as f64 / analysed.max(1) as f64
    );
    println!("unaccounted uses, by position:");
    println!("  inside a function/method  : {}", totals.inside_a_function);
    println!("  at the file's top level   : {}", totals.at_top_level);
    println!("unaccounted uses, by kind   : {:?}", totals.by_kind);
    println!("most affected files:");
    for (count, name) in worst.iter().take(10) {
        println!("  {count:5}  {name}");
    }
    println!("most frequent names:");
    for (count, name) in ranked.iter().take(15) {
        println!("  {count:5}  {name}");
    }
    println!("───────────────────────────────────────────────\n");
}
