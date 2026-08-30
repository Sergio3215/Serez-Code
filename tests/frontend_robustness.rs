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

/// The README release badge and the DEVELOPMENT status table state the runtime
/// version by hand. Both had drifted six releases behind `Cargo.toml` — README
/// said 9.11.0 while the crate said 9.17.0 — and nothing noticed, because a
/// stale number in a document breaks no build.
///
/// This is the cheapest possible guard: the two places that name the version
/// have to name the one that is actually shipping.
#[test]
fn docs_versions_match_the_crate() {
    let version = env!("CARGO_PKG_VERSION");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let readme =
        std::fs::read_to_string(root.join("README.md")).expect("README.md must be readable");
    let badge = format!("badge/release-v{version}-");
    assert!(
        readme.contains(&badge),
        "README.md release badge must say v{version}; \
         update the shields.io badge on the title block"
    );

    let development = std::fs::read_to_string(root.join("DEVELOPMENT.md"))
        .expect("DEVELOPMENT.md must be readable");
    let row = format!("| Version | {version} (`Cargo.toml`) |");
    assert!(
        development.contains(&row),
        "DEVELOPMENT.md project-status table must say {version}"
    );
}

/// The conformance runners load fixture trees that `.gitignore` used to swallow.
///
/// `.gitignore` excludes `*.sz`, `*.json` and friends repository-wide and then
/// un-ignores specific paths. `!tests/*.sz` covers only the top level, so
/// `tests/lib/`, `tests/packages/`, `tests/runner_fixtures/` and the whole
/// Serez-source `std/` library were never committed. Every checkout that was not this working tree — CI included —
/// ran the import, export, package and runner-integrity tests against files
/// that did not exist, and got `ModuleNotFound` instead of a result.
///
/// The runners now refuse to start when a fixture is missing, which makes the
/// symptom legible. This test attacks the cause: a fixture that exists on disk
/// but is not tracked is exactly the state that produced the bug, and nothing
/// else notices it.
///
/// Skipped when git is unavailable or this is not a checkout (a source tarball,
/// a vendored build): the invariant is about the repository, so with no
/// repository there is nothing to assert.
#[test]
fn runner_fixtures_are_tracked_by_git() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let output = match std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "ls-files",
            "--",
            "tests/lib",
            "tests/packages",
            "tests/runner_fixtures",
            "std",
        ])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return, // no git, or not a checkout — nothing to assert
    };

    let tracked: std::collections::HashSet<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();

    // Not an exhaustive list on purpose: these are the ones whose absence
    // silently turned a language test into a "module not found" failure.
    let required = [
        "tests/lib/greet.sz",
        "tests/lib/greet_noexport.sz",
        "tests/lib/math_utils.sz",
        "tests/packages/serez.json",
        "tests/packages/math-helpers/index.sz",
        "tests/packages/string-tools/index.sz",
        "tests/runner_fixtures/unit_abort_before_summary.sz",
        // The Serez-source standard library. Twelve test files import it
        // through SEREZ_HOME, and it is shipped library source, not a
        // fixture: 476 lines that existed only in working trees.
        "std/collections.sz",
        "std/iter.sz",
        "std/math.sz",
        "std/result.sz",
        "std/string.sz",
    ];

    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|path| !tracked.contains(*path))
        .collect();

    assert!(
        missing.is_empty(),
        "these fixtures exist for the runners but are not tracked by git, so a \
         fresh clone will not have them: {missing:?}. Check .gitignore — the \
         `*.sz` / `*.json` rules need an explicit `!` for each fixture tree."
    );

    // Everything the module conformance suite imports must travel with it too.
    let module_fixtures: Vec<&String> = tracked
        .iter()
        .filter(|path| path.starts_with("tests/lib/mod_"))
        .collect();
    assert!(
        module_fixtures.len() >= 8,
        "tests/unit_modules.sz imports eight tests/lib/mod_* fixtures; git tracks \
         {}: {module_fixtures:?}",
        module_fixtures.len()
    );
}

