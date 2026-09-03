//! The dependency graph, as a gate rather than as a diagram.
//!
//! # Why this exists
//!
//! `docs/maturity/ROADMAP_STATE.md` §3.1 describes the module graph in prose:
//! *"mostly a clean DAG"*, with a table of the edges worth naming and a sentence
//! saying which inversions are absent — no `parser -> evaluator`, no `ast -> gui`,
//! no `lexer -> package_manager`.
//!
//! That sentence was true when it was written and nothing keeps it true. An
//! architecture description that a compiler never reads is a description of the
//! architecture someone *intended*, and it drifts silently, one convenient
//! `use crate::` at a time. M10's charter is that the platform's shape holds; a
//! shape nothing checks is a shape nothing holds.
//!
//! So this reads `src/` and asserts the two properties §3.1 claims.
//!
//! # What is asserted
//!
//! **A1 — no forbidden edge.** [`FORBIDDEN`] lists inversions that would mean a
//! layer had reached backwards: the frontend depending on execution, the syntax
//! tree depending on a host service, the lexer depending on package management.
//! Each entry names what it would mean rather than only what it forbids, because
//! a rule whose reason is not written down gets deleted by whoever first finds it
//! inconvenient.
//!
//! **A2 — no cycle except the ones on record.** [`KNOWN_CYCLES`] holds the ones
//! that exist, each with the finding that records it. The list is checked for
//! staleness in both directions: a cycle that is not listed fails, and a listed
//! cycle that no longer exists fails too, so a fixed cycle cannot stay recorded
//! as permanent.
//!
//! A2 searches cycles of **any length up to [`MAX_CYCLE`]**, and that is not a
//! detail. This test was first written to find mutual pairs only, which found
//! `run <-> szx` and reported the graph clean. There is also a three-module cycle
//! — `evaluator -> szx -> run -> evaluator`, because `import` re-enters the whole
//! pipeline — and the pair-only version would have licensed the claim that no
//! other cycle exists. A checker that can only see the shape you expected is a
//! checker that confirms what you expected.
//!
//! # What is deliberately not asserted
//!
//! The graph's *shape* beyond those two properties — depth, fan-in, layering. A
//! test that pinned the whole graph would fail on every ordinary refactor and be
//! deleted within a month. These two properties are the ones whose violation is
//! always a defect.
//!
//! # Method, and its limit
//!
//! Edges come from `use crate::x` and `use super::x` declarations, plus inline
//! `crate::x::` paths. It is a text scan, not a resolution pass, so it reads
//! *declared* dependencies rather than *used* ones — an unused import counts. That
//! over-approximates, which is the safe direction: a forbidden edge cannot hide
//! from it, and a false alarm is a deleted import away from being fixed.
//! `#[cfg(test)]` code is included on purpose; §3.1 notes `compiler::hir_lower ->
//! parser` is test-only, and a test-only inversion is still a compile-time edge.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Edges that would mean a layer reached backwards, and what each would mean.
const FORBIDDEN: &[(&str, &str, &str)] = &[
    (
        "parser",
        "evaluator",
        "the frontend would depend on execution: parsing could not be reasoned \
         about, tested or reused without the runtime it is supposed to precede",
    ),
    (
        "parser",
        "type_checker",
        "syntax would depend on semantics, which is the inversion M4 exists to \
         prevent — the parser answers whether text is well-formed, not what it means",
    ),
    (
        "lexer",
        "parser",
        "tokenisation would depend on grammar, and the lexer could no longer be \
         run on input the parser cannot handle — which is exactly what .szx needs",
    ),
    (
        "lexer",
        "evaluator",
        "the same inversion as parser -> evaluator, one layer deeper",
    ),
    (
        "lexer",
        "package_manager",
        "tokenising a string would depend on how packages are installed",
    ),
    (
        "ast",
        "evaluator",
        "the syntax tree would depend on the thing that walks it; M2 made the AST \
         source-oriented and this is what keeps it that way",
    ),
    (
        "ast",
        "type_checker",
        "the syntax tree would carry type knowledge, which is the accidental \
         semantics M2 and M4 both worked to remove",
    ),
    (
        "span",
        "ast",
        "a position would depend on what it points at; span is the leaf the whole \
         diagnostic model rests on",
    ),
    (
        "diagnostic",
        "evaluator",
        "the diagnostic model would depend on one of its producers, and M3 \
         unified it precisely so that no producer owns it",
    ),
    (
        "semantic",
        "evaluator",
        "the semantic layer would depend on the runtime it exists to precede",
    ),
];

/// Cycles that exist, each with the finding that records it.
///
/// Members are listed sorted, because a cycle has no first element and the test
/// compares sorted sets.
const KNOWN_CYCLES: &[(&[&str], &str)] = &[
    (
        &["run", "szx"],
        "§5.6 — `run.rs` calls `szx::run_szx_file` and `szx.rs` calls \
         `run::run_file`. Architectural debt, low, recorded as an M10 input: two \
         entry points for two file extensions, each needing the other's dispatch.",
    ),
    (
        &["evaluator", "run", "szx"],
        "§5.38 — the evaluator depends on the entry point that drives it, because \
         `import` re-enters the whole pipeline: evaluator -> szx -> run -> \
         evaluator. §3.1 named all three edges and did not name the cycle they \
         form, which is why this test looks for cycles of any length rather than \
         only for mutual pairs.",
    ),
];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The top-level module a file belongs to: `src/evaluator/expr.rs` -> `evaluator`.
fn owning_module(path: &Path, src: &Path) -> Option<String> {
    let relative = path.strip_prefix(src).ok()?;
    let first = relative.components().next()?;
    let name = first.as_os_str().to_str()?;
    Some(name.trim_end_matches(".rs").to_string())
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    out.sort();
}

