//! The parser's public surface, pinned.
//!
//! `Parser` is about to be taken apart into modules (M1; see
//! `docs/maturity/ROADMAP_STATE.md`). Six methods and four constants are all
//! that `run.rs`, the LSP, `import`, task workers and the test suite ever touch,
//! so that surface — not the internals — is the thing a reader of this file
//! should be able to rely on afterwards.
//!
//! `tests/parser_snapshot.rs` pins *what* the parser produces for the corpus.
//! This pins *how a caller interacts with it*: the shape of the API, when
//! errors become visible, what order they arrive in, and which of them are
//! affected by the labelling calls. Those are exactly the properties a
//! file-splitting refactor can break without changing a single parse tree.
//!
//! Some assertions below record behavior that is surprising rather than
//! desirable — the ordering in
//! `lexical_diagnostics_arrive_after_syntactic_ones` most of all. They are
//! written as measurements of what the parser does today, so that a later
//! change to any of them has to be a decision instead of an accident. Each one
//! says which it is.

use serez_code::lexer::Lexer;
use serez_code::parser::{MAX_PARSE_DEPTH, Parser, SZ_PARSE_ERROR};

/// The whole front door: lex, parse, collect. Every caller in the crate spells
/// exactly this, so the helper is the contract as much as the assertions are.
fn parse(source: &str, name: &str) -> (serez_code::ast::Program, Parser) {
    let lines: Vec<String> = source.lines().map(str::to_string).collect();
    let mut parser = Parser::new(Lexer::new(source.to_string()));
    parser.set_source(lines);
    parser.set_source_name(name);
    let program = parser.parse_program();
    (program, parser)
}

#[test]
fn a_clean_parse_reports_nothing() {
    let (program, parser) = parse("let x = 1;\nout x;\n", "clean.sz");
    assert_eq!(program.statements.len(), 2);
    assert!(!parser.has_errors());
    assert!(parser.take_errors().is_empty());
}

#[test]
fn a_syntax_error_is_coded_and_positioned() {
    let (_, parser) = parse("let = 1;\n", "broken.sz");
    assert!(parser.has_errors());

    let errors = parser.take_errors();
    assert!(!errors.is_empty(), "a rejected program reported no error");
    let first = &errors[0];
    assert_eq!(
        first.code, SZ_PARSE_ERROR,
        "a plain syntax error must stay SZ2000; narrower codes are promises and \
         get split out one at a time"
    );
    assert_eq!(first.span.line, 1, "lines are 1-based");
    assert!(first.span.column >= 1, "columns are 1-based");
    assert!(!first.message.is_empty());
}

#[test]
fn take_errors_reads_the_list_rather_than_draining_it() {
    // The name says "take". The body clones. Both `run.rs` and the LSP call it
    // exactly once, so nothing depends on the difference today — which is
    // precisely why a refactor could switch it to a real drain and nothing
    // would fail. Pinned as observed behavior, not as an endorsement of the
    // name.
    let (_, parser) = parse("let = 1;\n", "broken.sz");
    let first = parser.take_errors();
    let second = parser.take_errors();
    assert!(!first.is_empty());
    assert_eq!(
        first.len(),
        second.len(),
        "take_errors drained the list on the first call"
    );
}

#[test]
fn the_structured_payload_does_not_depend_on_the_source_label() {
    // `set_source` and `set_source_name` feed the stderr rendering — the file
    // prefix and the caret line. The collected `ParseError` is deliberately
    // free of both, so the LSP and the CLI classify identically no matter how
    // the file was labelled. A refactor that folded the label into `message`
    // for convenience would break that.
    let broken = "let = 1;\n";

    let mut bare = Parser::new(Lexer::new(broken.to_string()));
    bare.parse_program();

    let (_, labelled) = parse(broken, "some/deeply/nested/name.sz");

    let bare_errors = bare.take_errors();
    let labelled_errors = labelled.take_errors();
    assert_eq!(bare_errors.len(), labelled_errors.len());
    for (a, b) in bare_errors.iter().zip(labelled_errors.iter()) {
        assert_eq!(a.code, b.code);
        assert_eq!(a.span.line, b.span.line);
        assert_eq!(a.span.column, b.span.column);
        assert_eq!(
            a.message, b.message,
            "the source label leaked into the structured message"
        );
    }
}

#[test]
fn lexical_diagnostics_are_forwarded_into_the_same_list() {
    // One ordered list for the caller, two producers behind it. The LSP relies
    // on this: it publishes `SZ1xxx` and `SZ2xxx` from a single loop.
    let (_, parser) = parse("let s = \"unterminated;\n", "lex.sz");
    let errors = parser.take_errors();
    assert!(
        errors.iter().any(|e| e.code.starts_with("SZ1")),
        "a lexical failure never reached the parser's error list: {:?}",
        errors.iter().map(|e| e.code).collect::<Vec<_>>()
    );
    assert!(parser.has_errors());
}