/// The grammar frozen in `spec/syntax.md`, both halves.
///
/// A specification that says "this does not parse" is only a contract if
/// something fails when it starts to. These are the forms that read as
/// obviously valid and are not, alongside the ones they are usually confused
/// with — pinned together so a parser change cannot quietly move a case from
/// one column to the other.
#[test]
fn the_documented_grammar_is_what_the_parser_accepts() {
    fn parses(src: &str) -> bool {
        let lexer = Lexer::new(src.to_string());
        let mut parser = Parser::new(lexer);
        parser.set_source(src.lines().map(str::to_string).collect());
        parser.set_source_name("<syntax>");
        let _ = parser.parse_program();
        !parser.has_errors()
    }

    // Forms spec/syntax.md records as rejected.
    let rejected = [
        ("brace-less if body", "if (true) out 1;"),
        ("brace-less else body", "if (true) { } else out 1;"),
        ("brace-less while body", "while (false) out 1;"),
        ("brace-less for body", "for (let i = 0; i < 1; i++) out 1;"),
        ("for-in without let", "for (item in [1]) { }"),
        (
            "typed lambda parameter",
            "let f = (int a) => { return a; };",
        ),
        (
            "lambda parameter with a default",
            "let f = (a = 1) => { return a; };",
        ),
        ("scalar type on a let", "let x int = 5;"),
        ("nullable type on a let", "let n int? = null;"),
        ("class name as array element type", "let a [Base] = [];"),
        ("JSON-style object literal", "let d = {\"a\": 1};"),
        ("trailing comma in an array literal", "let a = [1, 2,];"),
        (
            "trailing comma in call arguments",
            "fn int f(int a) { return a; } f(1,);",
        ),
        (
            "trailing comma in a parameter list",
            "fn int f(int a,) { return a; }",
        ),
        (
            "trailing comma in a dict literal",
            "let d <string, int> = ({\"a\", 1},);",
        ),
        ("nested block comment", "/* a /* b */ c */ out 1;"),
        ("sizeof over an expression", "out sizeof(5);"),
    ];
    for (what, src) in rejected {
        assert!(
            !parses(src),
            "spec/syntax.md says this does not parse, but it does — {what}: {src}"
        );
    }

    // The neighbouring forms that do parse. Half the value of the list above is
    // that these keep working.
    let accepted = [
        ("if with a block", "if (true) { out 1; }"),
        (
            "else-if chain",
            "if (false) { } else if (false) { } else { }",
        ),
        ("for-in with let", "for (let item in [1]) { }"),
        ("classic for", "for (let i = 0; i < 1; i = i + 1) { }"),
        (
            "labeled break",
            "outer: for (let i = 0; i < 2; i++) { break outer; }",
        ),
        ("labeled continue", "w: while (false) { continue w; }"),
        ("untyped lambda parameter", "let f = (a) => { return a; };"),
        ("bare single lambda parameter", "let f = a => a * 2;"),
        ("typed array binding", "let a [int] = [1];"),
        ("typed dict binding", "let d <string, int> = ({\"a\", 1});"),
        (
            "array destructuring with rest",
            "let [a, ...rest] = [1, 2, 3];",
        ),
        (
            "array destructuring trailing comma",
            "let [a, b,] = [1, 2];",
        ),
        (
            "dict destructuring",
            "let d <string, int> = ({\"x\", 1}); let {x} = d;",
        ),
        (
            "match with a trailing comma",
            "let r = match 1 { 1 => \"a\", _ => \"b\", };",
        ),
        (
            "match with a guard",
            "let r = match 7 { n if n > 5 => \"big\", _ => \"small\" };",
        ),
        ("enum with a trailing comma", "enum E { A, B, }"),
        ("switch without default", "switch (1) { case 1: { } }"),
        (
            "switch with multiple case values",
            "switch (1) { case 1, 2: { } }",
        ),
        ("try/finally without catch", "try { } finally { }"),
        ("generator declaration", "fn* int g() { yield 1; }"),
        ("rest parameter", "fn int f(...rest) { return 1; }"),
        ("default parameter", "fn int f(int a = 1) { return a; }"),
        (
            "spread at a call site",
            "fn int f(...r) { return 1; } f(...[1, 2]);",
        ),
        (
            "getter and setter",
            "class C { public C() { this.v = 1; } \
             public get int val() { return this.v; } \
             public set val(int n) { this.v = n; } }",
        ),
        (
            "declared field with a default",
            "class C { n: int = 1; public C() { } }",
        ),
        ("interface declaration", "interface I { a: int; b: int; }"),
        (
            "nested declarations",
            "fn int f() { class C { public C() { } } return 1; }",
        ),
        ("statements without semicolons", "let x = 1\nout x\n"),
        ("sizeof over a type keyword", "out sizeof(int);"),
        ("bare block", "{ let x = 1; }"),
        ("empty program", ""),
    ];
    for (what, src) in accepted {
        assert!(
            parses(src),
            "spec/syntax.md says this parses, but it does not — {what}: {src}"
        );
    }
}

