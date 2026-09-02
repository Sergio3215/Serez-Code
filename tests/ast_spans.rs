//! Do the byte offsets on an AST node point at that node's source?
//!
//! Same reasoning as `tests/lexer_spans.rs`: these offsets have no consumer yet
//! (`docs/maturity/ROADMAP_STATE.md` §9B.1), so nothing else would notice them
//! being wrong. The difference is that an AST node's extent is a *decision*
//! rather than a fact — a call spans something, but exactly what depends on
//! which of its parts already carry spans.
//!
//! # What a node's extent currently covers, and why not more
//!
//! A node's span runs from **its own opening token** to its last token — for a
//! call, from `(`; for an infix expression, from the operator. Not from the
//! start of the callee or the left operand, because those are `Identifier`s and
//! the like, which do not carry spans until M2.6. So the extents here are real
//! and correct, and they are narrower than they will eventually be. These tests
//! assert what is true today rather than what is wanted later; when M2.6 widens
//! them, they are the tests that have to be updated deliberately.
//!
//! `line` and `column` are unaffected by any of this and must stay so — they
//! are what `spec/errors.md` promises a caught `Error.span` reports.

use serez_code::ast::{Expression, Program, Statement};
use serez_code::lexer::Lexer;
use serez_code::parser::Parser;
use serez_code::span::Span;

fn parse(source: &str) -> Program {
    let lines: Vec<String> = source.lines().map(str::to_string).collect();
    let mut parser = Parser::new(Lexer::new(source.to_string()));
    parser.set_source(lines);
    let program = parser.parse_program();
    assert!(
        !parser.has_errors(),
        "fixture failed to parse: {source:?}\n{:?}",
        parser.take_errors()
    );
    program
}

/// The first expression of the first `out` statement, which is how every
/// fixture below is written.
fn first_out_expression(program: &Program) -> &Expression {
    for statement in &program.statements {
        if let Statement::Out(out) = statement {
            return &out.value;
        }
    }
    panic!("fixture has no `out` statement");
}

fn slice(source: &str, span: Span) -> &str {
    &source[span.start..span.end]
}

#[test]
fn a_call_spans_its_argument_list() {
    let source = "out foo(1, 2);\n";
    let program = parse(source);
    let Expression::Call(call) = first_out_expression(&program) else {
        panic!("expected a call");
    };
    assert_eq!(
        slice(source, call.span),
        "(1, 2)",
        "a call's extent runs from its own `(`; widening it to include the \
         callee needs the callee to carry a span, which is M2.6"
    );
    // The rendered position is unchanged by any of that: it is the `(`.
    assert_eq!((call.span.line, call.span.column), (1, 8));
}

#[test]
fn an_infix_expression_spans_from_its_operator_to_its_right_operand() {
    let source = "out 10 + 20;\n";
    let program = parse(source);
    let Expression::Infix(infix) = first_out_expression(&program) else {
        panic!("expected an infix expression");
    };
    assert_eq!(slice(source, infix.span), "+ 20");
    assert_eq!((infix.span.line, infix.span.column), (1, 8));
}

#[test]
fn a_dot_call_spans_from_its_dot() {
    let source = "out \"abc\".length();\n";
    let program = parse(source);
    let Expression::DotCall(dot) = first_out_expression(&program) else {
        panic!("expected a dot call");
    };
    assert_eq!(slice(source, dot.span), ".length()");
}

#[test]
fn a_multiline_node_spans_across_the_newline() {
    // The case a single-line fixture cannot catch: an extent that crosses a line
    // boundary has to keep counting bytes while `line`/`column` stay at the
    // opening token.
    let source = "out foo(\n    1,\n    2\n);\n";
    let program = parse(source);
    let Expression::Call(call) = first_out_expression(&program) else {
        panic!("expected a call");
    };
    assert_eq!(slice(source, call.span), "(\n    1,\n    2\n)");
    assert_eq!(
        (call.span.line, call.span.column),
        (1, 8),
        "the reported position stays on the opening line"
    );
}

#[test]
fn a_span_with_multibyte_source_slices_on_character_boundaries() {
    // Offsets are bytes and columns are scalar values; a fixture that is pure
    // ASCII can never tell the two apart.
    let source = "out fá(\"ñ\");\n";
    let program = parse(source);
    let Expression::Call(call) = first_out_expression(&program) else {
        panic!("expected a call");
    };
    assert!(
        source.is_char_boundary(call.span.start) && source.is_char_boundary(call.span.end),
        "span {:?} splits a UTF-8 character",
        call.span
    );
    assert_eq!(slice(source, call.span), "(\"ñ\")");
}

#[test]
fn every_populated_span_is_a_valid_slice_of_its_source() {
    // A sweep rather than a shape assertion: whatever the extents end up being,
    // none of them may be inverted, out of bounds, or mid-character. This is the
    // test that keeps holding as M2.4-M2.6 widen coverage.
    let sources = [
        "out foo(1, 2);\n",
        "out a.b().c();\n",
        "out 1 + 2 * 3 - 4;\n",
        "out f(g(h(1)));\n",
        "let xs [int] = [1, 2, 3];\nout xs.map((x) => x * 2);\n",
        "class K { public int m() { return 1; } }\nout new K().m();\n",
    ];
    for source in sources {
        let program = parse(source);
        for statement in &program.statements {
            if let Statement::Out(out) = statement {
                check(source, &out.value);
            }
        }
    }

    fn check(source: &str, expression: &Expression) {
        let span = match expression {
            Expression::Call(call) => {
                check(source, &call.function);
                for argument in &call.arguments {
                    check(source, argument);
                }
                call.span
            }
            Expression::DotCall(dot) => {
                check(source, &dot.object);
                for argument in &dot.arguments {
                    check(source, argument);
                }
                dot.span
            }
            Expression::Infix(infix) => {
                check(source, &infix.left);
                check(source, &infix.right);
                infix.span
            }
            _ => return,
        };
        assert!(
            span.start <= span.end,
            "inverted span {span:?} in {source:?}"
        );
        assert!(
            span.end <= source.len(),
            "span {span:?} runs past a {}-byte source {source:?}",
            source.len()
        );
        assert!(
            source.is_char_boundary(span.start) && source.is_char_boundary(span.end),
            "span {span:?} splits a character in {source:?}"
        );
    }
}
