//! What a program declares, according to the program.
//!
//! # Why this module exists
//!
//! M4's charter asks for resolver, symbols and scopes to live outside the
//! parser. The M4.0 audit found they do not live in the parser either — they do
//! not live anywhere. Four things improvise the answer, from three different
//! inputs, and `docs/maturity/ROADMAP_STATE.md` §9F.0 lists them. §9F.2 measured
//! what one pair disagreeing costs: **95 of 483 files**, because the editor's
//! outline is built from a token scan that cannot see nesting.
//!
//! So this is not an extraction. It is the missing layer, introduced the way
//! `span` and `diagnostic` were: as a leaf with **no consumers**, so that
//! adopting it is a separate, verifiable step for each caller rather than a flag
//! day.
//!
//! # What it does, and deliberately does not do yet
//!
//! It answers one question: *what does this program declare, and where?* It
//! walks the AST — the real tree, not a token stream — so nesting is a fact it
//! knows rather than a heuristic it guesses.
//!
//! It does **not** resolve uses to declarations. There is no resolver in the
//! language today: `name -> declaration` is answered once, at run time, by
//! `scope.rs::ScopeStack`. That is why free variables resolve dynamically and
//! `--check` cannot flag them. A static resolver would change which programs are
//! accepted, and §6 records that as needing an explicit product decision.
//! Collecting declarations changes nothing, so it comes first.
//!
//! It also reports **no diagnostics**. A collector that can reject is a
//! validator, and which names are legal is exactly the question M4 may not
//! answer on its own — `is_reserved_name` covers 7 of 22 namespaces, and
//! changing that is breaking (§5.20).

use crate::ast::{Program, Statement};
use crate::span::Span;

/// What kind of thing a name was declared as.
///
/// Deliberately narrower than the LSP's `SymbolKind`: this enumerates what the
/// *language* declares. No `Import` — that is a path, not a name — and no
/// `Constructor`, which has no name of its own. A consumer needing the editor's
/// categories maps onto them; the reverse would bake an editor's vocabulary into
/// the language's model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SymbolKind {
    Function,
    /// `native fn …;` — declared here, defined outside the language.
    NativeFunction,
    Class,
    Interface,
    Enum,
    /// A variant of an enum. Its container is the enum.
    EnumVariant,
    Method,
    Field,
    /// `let`.
    Variable,
    /// `const`.
    Constant,
}

impl SymbolKind {
    /// Can this kind introduce a name other code refers to by that name alone,
    /// with no receiver?
    ///
    /// False for members: a `Field`, a `Method` and an `EnumVariant` are all
    /// reached through something else.
    pub fn is_free_standing(&self) -> bool {
        !matches!(
            self,
            SymbolKind::Method | SymbolKind::Field | SymbolKind::EnumVariant
        )
    }
}

/// One declared name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Where the declaration is. Taken from the AST node, so it is the same span
    /// a diagnostic about that declaration would carry.
    pub span: Span,
    /// The class or enum this belongs to, for members. `None` when free-standing.
    pub container: Option<String>,
    /// How deeply nested the declaration is: `0` at the top level, `1` inside
    /// one enclosing body, and so on.
    ///
    /// This is the fact a token scan cannot recover, and the whole reason the
    /// editor's outline disagrees with the tree in a fifth of the corpus
    /// (§9F.2). A `fn` inside a lambda passed to `test(…)` has a non-zero depth
    /// and is not a top-level symbol.
    pub depth: usize,
}

/// Every name a program declares, in source order.
///
/// Source order rather than sorted: it is the order a reader sees, and a
/// consumer wanting another can sort. A nested declaration follows the one that
/// encloses it.
pub fn declarations(program: &Program) -> Vec<Symbol> {
    let mut out = Vec::new();
    collect_statements(&program.statements, 0, &mut out);
    out
}

/// The names a program introduces at its top level.
///
/// The common case for an outline, and the one §9F.2 measures.
pub fn top_level(program: &Program) -> Vec<Symbol> {
    declarations(program)
        .into_iter()
        .filter(|s| s.depth == 0 && s.kind.is_free_standing())
        .collect()
}

