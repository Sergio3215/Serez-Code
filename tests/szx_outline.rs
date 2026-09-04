//! What the editor's outline can say about a `.szx` file.
//!
//! # Why `.szx` is scanned rather than parsed
//!
//! DEC-M4-004 moved the `.sz` outline onto `semantic::declarations`, which reads
//! the tree the compiler builds. `.szx` is JSX and this frontend does not parse
//! it — `modules::load_source` translates it by running the serez-ui translator
//! as a subprocess, which is not something an editor can do on every keystroke.
//! So `.szx` still goes through `scan_symbols`, a token walk.
//!
//! **This file does not change that**, and does not try to. Building a
//! structural JSX frontend is registered as debt (§5.53) rather than improvised
//! here. What it does is measure what the scan actually produces, over a corpus
//! that did not exist, and fix what the scan could recover and was not.
//!
//! # What was measured before
//!
//! Every symbol arrived with `depth: 0`, hard-coded, under a comment saying the
//! token scan could not see nesting — while the scanner was tracking brace depth
//! two lines above. And `container` was only ever a class, so:
//!
//! ```text
//!   depth=0 Function outer          container=None
//!   depth=0 Variable insideFn       container=None
//!   depth=0 Function inner          container=None     <- nested in outer
//!   depth=0 Variable insideInner    container=None     <- nested two deep
//!   depth=0 Variable insideMethod   container=Panel    <- the class, not render
//! ```
//!
//! `depth == 0` is what `analysis` uses to decide a symbol is top-level, so a
//! `fn` nested two levels down was offered as a top-level name. And no field of
//! any class in the four fixtures appeared at all.
//!
//! # What the fixtures are
//!
//! `tests/szx/` — four files shaped like the ecosystem's real ones
//! (`serez-ui/apps/counter.szx`, `serez-strike/app.szx`, the `proyecto03`
//! demos): a component class, four levels of nesting, Serez expressions inside
//! JSX braces, and one of each top-level declaration form. The corpus is the
//! point: the scan was previously unmeasured against `.szx` at all.

use serez_code::lsp::analysis::{self, SymbolInfo, SymbolKind};

fn outline(fixture: &str) -> Vec<SymbolInfo> {
    let path = format!("tests/szx/{}.szx", fixture);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {}", path, e));
    analysis::analyze_szx(&text).symbols
}

fn find<'a>(symbols: &'a [SymbolInfo], name: &str) -> &'a SymbolInfo {
    symbols
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no symbol named '{}' in {:?}", name, names(symbols)))
}

fn names(symbols: &[SymbolInfo]) -> Vec<&str> {
    symbols.iter().map(|s| s.name.as_str()).collect()
}

/// Nesting depth is the fact the outline uses to decide what is top-level.
#[test]
fn a_nested_declaration_is_not_reported_at_depth_zero() {
    let symbols = outline("nesting");

    assert_eq!(find(&symbols, "outer").depth, 0, "a top-level fn is at 0");
    assert_eq!(find(&symbols, "TOP").depth, 0, "a top-level const is at 0");

    assert_eq!(find(&symbols, "insideFn").depth, 1);
    assert_eq!(
        find(&symbols, "inner").depth,
        1,
        "a fn inside a fn is nested"
    );
    assert_eq!(find(&symbols, "insideInner").depth, 2);
    assert_eq!(
        find(&symbols, "insideLambda").depth,
        1,
        "a lambda body nests"
    );
}

/// And its container is what encloses it, not the nearest class.
#[test]
fn a_nested_declaration_names_its_enclosing_scope() {
    let symbols = outline("nesting");

    assert_eq!(find(&symbols, "outer").container, None);
    assert_eq!(
        find(&symbols, "insideFn").container.as_deref(),
        Some("outer")
    );
    assert_eq!(find(&symbols, "inner").container.as_deref(), Some("outer"));
    assert_eq!(
        find(&symbols, "insideInner").container.as_deref(),
        Some("inner"),
        "a declaration two levels down was attributed to the wrong scope"
    );
    assert_eq!(
        find(&symbols, "insideMethod").container.as_deref(),
        Some("render"),
        "a method-local was attributed to the class, which puts it beside the \
         method in the outline instead of inside it"
    );
}

