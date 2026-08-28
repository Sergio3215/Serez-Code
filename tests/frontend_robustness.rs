//! The frontend must never crash on text a user can type.
//!
//! Every case here is source the lexer/parser/type-checker is allowed to
//! *reject* — none of it is valid Serez. What none of it is allowed to do is
//! take the process down: a stack overflow or a panic is not a diagnostic, it
//! is a crash with no line number, no exit code the CLI chose, and nothing for
//! the LSP to underline.
//!
//! Two failure modes are covered:
//!
//!   * **Unbounded recursion over the AST.** Two shapes reached it: nesting
//!     (`((((…))))`, one Rust stack frame per level in the recursive-descent
//!     parser) and operator chains (`1+1+1+…`, which parse in a flat loop but
//!     build a tree one level deeper per operator for the type checker, the
//!     evaluator and the AST's drop glue to recurse over). Both used to exhaust
//!     the native stack — `STATUS_STACK_OVERFLOW`, exit `0xC00000FD`, empty
//!     stderr. [`MAX_PARSE_DEPTH`] now bounds both. A stack overflow aborts the
//!     process rather than unwinding, so these cases cannot be caught — if the
//!     ceiling regresses, the whole test binary dies, which is the signal.
//!
//!   * **Panics on malformed input.** Truncated literals, unterminated
//!     constructs and odd Unicode are caught with `catch_unwind` and reported
//!     per-case, so one regression names itself instead of hiding the rest.

use serez_code::lexer::{
    Lexer, SZ_LEX_INVALID_BASE_INTEGER, SZ_LEX_UNEXPECTED_CHARACTER, SZ_LEX_UNTERMINATED_COMMENT,
    SZ_LEX_UNTERMINATED_STRING,
};
use serez_code::parser::{MAX_PARSE_DEPTH, Parser};
use serez_code::type_checker::TypeChecker;

/// Stack for the thread each case runs on.
///
/// This is *not* the budget the ceiling is sized against — it is the budget
/// needed to exercise the ceiling in a **debug** build. `parse_expression` is
/// one enormous `match`, and an unoptimized frame for it measures around 8 KiB
/// against roughly 256 bytes when optimized. So the same `MAX_PARSE_DEPTH`
/// levels that cost the shipped release binary ~128 KiB cost this test binary
/// several MiB.
///
/// `cargo test` gives its threads 2 MiB, which is right at that edge, so the
/// cases run on an explicit thread instead: sized for the ceiling times the
/// debug frame, with room to spare. Shrinking this does not make the test
/// stricter, only flakier.
const TEST_STACK: usize = 16 * 1024 * 1024;

/// Run `body` on a thread with [`TEST_STACK`], propagating its result.
///
/// A stack overflow aborts the process rather than unwinding, so this cannot
/// catch one — that is the point. If the ceiling regresses, the test binary
/// dies with `STATUS_STACK_OVERFLOW`/`SIGSEGV`, which is a louder and more
/// honest signal than a failed assertion.
fn on_test_stack<T: Send + 'static>(body: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(TEST_STACK)
        .spawn(body)
        .expect("spawn frontend thread")
        .join()
        .expect("frontend thread panicked")
}

/// Run the whole frontend over `src`, exactly as `run_source` does minus
/// evaluation. Returns whether the parser reported at least one error.
///
/// The type check is not incidental: it is a second recursive walk over the
/// same AST, and the AST's drop glue is a third. All three are bounded by the
/// parser's ceiling, so all three belong in the measurement.
fn parse_frontend(src: &str) -> bool {
    let lexer = Lexer::new(src.to_string());
    let mut parser = Parser::new(lexer);
    parser.set_source(src.lines().map(|l| l.to_string()).collect());
    parser.set_source_name("<robustness>");
    let program = parser.parse_program();
    let had_errors = parser.has_errors();

    // The type checker walks the same AST and is just as recursive.
    let mut checker = TypeChecker::new(&program);
    checker.check();

    had_errors
}

