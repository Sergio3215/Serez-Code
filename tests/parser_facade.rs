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
fn a_parser_whose_source_is_already_broken_says_so_before_parsing() {
    // `Parser::new` pulls two tokens, so a malformed token at the head of the
    // file already exists inside the parser before `parse_program` is called.
    // It waits in a separate queue, flushed at the end of `parse_program` once
    // the source lines and label are known — and `has_errors()` used to read
    // only the flag that flush sets, so it answered `false` about source the
    // parser had already failed to lex (ROADMAP_STATE.md §5.13).
    //
    // Decided and fixed: a known error is a known error. `has_errors()` reads
    // the queue as well, so it is true the moment the parser exists.
    let parser = Parser::new(Lexer::new(
        "0x;
"
        .to_string(),
    ));
    assert!(
        parser.has_errors(),
        "the parser held a lexical error from construction and denied it"
    );
    // The queue is the only thing that can be answering: nothing has been
    // flushed, so the *list* is still empty. §5.13 moved the yes/no question
    // and left ordering (§5.12, decision D6) alone.
    assert!(
        parser.take_errors().is_empty(),
        "take_errors() must still report only what has been flushed"
    );
}

#[test]
fn a_parser_over_clean_source_still_reports_no_errors() {
    // The positive control for the test above. If `has_errors()` had simply
    // become `true`, that test would pass for the wrong reason and this one
    // would fail — which is the whole point of writing it.
    let parser = Parser::new(Lexer::new(
        "let s = \"fine\";
"
        .to_string(),
    ));
    assert!(
        !parser.has_errors(),
        "a parser over well-formed source invented an error"
    );
}

#[test]
fn a_lexical_error_the_parser_has_not_reached_yet_is_not_yet_known() {
    // The boundary, stated precisely, because §5.13's own example sat on the
    // wrong side of it: `let s = "unterminated;` is *not* known at construction
    // — two tokens of lookahead reach `let` and `s`, and the bad string is the
    // fourth token. `has_errors()` answers "does the parser know of an error",
    // not "does this source contain one", and it cannot answer the second
    // without lexing the whole file eagerly.
    //
    // What the fix guarantees is that nothing the parser has already read stays
    // hidden behind the flush.
    let mut parser = Parser::new(Lexer::new(
        "let a = 0x;
"
        .to_string(),
    ));
    assert!(
        !parser.has_errors(),
        "the parser claimed an error it has not read yet"
    );
    parser.parse_program();
    assert!(
        parser.has_errors(),
        "the error was still hidden after the whole file was read"
    );
    assert!(
        parser
            .take_errors()
            .iter()
            .any(|e| e.code.starts_with("SZ1")),
        "expected a lexical SZ1xxx in the list after the flush"
    );
}

#[test]
fn lexical_diagnostics_arrive_after_syntactic_ones() {
    // The flush happens at the end of `parse_program`, so the list is grouped
    // by producer rather than sorted by position: a lexical failure on line 1
    // is reported after a syntax error on line 3.
    //
    // M3.8 decided this, as decision D6 in `docs/maturity/ROADMAP_STATE.md`:
    // **keep it**. `spec/errors.md` documents the codes and the stderr shape and
    // says nothing about order, so nothing is violated, and across all 499
    // corpus files not one produces both a lexical and a syntactic diagnostic
    // — the case below had to be constructed by hand. Changing the order a
    // user reads diagnostics in is a UX call for Sergio, not a side effect of a
    // diagnostics refactor. §9D.7 costs the alternative if he wants it.
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
        "diagnostics are no longer grouped lexer-last. Sorting by position may well be better — §9D.7 costs exactly how — but D6 decided to keep it, so reaching here means it changed by accident"
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
fn every_rejected_program_says_why_in_the_error_list() {
    // This test used to assert the opposite. See ROADMAP_STATE.md §5.17 and
    // §9D.6.
    //
    // Nine sites in the parser reported by hand — `had_error.set(true)` plus a
    // bare `eprintln!` — instead of going through `parser_error`. Nothing was
    // pushed into `errors`, so `take_errors()` came back **empty** for a program
    // the parser had just rejected, and everything downstream was blind: the LSP
    // underlined nothing, and `run.rs` built a `RunFailure::Frontend(vec![])` —
    // a failure with no reason attached. The lines they printed carried no `SZ`
    // code, no file, no line and no column, which contradicted `spec/errors.md`.
    //
    // M3.7 routed all nine through `parser_error`. The nine constructs below are
    // one per site, and each is also a conformance fixture
    // (`tests/err_parse_*.sz`) so the rendered form is pinned too.
    let cases = [
        "let n = sizeof 5;
",
        "let n = sizeof(int;
",
        "unsafe out 2;
",
        "let x = unsafe 2;
",
        "native x;
",
        "native fn 5;
",
        "native fn doThing;
",
        "class C {
    int x = 1;
}
",
        "public fn f() { out 1; }
",
    ];

    for source in cases {
        let (_, parser) = parse(source, "uncoded.sz");
        assert!(
            parser.has_errors(),
            "{source:?} was supposed to be rejected"
        );

        let errors = parser.take_errors();
        assert!(
            !errors.is_empty(),
            "{source:?} is rejected but reports nothing to a caller — the §5.17              defect is back, and the LSP shows the user a clean file"
        );
        let first = &errors[0];
        assert_eq!(
            first.code, SZ_PARSE_ERROR,
            "{source:?} reported {} — these are plain syntax errors and stay              SZ2000 until a narrower code is deliberately split out",
            first.code
        );
        assert!(
            first.span.line >= 1 && first.span.column >= 1,
            "{source:?} reported a diagnostic with no position: {:?}",
            first.span
        );
        assert!(
            !first.message.is_empty(),
            "{source:?} reported an empty message"
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