/// A `fn` nested inside another is a function, not a method.
///
/// The container and the kind answer different questions, and merging the two
/// stacks that supply them would have made every nested `fn` a method of
/// whatever class happened to be open.
#[test]
fn a_nested_function_is_still_a_function() {
    let symbols = outline("nesting");
    assert_eq!(find(&symbols, "inner").kind, SymbolKind::Function);
    assert_eq!(find(&symbols, "render").kind, SymbolKind::Method);
    assert_eq!(find(&symbols, "Panel").kind, SymbolKind::Class);
}

/// The component shape every serez-ui app has.
#[test]
fn a_component_class_reports_its_members() {
    let symbols = outline("component");

    let class = find(&symbols, "Counter");
    assert_eq!(class.kind, SymbolKind::Class);
    assert_eq!(class.depth, 0);

    let count = find(&symbols, "count");
    assert_eq!(
        count.kind,
        SymbolKind::Field,
        "a class field was missing entirely"
    );
    assert_eq!(count.container.as_deref(), Some("Counter"));

    let render = symbols
        .iter()
        .find(|s| s.name == "render" && s.kind == SymbolKind::Method)
        .expect("render");
    assert_eq!(render.container.as_deref(), Some("Counter"));

    // The constructor shares the class's name, which is why the outline walks by
    // index rather than by name — see `document_symbols`.
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "Counter" && s.kind == SymbolKind::Constructor),
        "no constructor in {:?}",
        names(&symbols)
    );
}

/// Every top-level declaration form reaches the outline.
#[test]
fn each_declaration_form_appears_once_at_the_top() {
    let symbols = outline("declarations");
    for (name, kind) in [
        ("VERSION", SymbolKind::Constant),
        ("mutable", SymbolKind::Variable),
        ("Mode", SymbolKind::Enum),
        ("Renderable", SymbolKind::Interface),
        ("greet", SymbolKind::Function),
        ("Widget", SymbolKind::Class),
    ] {
        let s = find(&symbols, name);
        assert_eq!(s.kind, kind, "{} has the wrong kind", name);
        assert_eq!(s.depth, 0, "{} is not reported at the top level", name);
    }

    // Destructuring binds both names.
    assert_eq!(find(&symbols, "first").depth, 0);
    assert_eq!(find(&symbols, "second").depth, 0);
}

/// Serez expressions inside JSX braces do not throw the scan off its place.
///
/// `{c}`, `{"a" + b}`, `onChange={(v) => { … }}` — every one of these opens and
/// closes braces the scan counts. If they did not balance, everything after the
/// first JSX block in the file would be reported at the wrong depth.
#[test]
fn jsx_expression_braces_leave_the_depth_balanced() {
    let symbols = outline("expressions");

    // Declared after a render() full of JSX braces and lambdas.
    let form = find(&symbols, "form");
    assert_eq!(
        form.depth, 0,
        "a top-level `let` after a JSX block was reported as nested, so the \
         braces inside the JSX did not balance"
    );
    assert_eq!(form.container, None);

    assert_eq!(
        find(&symbols, "title").depth,
        2,
        "a method-local in render()"
    );
    assert_eq!(find(&symbols, "title").container.as_deref(), Some("render"));
}

/// The controls: an ordinary file is not over-nested, and nothing is invented.
#[test]
fn a_flat_file_stays_flat() {
    let symbols = outline("declarations");
    let nested: Vec<&str> = symbols
        .iter()
        .filter(|s| s.depth > 0 && s.container.is_none())
        .map(|s| s.name.as_str())
        .collect();
    assert!(
        nested.is_empty(),
        "symbols reported as nested with nothing containing them: {:?}",
        nested
    );
}

/// An `import` is reported, and at the top.
#[test]
fn an_import_is_reported() {
    let symbols = outline("component");
    let import = find(&symbols, "serez-ui");
    assert_eq!(import.kind, SymbolKind::Import);
    assert_eq!(import.depth, 0);
}

/// Unbalanced braces must not panic or produce a nonsense depth.
///
/// A `.szx` being typed is unbalanced most of the time, and the brace counter is
/// signed so that it recovers when the missing `{` arrives. An outline depth
/// cannot be negative, so it clamps.
#[test]
fn a_file_with_more_closing_braces_than_opening_ones_is_survivable() {
    let text = "class A {\n  x = 1\n}\n}\n}\nlet after = 1\n";
    let symbols = analysis::analyze_szx(text).symbols;
    let after = symbols
        .iter()
        .find(|s| s.name == "after")
        .expect("the declaration after the stray braces");
    assert_eq!(
        after.depth, 0,
        "a symbol after unbalanced braces must still be reachable in the outline"
    );
}