#[test]
fn lexical_diagnostics_become_visible_only_once_parsing_has_run() {
    // `Parser::new` pulls two tokens, so a lexical failure on the first line
    // already exists inside the parser before `parse_program` is called — but
    // it is held in a separate queue and flushed at the end of `parse_program`,
    // after the source lines and label are known. So `has_errors()` is false on
    // a freshly constructed parser even when the source is already broken.
    //
    // This is a hazard, not a feature: a caller that checked `has_errors()`
    // before parsing would get the wrong answer. Nothing in the crate does.
    // Recorded so that any change to it is deliberate.
    let parser = Parser::new(Lexer::new("let s = \"unterminated;\n".to_string()));
    assert!(
        !parser.has_errors(),
        "lexical errors now surface before parse_program; that is a behavior \
         change, and possibly a good one — decide it, do not drift into it"
    );
}

#[test]
fn lexical_diagnostics_arrive_after_syntactic_ones() {
    // The flush happens at the end of `parse_program`, so the list is grouped
    // by producer rather than sorted by position: a lexical failure on line 1
    // is reported after a syntax error on line 3.
    //
    // This is the ordering M3 (unified diagnostics) has to decide about. It is
    // pinned here as the current, unspecified behavior — `spec/errors.md`
    // documents the codes and the stderr shape but says nothing about order.
    //
    // The malformed literal has to be `0x` rather than an unterminated string:
    // an unterminated string reads to EOF, so it would swallow the syntax error
    // on the last line and there would be nothing left to order against.
    let source = "let a = 0x;\nlet b = 2;\nlet = 3;\n";
    let (_, parser) = parse(source, "both.sz");
    let errors = parser.take_errors();

    let lexical = errors.iter().position(|e| e.code.starts_with("SZ1"));
    let syntactic = errors.iter().position(|e| e.code.starts_with("SZ2"));
    let (Some(lexical), Some(syntactic)) = (lexical, syntactic) else {
        panic!(
            "expected both a lexical and a syntactic diagnostic, got {:?}",
            errors.iter().map(|e| e.code).collect::<Vec<_>>()
        );
    };
    assert!(
        syntactic < lexical,
        "diagnostics are no longer grouped lexer-last. If they are sorted by \
         position now, that is an improvement — but it is an M3 decision, not a \
         side effect of moving code between files"
    );
}

#[test]
fn parsing_recovers_and_keeps_going_after_a_rejected_statement() {
    // `parse_program` calls `synchronize` on a failed statement rather than
    // stopping, so a single bad line does not swallow the rest of the file.
    // The LSP depends on this: it needs symbols from the whole document while
    // the user is mid-edit.
    let (program, parser) = parse("let = 1;\nlet good = 2;\nout good;\n", "recover.sz");
    assert!(parser.has_errors());
    assert!(
        program.statements.len() >= 2,
        "recovery stopped early: {} statements survived a single bad line",
        program.statements.len()
    );
}

#[test]
fn some_syntax_errors_never_reach_the_error_list_at_all() {
    // KNOWN DEFECT, pinned rather than endorsed. See ROADMAP_STATE.md §5.17.
    //
    // Nine sites in the parser report by hand — `had_error.set(true)` plus a
    // bare `eprintln!` — instead of going through `parser_error`. Nothing is
    // pushed into `errors`, so `take_errors()` comes back **empty** for a
    // program the parser has just rejected. Everything downstream of the list
    // is therefore blind to them:
    //
    //   * the LSP publishes no diagnostic, so the editor underlines nothing;
    //   * `run.rs` builds `RunFailure::Frontend(vec![])` — a failure with no
    //     reason attached;
    //   * the message carries no `SZ` code, no file, no line and no column,
    //     which contradicts `spec/errors.md`: "SZ2000 | Syntax error", reported
    //     as `❌ PARSER ERROR [SZ2000] [file line:col]`. These print
    //     `❌ PARSE ERROR:` — a different prefix as well as a different shape.
    //
    // `has_errors()` is still true, which is why the CLI exits 1 and why the
    // 63 error tests pass: they assert an exit code and the presence of a
    // `❌`, and both survive.
    //
    // Pinned so that M1 cannot silently change it in either direction. When it
    // is fixed — routed through `parser_error_code` with a real span — this
    // test fails, and that failure is the reminder to update §5.17,
    // `spec/errors.md` and the snapshot manifest together.
    for source in ["let n = sizeof 5;\n", "unsafe out 2;\n", "native x;\n"] {
        let (_, parser) = parse(source, "uncoded.sz");
        assert!(
            parser.has_errors(),
            "{source:?} was supposed to be rejected"
        );
        assert!(
            parser.take_errors().is_empty(),
            "{source:?} now produces a structured diagnostic. That is the right \
             behavior and the wrong way to arrive at it — if it was deliberate, \
             update ROADMAP_STATE.md §5.17 and this test together"
        );
    }
}

#[test]
fn the_depth_ceiling_is_the_one_the_specification_names() {
    // `spec/errors.md` and `spec/syntax.md` both name 512. The constant is
    // public because `tests/frontend_robustness.rs` builds source against it;
    // this asserts the documented number itself, so the spec and the code
    // cannot drift apart silently.
    assert_eq!(
        MAX_PARSE_DEPTH, 512,
        "spec/errors.md and spec/syntax.md both state 512"
    );
}