/// Frontend result, or `None` if the case panicked.
fn parse_frontend_caught(src: &str) -> Option<bool> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_frontend(src))).ok()
}

/// Source nested `depth` levels deep, one construct per case.
fn nested(depth: usize) -> Vec<(&'static str, String)> {
    vec![
        (
            "parentheses",
            format!("out({}1{});", "(".repeat(depth), ")".repeat(depth)),
        ),
        (
            "array literals",
            format!("out({}{});", "[".repeat(depth), "]".repeat(depth)),
        ),
        (
            "blocks",
            format!("{}{}", "{".repeat(depth), "}".repeat(depth)),
        ),
        // Spaced, because `--` lexes as decrement rather than as two negations.
        ("unary minus", format!("out({}1);", "- ".repeat(depth))),
        ("logical not", format!("out({}true);", "!".repeat(depth))),
        (
            "calls",
            format!("out({}1{});", "f(".repeat(depth), ")".repeat(depth)),
        ),
        ("indexing", format!("out(a{});", "[0]".repeat(depth))),
        (
            "ternaries",
            format!("out({}1{});", "true ? ".repeat(depth), " : 0".repeat(depth)),
        ),
    ]
}

#[test]
fn deep_nesting_is_rejected_instead_of_overflowing_the_stack() {
    // Far past the ceiling, and at the depth that used to abort the process
    // outright in a release build (~32k levels).
    let depth = MAX_PARSE_DEPTH * 64;
    on_test_stack(move || {
        for (what, src) in nested(depth) {
            assert!(
                parse_frontend(&src),
                "{what}: source nested {depth} levels deep parsed without an \
                 error; the depth ceiling is not being enforced"
            );
        }
    });
}

#[test]
fn nesting_within_the_ceiling_still_parses() {
    // The ceiling must not cost real code anything. The deepest nesting across
    // the 999 .sz/.szx files in the official ecosystem is 19 levels.
    //
    // The headroom asserted here is deliberately well under MAX_PARSE_DEPTH:
    // constructs do not map one-to-one onto levels. `-x` costs two (the prefix
    // re-enters expression parsing), and `f(x)` costs one for the call plus one
    // for the argument. The contract is "far more nesting than real code uses",
    // not "exactly MAX_PARSE_DEPTH of any construct".
    let depth = MAX_PARSE_DEPTH / 4;
    on_test_stack(move || {
        for (what, src) in nested(depth) {
            // `blocks`, `calls` and `indexing` name things that do not exist,
            // but that is a *runtime* concern; the parser must accept them.
            assert!(
                !parse_frontend(&src),
                "{what}: {depth} levels of nesting was rejected; the ceiling is \
                 too tight or is counting levels it should not"
            );
        }
    });
}

#[test]
fn operator_chains_are_charged_against_the_same_ceiling() {
    // `1+1+1+…` parses in a flat loop, so it costs the parser nothing — but it
    // builds a tree one level deeper per operator, which is what the type
    // checker, the evaluator and the AST's drop glue recurse over. It used to
    // take the process down at ~32k terms with no diagnostic.
    on_test_stack(|| {
        let over = format!("out(1{});", "+1".repeat(MAX_PARSE_DEPTH * 200));
        assert!(
            parse_frontend(&over),
            "a {}-term operator chain parsed without an error; chain length is \
             not being charged against the depth ceiling",
            MAX_PARSE_DEPTH * 200
        );

        // Chains real code actually writes must be untouched: the longest in
        // the official ecosystem is 25 operators.
        let under = format!("out(1{});", "+1".repeat(MAX_PARSE_DEPTH / 4));
        assert!(
            !parse_frontend(&under),
            "a {}-operator chain was rejected; the ceiling is too tight for \
             ordinary expressions",
            MAX_PARSE_DEPTH / 4
        );
    });
}

#[test]
fn malformed_input_never_panics() {
    on_test_stack(malformed_input_never_panics_body);
}