fn collect_statements(statements: &[Statement], depth: usize, out: &mut Vec<Symbol>) {
    for statement in statements {
        collect_statement(statement, depth, out);
    }
}

fn collect_statement(statement: &Statement, depth: usize, out: &mut Vec<Symbol>) {
    match statement {
        // `export` wraps a declaration. The wrapper is not itself a name, and
        // what it wraps is declared at the same depth.
        Statement::Export(inner) => collect_statement(inner, depth, out),

        Statement::FunctionDeclaration(f) => {
            out.push(Symbol {
                name: f.name.clone(),
                kind: SymbolKind::Function,
                span: f.span,
                container: None,
                depth,
            });
            collect_statements(&f.function.body.statements, depth + 1, out);
        }
        Statement::NativeDeclaration(n) => out.push(Symbol {
            name: n.name.clone(),
            kind: SymbolKind::NativeFunction,
            span: n.span,
            container: None,
            depth,
        }),

        Statement::ClassDeclaration(c) => {
            out.push(Symbol {
                name: c.name.clone(),
                kind: SymbolKind::Class,
                span: c.span,
                container: None,
                depth,
            });
            for field in &c.fields {
                out.push(Symbol {
                    name: field.name.clone(),
                    kind: SymbolKind::Field,
                    span: field.span,
                    container: Some(c.name.clone()),
                    depth,
                });
            }
            for method in &c.methods {
                out.push(Symbol {
                    name: method.name.clone(),
                    kind: SymbolKind::Method,
                    span: method.span,
                    container: Some(c.name.clone()),
                    depth,
                });
                collect_statements(&method.body.statements, depth + 1, out);
            }
            if let Some(constructor) = &c.constructor {
                collect_statements(&constructor.body.statements, depth + 1, out);
            }
        }

        Statement::InterfaceDeclaration(i) => out.push(Symbol {
            name: i.name.clone(),
            kind: SymbolKind::Interface,
            span: i.span,
            container: None,
            depth,
        }),

        Statement::EnumDeclaration(e) => {
            out.push(Symbol {
                name: e.name.clone(),
                kind: SymbolKind::Enum,
                span: e.span,
                container: None,
                depth,
            });
            for variant in &e.variants {
                out.push(Symbol {
                    name: variant.clone(),
                    kind: SymbolKind::EnumVariant,
                    // The variant list is `Vec<String>`: the AST carries no span
                    // per variant, so the enum's own is the closest true answer.
                    // Inventing a narrower one would be a guess.
                    span: e.span,
                    container: Some(e.name.clone()),
                    depth,
                });
            }
        }

        Statement::Let(l) => out.push(binding(l.name.clone(), l.is_const, l.span, depth)),

        // Everything that owns statements. A body is one level deeper than the
        // construct that owns it.
        Statement::Block(b) | Statement::Unsafe(b) => {
            collect_statements(&b.statements, depth + 1, out)
        }
        Statement::While(w) | Statement::DoWhile(w) => {
            collect_statements(&w.body.statements, depth + 1, out)
        }
        Statement::For(f) => collect_statements(&f.body.statements, depth + 1, out),
        Statement::ForEach(f) => collect_statements(&f.body.statements, depth + 1, out),

        // Declares nothing and owns no statements. An expression can hold a
        // function *literal* — `let f = () => { fn g() {} }` — but a literal is
        // not a declaration, and whether an outline should reach into one is a
        // separate question. Recorded rather than guessed at.
        _ => {}
    }
}

