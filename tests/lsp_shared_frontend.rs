//! The editor and the interpreter see one frontend — proved from outside both.
//!
//! # Why this file exists
//!
//! `docs/maturity/ROADMAP_STATE.md` §5.18: `src/lsp_main.rs` declared `mod ast;
//! mod lexer; mod parser; …` of its own, so `sz-lsp` compiled a second copy of
//! the frontend into a second crate rather than depending on the library. The
//! two copies were the same source, so they behaved the same — but only because
//! nobody had yet made a `pub(crate)` decision that was right under one module
//! root and wrong under the other, and nothing except the build would have said
//! so.
//!
//! **This file could not have compiled before the fix.** `lsp` was a private
//! module of a binary crate; an integration test cannot name it. That it
//! compiles is the assertion, and every test below is a second one on top.
//!
//! # What it pins
//!
//! That the analysis an editor gets is produced by the same lexer, parser and
//! type checker that run a program — same codes, same positions, same
//! severities — rather than by a copy that happens to agree.

use serez_code::lsp::analysis;

/// The LSP's diagnostics come from the library's parser, not from a copy.
///
/// The parser is asked directly and the analysis is asked for the same source;
/// if the editor were on a separate build of the frontend, nothing would keep
/// these two codes equal as the parser's codes changed.
#[test]
fn an_editor_diagnostic_carries_the_code_the_parser_produced() {
    let source = "let a = 1;\nlet b = ;\n";

    let mut parser =
        serez_code::parser::Parser::new(serez_code::lexer::Lexer::new(source.to_string()));
    parser.parse_program();
    let from_parser = parser.take_errors();

    let from_editor = analysis::analyze(source);

    assert_eq!(
        from_parser.len(),
        1,
        "fixture should produce exactly one parser diagnostic, got {:?}",
        from_parser.iter().map(|d| d.code).collect::<Vec<_>>()
    );
    assert_eq!(
        from_editor.diagnostics.len(),
        1,
        "the editor saw a different number of diagnostics than the parser"
    );

    assert_eq!(
        from_editor.diagnostics[0].code, from_parser[0].code,
        "the editor's code and the parser's code disagree"
    );
    assert_eq!(
        from_editor.diagnostics[0].line, from_parser[0].span.line,
        "the editor's line and the parser's line disagree"
    );
    assert_eq!(
        from_editor.diagnostics[0].column, from_parser[0].span.column,
        "the editor's column and the parser's column disagree"
    );
}

/// The positive control. Without it, an `analyze` that returned a fixed
/// diagnostic would satisfy the test above.
#[test]
fn a_program_with_nothing_wrong_produces_no_editor_diagnostics() {
    let analysis = analysis::analyze("let a = 1;\nout a;\n");
    assert!(
        analysis.diagnostics.is_empty(),
        "clean source produced {:?}",
        analysis
            .diagnostics
            .iter()
            .map(|d| (d.code, d.line, &d.message))
            .collect::<Vec<_>>()
    );
}

/// A lexical failure keeps its `SZ1xxx` on the way to the editor.
///
/// This is the one that would break first if the editor were on its own copy of
/// the lexer: §5.13 has just changed when a lexical error becomes visible, and a
/// second build of `lexer.rs` would only pick the change up by being rebuilt for
/// the same reason.
#[test]
fn a_lexical_failure_reaches_the_editor_as_a_lexical_code() {
    let analysis = analysis::analyze("let a = 0x;\n");
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|d| d.code.starts_with("SZ1")),
        "expected a lexical SZ1xxx, got {:?}",
        analysis
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>()
    );
}

/// The semantic phase does **not** reach the editor — pinned, not endorsed.
///
/// `semantic` is one of the ten modules that used to be compiled twice, and it
/// is the newest: DEC-M4-001 landed the phase in `9d91f3c`, wired into
/// `run::run_source_detailed`. `lsp::analysis::analyze` was not changed with it,
/// so `sz` rejects `class Task { … }` with a fatal `SZ8000` and exit 1 while the
/// editor underlines nothing. Verified against both release binaries, not only
/// in-process.
///
/// Recorded as §5.41 and **DEC-M4-006**, and left alone here on purpose: §5.18's
/// contract is that consolidating the two builds changes no observable
/// behaviour, and making the editor start reporting a fatal phase is a
/// behaviour change with more than one defensible shape. The pin exists so the
/// gap cannot be closed by accident and then discovered as a surprise.
#[test]
fn the_semantic_phase_does_not_yet_reach_the_editor() {
    // Every rule the phase has, not just the one it had when this pin was
    // written. The gap is not a property of the reserved-name rule; it is that
    // `analyze` never calls `semantic::validate`, so it widens with every rule
    // the phase gains — and it gained two more in this cycle.
    let rejected: &[(&str, &str)] = &[
        (
            "a reserved namespace name",
            "class Task { public Task() {} }\n",
        ),
        (
            "a duplicate declaration",
            "fn int f() { return 1; }\nfn int f() { return 2; }\n",
        ),
        (
            "an unresolvable parent",
            "public class Child : NeverDeclared { public Child() { this.a = 1; } }\n",
        ),
        ("a name declared nowhere", "out totallyUndefined;\n"),
        ("a name used before it is declared", "out x;\nlet x = 1;\n"),
    ];

    for (what, source) in rejected {
        // The positive control comes first, and it is what makes the pin mean
        // something: the rule really does fire, in the same process, through the
        // same library. Without it, the assertion below would pass against a
        // phase that had stopped working.
        let mut parser =
            serez_code::parser::Parser::new(serez_code::lexer::Lexer::new(source.to_string()));
        let program = parser.parse_program();
        assert!(!parser.has_errors(), "fixture must parse cleanly: {what}");
        let findings = serez_code::semantic::validate::validate(&program);
        assert!(
            findings.iter().any(|d| d.code == "SZ8000"),
            "the semantic phase no longer rejects {what}, so this pin proves \
             nothing: got {:?}",
            findings.iter().map(|d| d.code).collect::<Vec<_>>()
        );

        let analysis = analysis::analyze(source);
        assert!(
            !analysis.diagnostics.iter().any(|d| d.code == "SZ8000"),
            "the editor now reports the semantic phase for {what}. That is very \
             likely an improvement — but it is DEC-M4-006, and this pin exists so \
             it is decided rather than drifted into. Answer the decision, then \
             delete this."
        );
    }
}

/// And the editor is not silent in general — it reports what it was wired for.
///
/// The control on the control. Every assertion above is that a diagnostic is
/// *absent*, and all of them would pass against an `analyze` that had stopped
/// producing diagnostics at all.
#[test]
fn the_editor_still_reports_the_phases_it_was_wired_for() {
    let broken = analysis::analyze("let = 5;\n");
    assert!(
        !broken.diagnostics.is_empty(),
        "the editor reported nothing for a file that does not parse"
    );
}

/// The outline the editor draws comes from the shared symbol scan.
#[test]
fn the_editor_outline_sees_the_declarations_in_the_source() {
    let analysis = analysis::analyze(
        "class Point { public Point() {} }\nfn int twice(int n) { return n * 2; }\n",
    );
    let names: Vec<&str> = analysis.symbols.iter().map(|s| s.name.as_str()).collect();
    assert!(
        names.contains(&"Point") && names.contains(&"twice"),
        "outline was {names:?}"
    );
}