/// The reserved-word table in `spec/lexical-grammar.md` is the lexer's table.
///
/// `lexical-grammar.md` said keyword recognition was "exact and case-sensitive"
/// and then never listed the keywords, so a reader had no way to know that
/// `out`, `get` and `set` are unavailable as names. README.md used all three as
/// identifiers. The list exists now; this keeps it honest, because a documented
/// list that drifts is worse than none — it reads as authoritative.
#[test]
fn reserved_words_match_the_lexer() {
    use serez_code::token::{TokenType, lookup_ident};

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let doc = std::fs::read_to_string(root.join("spec").join("lexical-grammar.md"))
        .expect("spec/lexical-grammar.md must be readable");

    let section = doc
        .split("## Reserved words")
        .nth(1)
        .expect("spec/lexical-grammar.md must have a Reserved words section")
        .split("\n## ")
        .next()
        .unwrap();

    let mut documented: Vec<String> = Vec::new();
    for line in section.lines() {
        let line = line.trim();
        if !line.starts_with('|') || line.contains("---") {
            continue;
        }
        for cell in line.split('|') {
            let cell = cell.trim();
            if let Some(word) = cell.strip_prefix('`').and_then(|c| c.strip_suffix('`')) {
                documented.push(word.to_string());
            }
        }
    }
    documented.sort();
    documented.dedup();

    // Every documented word must really be a keyword.
    for word in &documented {
        assert!(
            !matches!(lookup_ident(word), TokenType::Ident),
            "spec/lexical-grammar.md lists `{word}` as reserved, but the lexer \
             treats it as an ordinary identifier"
        );
    }

    // And every keyword must be documented. The lexer's table is the source of
    // truth; this reads it back out of the one place that has it.
    let table = std::fs::read_to_string(root.join("src").join("token.rs"))
        .expect("src/token.rs must be readable");
    let body = table
        .split("pub fn lookup_ident")
        .nth(1)
        .expect("lookup_ident must exist");
    let mut actual: Vec<String> = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if !line.contains("=> TokenType::") {
            continue;
        }
        if let Some(word) = line.split('"').nth(1) {
            if word.chars().all(|c| c.is_ascii_lowercase()) && !word.is_empty() {
                actual.push(word.to_string());
            }
        }
        if line.starts_with('_') {
            break;
        }
    }
    actual.sort();
    actual.dedup();

    let missing: Vec<&String> = actual.iter().filter(|w| !documented.contains(w)).collect();
    assert!(
        missing.is_empty(),
        "these keywords are not in spec/lexical-grammar.md's Reserved words \
         table: {missing:?}"
    );
    assert_eq!(
        documented.len(),
        actual.len(),
        "the documented table and the lexer disagree\n  documented: {documented:?}\n  lexer: {actual:?}"
    );

    // The count in the prose has to move with the table.
    let count_line = format!("These {} words are keywords", actual.len());
    assert!(
        section.contains(&count_line),
        "spec/lexical-grammar.md must say \"{count_line}\""
    );
}