fn binding(name: String, is_const: bool, span: Span, depth: usize) -> Symbol {
    Symbol {
        name,
        kind: if is_const {
            SymbolKind::Constant
        } else {
            SymbolKind::Variable
        },
        span,
        container: None,
        depth,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn parse(source: &str) -> Program {
        Parser::new(Lexer::new(source.to_string())).parse_program()
    }

    fn names(source: &str) -> Vec<String> {
        top_level(&parse(source))
            .into_iter()
            .map(|s| s.name)
            .collect()
    }

    #[test]
    fn the_top_level_forms_are_all_found() {
        let source = "\
fn int f(int a) { return a; }
class C { x: int = 1; }
interface I { y: int; }
enum E { Red, Green }
let v = 1;
const k = 2;
";
        assert_eq!(names(source), ["f", "C", "I", "E", "v", "k"]);
    }

    #[test]
    fn a_nested_function_is_not_a_top_level_symbol() {
        // The exact shape behind §9F.2's 95 files: the token scanner reports
        // `inner` with no container, as though it were declared at the top.
        let source = "fn outer() { fn inner() { out 1; } }\n";
        assert_eq!(names(source), ["outer"]);

        let all = declarations(&parse(source));
        let inner = all.iter().find(|s| s.name == "inner").expect("inner lost");
        assert_eq!(inner.depth, 1, "a function inside a function is depth 1");
        assert_eq!(all[0].depth, 0);
    }

    #[test]
    fn members_are_attributed_to_their_container_and_are_not_free_standing() {
        let source = "\
class Point {
    x: int = 0;
    public int get() { return 1; }
}
";
        let all = declarations(&parse(source));

        let field = all.iter().find(|s| s.name == "x").expect("field lost");
        assert_eq!(field.kind, SymbolKind::Field);
        assert_eq!(field.container.as_deref(), Some("Point"));
        assert!(!field.kind.is_free_standing());

        let method = all.iter().find(|s| s.name == "get").expect("method lost");
        assert_eq!(method.kind, SymbolKind::Method);
        assert_eq!(method.container.as_deref(), Some("Point"));

        // Only the class itself reaches an outline.
        assert_eq!(names(source), ["Point"]);
    }

    #[test]
    fn enum_variants_belong_to_the_enum() {
        let all = declarations(&parse("enum Color { Red, Green }\n"));
        let red = all.iter().find(|s| s.name == "Red").expect("variant lost");
        assert_eq!(red.kind, SymbolKind::EnumVariant);
        assert_eq!(red.container.as_deref(), Some("Color"));
        assert_eq!(names("enum Color { Red, Green }\n"), ["Color"]);
    }

    #[test]
    fn export_declares_the_thing_it_wraps_at_the_same_depth() {
        let all = declarations(&parse("export fn f() { out 1; }\n"));
        assert_eq!(all.len(), 1, "the wrapper must not be a symbol of its own");
        assert_eq!(all[0].name, "f");
        assert_eq!(all[0].kind, SymbolKind::Function);
        assert_eq!(all[0].depth, 0);
    }

    #[test]
    fn const_and_let_are_told_apart() {
        let all = declarations(&parse("let v = 1;\nconst k = 2;\n"));
        assert_eq!(all[0].kind, SymbolKind::Variable);
        assert_eq!(all[1].kind, SymbolKind::Constant);
    }

    #[test]
    fn a_declaration_carries_the_span_the_ast_gave_it() {
        let all = declarations(&parse("\nfn f() { out 1; }\n"));
        assert_eq!(all[0].span.line, 2, "the declaration is on line 2");
        assert!(all[0].span.column >= 1, "columns are 1-based");
    }

    #[test]
    fn a_body_inside_a_loop_or_a_block_counts_as_nesting() {
        for source in [
            "while (true) { fn f() { out 1; } }\n",
            "{ fn f() { out 1; } }\n",
            "for (let i = 0; i < 1; i = i + 1) { fn f() { out 1; } }\n",
        ] {
            assert!(
                names(source).is_empty(),
                "{source:?} reported a nested declaration as top level"
            );
            let all = declarations(&parse(source));
            let f = all.iter().find(|s| s.name == "f");
            assert!(f.is_some(), "{source:?}: the declaration was lost entirely");
            assert!(f.unwrap().depth >= 1, "{source:?}: depth was not counted");
        }
    }
}