fn malformed_input_never_panics_body() {
    let cases: Vec<(&str, String)> = vec![
        ("empty source", String::new()),
        ("only whitespace", "   \n\t\r\n  ".to_string()),
        ("lone brace", "}".to_string()),
        ("unterminated string", "let s <string> = \"abc".to_string()),
        (
            "unterminated interpolation",
            "let s <string> = \"a{1 + \"".to_string(),
        ),
        (
            "empty interpolation",
            "let s <string> = \"{}\";".to_string(),
        ),
        ("unterminated block comment", "/* never closed".to_string()),
        ("unterminated char", "let c <string> = '".to_string()),
        (
            "integer out of range",
            "let n <int> = 99999999999999999999999;".to_string(),
        ),
        (
            "decimal out of range",
            "let d <dec> = 1e999999;".to_string(),
        ),
        ("bare exponent", "let d <dec> = 1e;".to_string()),
        ("hex with no digits", "let n <int> = 0x;".to_string()),
        ("lone dot", ".".to_string()),
        (
            "dangling operators",
            "+ * / % ** <= >= == != && || ??".to_string(),
        ),
        ("truncated let", "let".to_string()),
        ("truncated annotation", "let x <".to_string()),
        ("unclosed generic", "let d <string, <int".to_string()),
        ("truncated class", "class".to_string()),
        ("class with no body", "class Foo extends".to_string()),
        ("truncated function", "function f(".to_string()),
        ("truncated lambda", "let f <any> = (a, b) =>".to_string()),
        ("truncated import", "import".to_string()),
        ("import of nothing", "import { } from".to_string()),
        ("truncated try", "try {".to_string()),
        ("catch with no try", "catch (e) { }".to_string()),
        ("truncated switch", "switch (x) {".to_string()),
        ("truncated for", "for (".to_string()),
        ("truncated interface", "interface I {".to_string()),
        ("stray arrow", "=>".to_string()),
        ("stray pipeline", "|>".to_string()),
        ("nul byte", "let x <int> = 1;\0let y <int> = 2;".to_string()),
        (
            "combining marks",
            "let a\u{0301}\u{0301}\u{0301} <int> = 1;".to_string(),
        ),
        (
            "right-to-left override",
            "let \u{202E}x <int> = 1;".to_string(),
        ),
        ("zero-width joiner", "let a\u{200D}b <int> = 1;".to_string()),
        ("emoji identifier", "let \u{1F600} <int> = 1;".to_string()),
        (
            "astral plane string",
            "out(\"\u{1F4A9}\u{10FFFF}\");".to_string(),
        ),
        ("bom then code", "\u{FEFF}out(1);".to_string()),
        ("cr only line endings", "out(1);\rout(2);\r".to_string()),
        (
            "very long identifier",
            format!("let {} <int> = 1;", "a".repeat(100_000)),
        ),
        (
            "very long string",
            format!("out(\"{}\");", "x".repeat(1_000_000)),
        ),
        ("unbalanced open only", "(".repeat(5_000)),
        ("unbalanced close only", ")".repeat(5_000)),
        ("alternating brackets", "([{".repeat(2_000)),
    ];

    let mut panicked = Vec::new();
    for (what, src) in cases {
        // Keep the panic hook quiet: a caught panic is reported by this test,
        // not by a backtrace interleaved with every other case.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let outcome = parse_frontend_caught(&src);
        std::panic::set_hook(previous);

        if outcome.is_none() {
            panicked.push(what);
        }
    }

    assert!(
        panicked.is_empty(),
        "the frontend panicked on user-typeable source: {panicked:?}"
    );
}

/// Parse `src` and return the diagnostics the parser collected, in order.
fn parse_errors(src: &str) -> Vec<serez_code::parser::ParseError> {
    let lexer = Lexer::new(src.to_string());
    let mut parser = Parser::new(lexer);
    parser.set_source(src.lines().map(|l| l.to_string()).collect());
    parser.parse_program();
    parser.take_errors()
}