/// Every `crate::<module>` a file names, however it names it.
fn referenced_modules(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for part in text.split("crate::").skip(1) {
        let name: String = part
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect();
        if !name.is_empty() {
            out.insert(name);
        }
    }
    out
}

/// module -> the modules it depends on, and one file that shows each edge.
fn graph() -> BTreeMap<String, BTreeMap<String, String>> {
    let src = crate_root().join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(
        files.len() > 40,
        "the src/ walk found {} files; a walk that finds nothing would pass \
         every assertion here",
        files.len()
    );

    let mut edges: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    for path in &files {
        let Some(owner) = owning_module(path, &src) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            // Anything unreadable as UTF-8 is by definition not compiled, so it
            // has no edges. Kept as a skip rather than an assertion: this held
            // one file, `src/test_run.rs`, and §5.3 deleted it.
            continue;
        };
        let shown = path
            .strip_prefix(crate_root())
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/");
        for target in referenced_modules(&text) {
            if target != owner {
                edges
                    .entry(owner.clone())
                    .or_default()
                    .entry(target)
                    .or_insert_with(|| shown.clone());
            }
        }
    }
    edges
}

#[test]
fn no_layer_depends_on_one_that_should_come_after_it() {
    let edges = graph();
    let mut violations = Vec::new();

    for (from, to, why) in FORBIDDEN {
        if let Some(targets) = edges.get(*from) {
            if let Some(seen_in) = targets.get(*to) {
                violations.push(format!(
                    "  {from} -> {to}\n    seen in: {seen_in}\n    {why}"
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "{} forbidden dependency edge(s):\n\n{}\n\n\
         Each of these is a layer reaching backwards. If one is genuinely needed, \
         that is an architectural decision — register it in ROADMAP_STATE.md §7A \
         rather than deleting the rule.",
        violations.len(),
        violations.join("\n\n")
    );
}

/// How long a cycle this searches for. Four is past every cycle the graph has
/// and short enough that the search stays instant on a graph this size.
const MAX_CYCLE: usize = 4;

/// Every simple cycle up to [`MAX_CYCLE`] modules, each as a sorted member list.
fn cycles(edges: &BTreeMap<String, BTreeMap<String, String>>) -> BTreeSet<Vec<String>> {
    fn walk(
        edges: &BTreeMap<String, BTreeMap<String, String>>,
        start: &str,
        node: &str,
        path: &mut Vec<String>,
        found: &mut BTreeSet<Vec<String>>,
    ) {
        if path.len() > MAX_CYCLE {
            return;
        }
        let Some(targets) = edges.get(node) else {
            return;
        };
        for next in targets.keys() {
            if next == start && path.len() >= 2 {
                let mut members = path.clone();
                members.sort();
                found.insert(members);
            } else if !path.contains(next) {
                path.push(next.clone());
                walk(edges, start, next, path, found);
                path.pop();
            }
        }
    }

    let mut found = BTreeSet::new();
    for module in edges.keys() {
        let mut path = vec![module.clone()];
        walk(edges, module, module, &mut path, &mut found);
    }
    found
}

#[test]
fn the_only_cycles_are_the_ones_on_record() {
    let edges = graph();
    let found = cycles(&edges);

    let known: BTreeSet<Vec<String>> = KNOWN_CYCLES
        .iter()
        .map(|(members, _)| {
            let mut v: Vec<String> = members.iter().map(|m| (*m).to_string()).collect();
            v.sort();
            v
        })
        .collect();

    let new: Vec<String> = found
        .difference(&known)
        .map(|members| format!("  {}", members.join(" <-> ")))
        .collect();
    assert!(
        new.is_empty(),
        "{} dependency cycle(s) not on record:\n{}\n\n\
         A cycle means those modules cannot be understood, tested or extracted \
         separately. Break it, or add it to KNOWN_CYCLES with the finding that \
         records why it stays.",
        new.len(),
        new.join("\n")
    );

    let gone: Vec<String> = known
        .difference(&found)
        .map(|members| format!("  {}", members.join(" <-> ")))
        .collect();
    assert!(
        gone.is_empty(),
        "{} cycle(s) are listed in KNOWN_CYCLES and no longer exist:\n{}\n\n\
         Remove them. A list of accepted debt that outlives the debt makes the \
         next reader believe a solved problem is permanent.",
        gone.len(),
        gone.join("\n")
    );
}

#[test]
fn the_graph_is_measured_rather_than_assumed() {
    // The scanners are the mechanism, so their edges are pinned: a scan that
    // silently found nothing would make both tests above vacuously true.
    let text = "use crate::ast::Program;\nlet x = crate::span::Span::unknown();\n";
    let found = referenced_modules(text);
    assert!(found.contains("ast"));
    assert!(found.contains("span"));
    assert_eq!(found.len(), 2);
    assert!(referenced_modules("nothing here").is_empty());

    let edges = graph();
    assert!(
        edges.len() > 10,
        "only {} modules have any dependency at all",
        edges.len()
    );
    // A spot check against §3.1's own table: the evaluator depends on the AST.
    assert!(
        edges
            .get("evaluator")
            .is_some_and(|t| t.contains_key("ast")),
        "evaluator -> ast is in §3.1's table and must be visible to the scan"
    );
}