/// Every `serez` example in README.md parses, unless it says it should not.
///
/// The README is what people copy. Nothing checked it, and five examples had
/// drifted into syntax the language does not accept: a dict literal without an
/// annotated binding, `fn any get()` (a keyword), `while cond {` without
/// parentheses, `public abstract decimal area();` — which the README's own
/// Known Gotchas section already said was unsupported — and `let name: string`.
///
/// A block that is *meant* to be invalid opts out with a first-line comment
/// `// parse-error-example: <why>`. The marker is explicit rather than inferred
/// from a ⚠️ in the text, because one of the broken blocks carried a ⚠️ for an
/// unrelated reason and would have been skipped for the wrong one.
#[test]
fn readme_serez_examples_parse() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let readme =
        std::fs::read_to_string(root.join("README.md")).expect("README.md must be readable");

    let lines: Vec<&str> = readme.lines().collect();
    let mut blocks: Vec<(usize, String)> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if lines[i].trim() == "```serez" {
            let start = i + 2; // 1-based line of the first content line
            let mut j = i + 1;
            let mut buf: Vec<&str> = Vec::new();
            while j < lines.len() && lines[j].trim() != "```" {
                buf.push(lines[j]);
                j += 1;
            }
            blocks.push((start, buf.join("\n")));
            i = j + 1;
        } else {
            i += 1;
        }
    }

    assert!(
        blocks.len() > 150,
        "expected the README to still carry its examples, found {}",
        blocks.len()
    );

    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for (line, src) in &blocks {
        if src.contains("parse-error-example") {
            continue;
        }
        checked += 1;
        let lexer = serez_code::lexer::Lexer::new(src.clone());
        let mut parser = serez_code::parser::Parser::new(lexer);
        parser.set_source(src.lines().map(str::to_string).collect());
        parser.set_source_name("<readme>");
        let _ = parser.parse_program();
        if parser.has_errors() {
            let first = parser
                .take_errors()
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_default();
            failures.push(format!("README.md:{line}: {first}"));
        }
    }

    assert!(
        checked > 150,
        "almost every block opted out of parsing; that is not the intent ({checked} checked)"
    );
    assert!(
        failures.is_empty(),
        "README examples that do not parse — fix the example, or mark the block \
         with a first-line `// parse-error-example: <why>` if being invalid is \
         the point:\n{}",
        failures.join("\n")
    );
}

/// The same guard the README has, over `spec/`.
///
/// `spec/` is the normative contract: an example there that cannot run is worse
/// than one in a guide, because a reader takes it for the definition. Nothing
/// checked those 46 blocks. Five did not parse — all in `syntax.md`, all
/// deliberately invalid, and two of the five already said so. The three that did
/// not are now marked, which is the point of the marker: being invalid has to be
/// stated, not inferred.
///
/// This parses; it does not run. The defect that prompted it would have slipped
/// through either way — `values.md` carried a writeback example whose dict
/// literal parses fine and fails at runtime with `SZ4002`, because a dict
/// literal is not an expression. Parsing is the cheap half, and it is the half
/// that catches syntax drifting away from 25 documents.
#[test]
fn spec_serez_examples_parse() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec_dir = root.join("spec");

    let mut documents: Vec<std::path::PathBuf> = std::fs::read_dir(&spec_dir)
        .expect("spec/ must be readable")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    documents.sort();
    assert!(
        documents.len() > 20,
        "expected the spec to still be there, found {}",
        documents.len()
    );

    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for document in &documents {
        let text = std::fs::read_to_string(document).expect("a spec document must be readable");
        let name = document
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            if lines[i].trim() != "```serez" {
                i += 1;
                continue;
            }
            let start = i + 2; // 1-based line of the first content line
            let mut j = i + 1;
            let mut buf: Vec<&str> = Vec::new();
            while j < lines.len() && lines[j].trim() != "```" {
                buf.push(lines[j]);
                j += 1;
            }
            i = j + 1;

            let src = buf.join("\n");
            if src.contains("parse-error-example") {
                continue;
            }
            checked += 1;
            let lexer = serez_code::lexer::Lexer::new(src.clone());
            let mut parser = serez_code::parser::Parser::new(lexer);
            parser.set_source(src.lines().map(str::to_string).collect());
            parser.set_source_name("<spec>");
            let _ = parser.parse_program();
            if parser.has_errors() {
                let first = parser
                    .take_errors()
                    .first()
                    .map(|e| e.message.clone())
                    .unwrap_or_default();
                failures.push(format!("spec/{name}:{start}: {first}"));
            }
        }
    }

    assert!(
        checked > 30,
        "almost every block opted out of parsing; that is not the intent \
         ({checked} checked)"
    );
    assert!(
        failures.is_empty(),
        "spec examples that do not parse — fix the example, or mark the block \
         with a first-line `// parse-error-example: <why>` if being invalid is \
         the point:\n{}",
        failures.join("\n")
    );
}