#[test]
fn parser_diagnostics_carry_a_stable_code() {
    use serez_code::parser::{SZ_PARSE_DEPTH_EXCEEDED, SZ_PARSE_ERROR};

    // Tooling classifies on the code, so every diagnostic must have one.
    let generic = parse_errors("let x <int> = ;");
    assert!(
        !generic.is_empty(),
        "expected a diagnostic for invalid source"
    );
    assert_eq!(
        generic[0].code, SZ_PARSE_ERROR,
        "an unclassified syntax error must fall back to the generic parser code"
    );

    // The depth ceiling is the one parser diagnostic with its own code, so a
    // client can recognize it without matching on the wording.
    on_test_stack(|| {
        let deep = format!("out({}1{});", "(".repeat(2_000), ")".repeat(2_000));
        let errors = parse_errors(&deep);
        assert!(
            errors.iter().any(|e| e.code == SZ_PARSE_DEPTH_EXCEEDED),
            "the depth ceiling must report {SZ_PARSE_DEPTH_EXCEEDED}, got {:?}",
            errors.iter().map(|e| e.code).collect::<Vec<_>>()
        );
    });
}

#[test]
fn lexer_diagnostics_reach_the_shared_frontend_channel() {
    for (source, code) in [
        ("@", SZ_LEX_UNEXPECTED_CHARACTER),
        ("\"unterminated", SZ_LEX_UNTERMINATED_STRING),
        ("/* unterminated", SZ_LEX_UNTERMINATED_COMMENT),
        ("out(0x);", SZ_LEX_INVALID_BASE_INTEGER),
    ] {
        let errors = parse_errors(source);
        assert!(
            errors.iter().any(|error| error.code == code),
            "source {source:?} must report {code}, got {:?}",
            errors.iter().map(|error| error.code).collect::<Vec<_>>()
        );
    }
}

#[test]
fn type_checker_diagnostics_carry_a_stable_code() {
    use serez_code::type_checker::SZ_TYPE_ERROR;

    let src = "fn int add(int a, int b) { return a + b; }\nout add(1);\n";
    let lexer = Lexer::new(src.to_string());
    let mut parser = Parser::new(lexer);
    let program = parser.parse_program();
    let mut checker = TypeChecker::new(&program);
    checker.check();

    let errors = checker.take_errors();
    assert!(!errors.is_empty(), "expected an arity diagnostic");
    assert_eq!(
        errors[0].code, SZ_TYPE_ERROR,
        "an unclassified semantic error must fall back to the generic type code"
    );
}

#[test]
fn required_parameters_cannot_follow_defaults() {
    use serez_code::parser::SZ_PARSE_ERROR;

    for (form, source) in [
        (
            "named function",
            "fn int bad(int optional = 1, int required) { return required; }",
        ),
        (
            "anonymous function",
            "let bad = fn int (int optional = 1, int required) { return required; };",
        ),
        (
            "typed arrow function",
            "let bad = int (int optional = 1, int required) => { return required; }",
        ),
        (
            "class constructor",
            "class Bad { public Bad(int optional = 1, int required) {} }",
        ),
        (
            "class method",
            "class Bad { public int read(int optional = 1, int required) { return required; } }",
        ),
    ] {
        let errors = parse_errors(source);
        assert!(
            errors.iter().any(|error| {
                error.code == SZ_PARSE_ERROR
                    && error.message == "Required parameter cannot follow a default parameter"
            }),
            "{form} must reject a required parameter after a default; got {errors:?}"
        );
    }

    let valid =
        parse_errors("fn int valid(int required, int optional = 1, ...rest) { return required; }");
    assert!(
        valid.is_empty(),
        "a rest parameter may follow defaults and remains last; got {valid:?}"
    );

    let invalid_rest = parse_errors("fn bad(...rest, int later) { return later; }");
    assert!(
        invalid_rest.iter().any(|error| {
            error.code == SZ_PARSE_ERROR && error.message == "Rest parameter must be last"
        }),
        "a rest parameter followed by another parameter must report a diagnostic; got {invalid_rest:?}"
    );
}
