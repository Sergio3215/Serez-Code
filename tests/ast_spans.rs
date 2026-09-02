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
//! A node's span runs from **its own opening token** to its last token. What
//! that means differs by kind, and the difference is the point:
//!
//! - **Declarations are complete.** `let`, `fn`, `class`, `interface` and
//!   `native fn` each begin with the keyword that names them, so the extent is
//!   the whole construct — `let x = 1 + 2;`, or a function including its body.
//! - **Expressions are partial.** A call spans from its `(` and an infix
//!   expression from its operator, *not* from the callee or the left operand,
//!   because those are `Identifier`s and the like which carry no span until
//!   M2.6. `foo(1, 2)` therefore yields `(1, 2)`.
//!
//! Both are real and correct; the second is narrower than it will eventually be.
//! These tests assert what is true today rather than what is wanted later, so
//! that when M2.6 widens the expression extents it has to update them
//! deliberately rather than discovering the change afterwards.
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

/// The first statement, for the declaration fixtures below.
fn first_statement(program: &Program) -> &Statement {
    program
        .statements
        .first()
        .expect("fixture has no statement")
}

#[test]
fn a_let_declaration_spans_from_its_keyword() {
    // Unlike an expression node, a declaration *does* start at its own first
    // token — there is no callee or left operand in front of it — so these
    // extents are complete rather than partial.
    let source = "let x = 1 + 2;\n";
    let program = parse(source);
    let Statement::Let(let_statement) = first_statement(&program) else {
        panic!("expected a let");
    };
    assert_eq!(slice(source, let_statement.span), "let x = 1 + 2;");
    assert_eq!((let_statement.span.line, let_statement.span.column), (1, 1));
}

#[test]
fn a_const_declaration_spans_from_its_own_keyword_not_from_let() {
    let source = "const k = 7;\n";
    let program = parse(source);
    let Statement::Let(let_statement) = first_statement(&program) else {
        panic!("expected a const");
    };
    assert_eq!(slice(source, let_statement.span), "const k = 7;");
}

#[test]
fn a_function_declaration_spans_its_whole_body() {
    let source = "fn int add(int a, int b) {\n    return a + b;\n}\n";
    let program = parse(source);
    let Statement::FunctionDeclaration(function) = first_statement(&program) else {
        panic!("expected a function");
    };
    assert_eq!(
        slice(source, function.span),
        "fn int add(int a, int b) {\n    return a + b;\n}"
    );
    assert_eq!((function.span.line, function.span.column), (1, 1));
}

#[test]
fn a_class_declaration_spans_from_class_not_from_its_modifier() {
    // `public`/`abstract`/`sealed` are consumed by the caller before
    // `parse_class_declaration` runs, so the extent starts at `class`. Recorded
    // as a deliberate choice rather than left to be rediscovered: it is the same
    // rule every other declaration follows — the extent begins at the keyword
    // that names the construct.
    let source = "public class K {\n    public int m() { return 1; }\n}\n";
    let program = parse(source);
    let Statement::ClassDeclaration(class) = first_statement(&program) else {
        panic!("expected a class");
    };
    assert!(
        slice(source, class.span).starts_with("class K {"),
        "expected the extent to begin at `class`, got {:?}",
        slice(source, class.span)
    );
    assert_eq!(
        class.span.column, 8,
        "column 8 is `class`, not the `public` at column 1"
    );
}

#[test]
fn a_for_loop_initializer_stops_at_the_semicolon() {
    // The one declaration whose extent could not be taken at construction time:
    // by then the cursor has moved past the `;` onto the condition. It is
    // captured earlier instead, and this is what proves the earlier capture was
    // needed — an extent taken at construction would have swallowed `i < 3`.
    let source = "for (let i = 0; i < 3; i = i + 1) { out i; }\n";
    let program = parse(source);
    let Statement::For(for_statement) = first_statement(&program) else {
        panic!("expected a for");
    };
    assert_eq!(slice(source, for_statement.init.span), "let i = 0");
}

#[test]
fn a_while_loop_spans_from_its_keyword_to_its_closing_brace() {
    let source = "while (x < 3) {\n    out x;\n}\n";
    let program = parse(source);
    let Statement::While(loop_statement) = first_statement(&program) else {
        panic!("expected a while");
    };
    assert_eq!(
        slice(source, loop_statement.span),
        "while (x < 3) {\n    out x;\n}"
    );
}

#[test]
fn a_return_spans_its_statement() {
    let source = "fn int f() {\n    return 1 + 2;\n}\n";
    let program = parse(source);
    let Statement::FunctionDeclaration(function) = first_statement(&program) else {
        panic!("expected a function");
    };
    let Statement::Return(returned) = &function.function.body.statements[0] else {
        panic!("expected a return");
    };
    assert_eq!(slice(source, returned.span), "return 1 + 2;");
    assert_eq!(returned.span.line, 2);
}

#[test]
fn a_block_spans_its_braces() {
    let source = "fn void f() {\n    out 1;\n}\n";
    let program = parse(source);
    let Statement::FunctionDeclaration(function) = first_statement(&program) else {
        panic!("expected a function");
    };
    assert_eq!(
        slice(source, function.function.body.span),
        "{\n    out 1;\n}"
    );
}

#[test]
fn a_synthetic_node_gets_a_position_but_no_extent() {
    // The rule in ROADMAP_STATE.md §5.23, asserted rather than merely written
    // down. `i++` desugars to `i = i + 1`; the assignment and the `+ 1` inside
    // it are nodes the programmer never wrote, so giving them an extent would
    // hand them source text that says something else. They get a point.
    let source = "let i = 0;\ni++;\n";
    let program = parse(source);
    let Statement::Assign(assignment) = &program.statements[1] else {
        panic!("expected the desugared assignment");
    };
    assert!(
        assignment.span.is_known(),
        "a synthetic node still reports where it came from"
    );
    assert!(
        !assignment.span.has_extent(),
        "a synthetic node must not claim source text it does not occupy, got {:?}",
        assignment.span
    );
    assert_eq!((assignment.span.line, assignment.span.column), (2, 1));
}

#[test]
fn an_else_if_wrapper_has_no_position_at_all() {
    // The other side of §5.23: `else if` is parsed by recursion into a
    // single-statement block that has no braces in the source. It is not merely
    // extent-less, it has no position — there is nothing to point at.
    let source = "if (a) {\n    out 1;\n} else if (b) {\n    out 2;\n}\n";
    let program = parse(source);
    let Statement::Expression(Expression::If(outer)) = first_statement(&program) else {
        panic!("expected an if");
    };
    let alternative = outer.alternative.as_ref().expect("expected an else branch");
    assert!(
        !alternative.span.is_known(),
        "the synthetic else-if wrapper should carry no position, got {:?}",
        alternative.span
    );
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