/// The half `spec_serez_examples_parse` cannot cover: a block that parses and
/// then does not run.
///
/// That is not hypothetical. `values.md` carried a receiver-writeback example
/// that parsed cleanly and failed at runtime with `SZ4002`, because it built a
/// dict literal inside a field initialiser and a dict literal is not an
/// expression. The parse checker was blind to it, and said so.
///
/// Every `serez` block in `spec/` must now either run to completion or say why
/// it does not, with a first-line marker:
///
/// - `parse-error-example: why` — the source is deliberately not valid syntax;
/// - `runtime-error-example: why` — it parses and is meant to fail, because the
///   failure is the thing being documented;
/// - `fragment: why` — it continues an earlier block, or needs files beside it.
///
/// Measured when this was written: of 41 blocks, 23 ran unaided and 18 needed a
/// marker — eight deliberate runtime-error demonstrations and ten fragments,
/// mostly multi-file module examples. None of the 18 was wrong; they were simply
/// never distinguished from the blocks a reader is meant to be able to paste.
#[test]
fn spec_serez_examples_run() {
    use std::process::Command;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut documents: Vec<std::path::PathBuf> = std::fs::read_dir(root.join("spec"))
        .expect("spec/ must be readable")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .collect();
    documents.sort();

    let scratch = std::env::temp_dir().join(format!("sz_spec_run_{}", std::process::id()));
    std::fs::create_dir_all(&scratch).expect("a scratch directory");
    let probe = scratch.join("block.sz");

    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0usize;

    for document in &documents {
        let text = std::fs::read_to_string(document).expect("a spec document must be readable");
        let name = document
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            if lines[i].trim() != "```serez" {
                i += 1;
                continue;
            }
            let start = i + 2;
            let mut j = i + 1;
            let mut buf: Vec<&str> = Vec::new();
            while j < lines.len() && lines[j].trim() != "```" {
                buf.push(lines[j]);
                j += 1;
            }
            i = j + 1;

            let src = buf.join("\n");
            if src.contains("parse-error-example")
                || src.contains("runtime-error-example")
                || src.contains("fragment:")
            {
                continue;
            }
            ran += 1;
            std::fs::write(&probe, format!("{src}\n")).expect("the probe must be writable");
            let out = Command::new(env!("CARGO_BIN_EXE_sz"))
                .arg(&probe)
                .current_dir(&scratch)
                .output()
                .expect("the sz binary must run");
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let first = stderr
                    .lines()
                    .find(|line| line.contains("ERROR") || line.contains("EXCEPTION"))
                    .unwrap_or("(no diagnostic)");
                failures.push(format!("spec/{name}:{start}: {first}"));
            }
        }
    }

    let _ = std::fs::remove_dir_all(&scratch);
    assert!(
        ran > 15,
        "almost every block opted out of running; that is not the intent ({ran} ran)"
    );
    assert!(
        failures.is_empty(),
        "spec examples that parse but do not run — fix the example, or mark the \
         block with a first-line `// runtime-error-example: <why>` when failing \
         is the point, or `// fragment: <why>` when it continues another \
         block:\n{}",
        failures.join("\n")
    );
}

/// `permissions::ENFORCED` is the list `require_permission` actually checks.
///
/// The nine enforced namespaces existed only as string literals at their call
/// sites. Naming them in one place is only an improvement while the name stays
/// true: a list that drifts would make `grant_warning` tell an author their
/// correct declaration does nothing, which is worse than the silence it
/// replaced.
#[test]
fn enforced_permissions_match_the_evaluator() {
    use serez_code::permissions::ENFORCED;

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found: Vec<String> = Vec::new();

    for entry in walk_rust_sources(&root.join("src")) {
        let source = std::fs::read_to_string(&entry).unwrap_or_default();
        // `require_permission("OS.exec", "OS")` — the second argument is the
        // permission; the first is the operation being reported.
        for (index, _) in source.match_indices("require_permission(") {
            // Skip the definition itself: its body holds the format string that
            // every call site's message comes from, and the scan would read that
            // as a permission name.
            if source[..index].trim_end().ends_with("fn") {
                continue;
            }
            let tail = &source[index..];
            let args: Vec<&str> = tail.split('"').skip(1).step_by(2).take(2).collect();
            if let Some(permission) = args.get(1) {
                found.push((*permission).to_string());
            }
        }
    }
    found.sort();
    found.dedup();

    assert!(
        !found.is_empty(),
        "no require_permission call sites found — the scan itself broke"
    );

    let mut declared: Vec<String> = ENFORCED.iter().map(|p| (*p).to_string()).collect();
    declared.sort();

    assert_eq!(
        declared, found,
        "permissions::ENFORCED and the require_permission call sites disagree.\n  \
         declared: {declared:?}\n  in source: {found:?}"
    );
}

fn walk_rust_sources(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_rust_sources(&path));
        } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
            out.push(path);
        }
    }
    out
}
