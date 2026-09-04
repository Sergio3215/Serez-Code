//! The semantic phase: rules about *meaning* that reject a program.
//!
//! # Why this exists
//!
//! Serez had exactly two modes of rejection. **Syntactic** — fatal, in the parser.
//! And **type** — advisory, which by contract in `spec/types.md` rejects nothing:
//! `sz file.sz` prints an `SZ3000` and runs the program anyway. There was no
//! third.
//!
//! So every rule about what a program *means* that needs to reject it had to
//! disguise itself as a rule about what it *looks like*. `Parser::is_reserved_name`
//! is the one that did, and the disguise is visible in its output: rejecting
//! `class Task { … }` produces the real error plus two invented ones, because the
//! parser abandons a half-built declaration in order to say something that is not
//! about structure at all.
//!
//! **DEC-M4-001**, decided 2026-09-03, is the choice to have this phase.
//! `docs/maturity/ROADMAP_STATE.md` §7A carries the rationale and the alternatives
//! that were rejected.
//!
//! # Where it sits
//!
//! ```text
//! Lexer → Parser → AST → [semantic] → TypeChecker → Evaluator
//!                         ↑ here      ↑ advisory     ↑ authoritative
//! ```
//!
//! After the parser, because it needs a complete tree — that is the whole point,
//! and what makes one error out of three possible. Before the type checker,
//! because it is *fatal* and the checker is not: a program this phase rejects must
//! not reach a stage whose findings are allowed to be ignored.
//!
//! It runs only on a tree the parser accepted. Validating a broken tree would
//! report consequences of the syntax error rather than problems of its own.
//!
//! # What it is not
//!
//! Not a type checker: it answers questions with yes-or-no answers that do not
//! need inference. Not a place to move checks that are genuinely syntactic; the
//! parser rejecting `let = 3` stays where it is.
//!
//! # The rules it holds
//!
//! | Rule | Decision |
//! |---|---|
//! | A reserved runtime namespace as a `class`/`interface`/`enum` name | DEC-M4-001 |
//! | A name declared twice in one scope | §5.39 |
//! | A class parent that resolves nowhere | §5.39 |
//! | **A name that resolves nowhere** | **DEC-M4-002** |
//!
//! # DEC-M4-002 changed reachability, not the evaluator
//!
//! Worth being exact about, because it is easy to read the rule as a change to
//! how Serez resolves names. It is not. `ScopeStack::lookup` still walks the
//! frame stack, a call still pushes onto it, and an evaluator driven directly —
//! as `tests/runtime_outcome.rs` drives it — still resolves a free name from the
//! caller's frame.
//!
//! What changed is which programs reach the evaluator. `run::run_source_detailed`
//! runs this phase first and refuses to evaluate a program it rejects, so through
//! `sz` the dynamic path is unreachable. A program that passes this phase behaves
//! exactly as it did.
//!
//! That is the smallest implementation that delivers the decision, and it is the
//! reason the change carries no risk to the runtime: nothing in the evaluation
//! model moved.

use crate::ast::{Program, Statement};
use crate::diagnostic::{Diagnostic, Phase};
use crate::semantic::scopes::{self, UseKind};
use crate::span::Span;
use std::collections::HashMap;

/// Generic semantic diagnostic: a meaning-level rejection not yet given a
/// narrower code.
///
/// `SZ8000` is the generic code of its range, the way `SZ2000` and `SZ3000` are
/// of theirs. `spec/errors.md` sets the rule: a message moves to a narrower code
/// only once a test pins what that code means, so `SZ8001`+ get allocated when
/// there is something to pin, not in advance. DEC-M4-005.
pub const SZ_SEMANTIC_ERROR: &str = "SZ8000";

/// Runtime namespaces that may not be shadowed by a user declaration.
///
/// Seven of the twenty-two the runtime actually has. That the list is a subset,
/// and which subset, is **DEC-M4-003** — still open, and deliberately not
/// answered by moving the rule here. §5.31 has the measurement: a program may
/// declare `class Math`, call `new Math(42)`, and still call `Math.floor(3.7)`,
/// because `Math` is one of the fifteen this list omits.
const RESERVED_NAMESPACES: &[&str] = &["Task", "Time", "DateTime", "System", "Gui", "Dec", "Media"];

fn reserved(name: &str) -> bool {
    RESERVED_NAMESPACES.contains(&name)
}

/// Every semantic problem in `program`, in source order.
///
/// Empty means the program is semantically valid *as far as this phase checks*,
/// which today is: nothing. The phase is wired into the pipeline before it has
/// any rules, deliberately — see the module docs and M4.5.2. Introducing the
/// stage and introducing a rule are separate changes, and the first one must be
/// provably invisible before the second one is trusted.
///
/// A caller treats a non-empty result as **fatal**: the program does not run. That
/// is the distinction from `TypeChecker`, whose findings are advisory.
pub fn validate(program: &Program) -> Vec<Diagnostic> {
    validate_in(program, None, None)
}

/// The same rules, with the entry file's location so `import` can be followed.
///
/// Without a directory, a file containing any `import` has its names left
/// unchecked — a genuinely undefined name escapes to a runtime error, after
/// whatever ran before it has already run. With one, the imported modules are
/// read and their contributed names are known, so a name is *local*, *imported*
/// or *unresolved* rather than "there were imports, so never mind". §5.50.
///
/// `current_dir` is the entry file's parent and `entry` is the file itself,
/// which is what `Evaluator::set_current_file` gives the runtime. Passing the
/// same pair makes the two resolve the same files.
///
/// A caller that cannot read the filesystem — a locked-down run, an unsaved
/// editor buffer — passes `None` and gets exactly the behaviour this phase had
/// before: conclusive for a file with no imports, silent for one with any.
pub fn validate_in(
    program: &Program,
    current_dir: Option<&std::path::Path>,
    entry: Option<&std::path::Path>,
) -> Vec<Diagnostic> {
    let mut findings = Vec::new();
    let mut declared: HashMap<(DeclKind, String), Span> = HashMap::new();
    for statement in &program.statements {
        check_statement(statement, &mut declared, &mut findings);
    }

    // One lexical walk, shared. Both name rules ask `semantic::scopes` the same
    // question and the walk is the expensive part of this phase.
    let mut scopes = scopes::analyze(program);
    if let Some(dir) = current_dir {
        let imported = crate::semantic::imports::resolve(&scopes.import_specs, Some(dir), entry);
        scopes.resolve(&imported);
    }
    check_inheritance(program, &scopes, &mut findings);
    check_names(&scopes, &mut findings);
    check_declaration_order(&scopes, &mut findings);

    findings.sort_by_key(|d| (d.span.line, d.span.column));
    findings
}

/// The four top-level declaration forms, kept apart so the duplicate rule is
/// per-kind.
///
/// Per-kind, not per-name, because a `class` and an `interface` of the same name
/// live in two different registries at run time and both resolve. Whether *that*
/// should be an error is a language question nobody has answered, and it is
/// registered as **DEC-M4-008** rather than settled here. Measured first: 0
/// cross-kind collisions across 1,070 corpus and ecosystem files, so nothing
/// depends on the answer today in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DeclKind {
    Class,
    Interface,
    Enum,
    Function,
}

impl DeclKind {
    fn noun(self) -> &'static str {
        match self {
            DeclKind::Class => "class",
            DeclKind::Interface => "interface",
            DeclKind::Enum => "enum",
            DeclKind::Function => "function",
        }
    }
}

/// Walks the top level, looking through `export`.
///
/// Top level only, matching what the parser rejected: the guard fired wherever a
/// `class`, `interface` or `enum` was parsed, and every one of those the parser
/// could reach was reachable from here. Nesting is deliberately not extended to
/// — that would change *which* programs are rejected, and DEC-M4-001 moved the
/// rule without changing its reach.
fn check_statement(
    statement: &Statement,
    declared: &mut HashMap<(DeclKind, String), Span>,
    findings: &mut Vec<Diagnostic>,
) {
    match statement {
        Statement::Export(inner) => check_statement(inner, declared, findings),
        Statement::ClassDeclaration(c) => {
            reject_if_reserved(&c.name, "a class name", c.span, findings);
            reject_if_duplicate(DeclKind::Class, &c.name, c.span, declared, findings);
        }
        Statement::InterfaceDeclaration(i) => {
            reject_if_reserved(&i.name, "an interface name", i.span, findings);
            reject_if_duplicate(DeclKind::Interface, &i.name, i.span, declared, findings);
        }
        Statement::EnumDeclaration(e) => {
            reject_if_reserved(&e.name, "an enum name", e.span, findings);
            reject_if_duplicate(DeclKind::Enum, &e.name, e.span, declared, findings);
        }
        Statement::FunctionDeclaration(f) => {
            reject_if_duplicate(DeclKind::Function, &f.name, f.span, declared, findings);
        }
        _ => {}
    }
}

/// A name declared twice in the same scope, by the same kind of declaration.
///
/// **§5.39.** `class A { … } class A { … }` was accepted with the second
/// silently replacing the first, and the same held for `fn`. Nothing reported
/// the collision at any phase, so a file that grew past the point where a reader
/// can see both definitions quietly got the second one.
///
/// Decided: a duplicate in the same scope is a fatal semantic error. What the
/// rule does **not** do is equally part of the contract:
///
///   * **Shadowing across scopes is untouched.** This walks the top level only —
///     the same reach the reserved-name rule has — so a class declared inside a
///     function body and another at the top level do not collide. Most of the
///     corpus fixtures that declare classes do so inside `test(…)` lambdas and
///     depend on that.
///   * **Cross-kind is not covered** — DEC-M4-008, above.
///   * **Cross-file is not covered.** A module whose `import` redeclares a name
///     the importer holds already warns at run time, and `spec/modules.md`
///     records that as its own hazard. Two files are not one scope.
///   * **No overload rule is invented.** Serez has no overloading; two `fn`s of
///     one name are a collision, not a signature set, and nothing here looks at
///     parameters.
///
/// The *second* declaration is reported and the message names the first one's
/// line, because the second is the edit that broke the program.
fn reject_if_duplicate(
    kind: DeclKind,
    name: &str,
    span: Span,
    declared: &mut HashMap<(DeclKind, String), Span>,
    findings: &mut Vec<Diagnostic>,
) {
    let key = (kind, name.to_string());
    match declared.get(&key) {
        Some(first) => findings.push(Diagnostic::frontend(
            SZ_SEMANTIC_ERROR,
            Phase::Semantic,
            span,
            format!(
                "{} '{}' is already declared in this scope, at line {}",
                kind.noun(),
                name,
                first.line
            ),
        )),
        None => {
            declared.insert(key, span);
        }
    }
}

/// A class whose declared parent cannot be resolved.
///
/// **§5.39.** `class Child : Missing { … }` ran to completion as long as nobody
/// constructed it: the parent was looked up in `class_registry` when an instance
/// was built, so `--check` could not tell you that you inherit from something
/// that does not exist. Decided: inheritance resolves in the semantic phase, and
/// an unresolvable parent is fatal.
///
/// # "Not declared in this file" is not "does not exist"
///
/// That distinction is the whole difficulty, and it is why this defers to
/// [`crate::semantic::scopes`] instead of scanning the top level itself. That
/// module walks the complete lexical structure of a program and reports which
/// uses no scope accounts for, under a bias stated in its own header: every
/// ambiguity resolves toward *treat it as bound*. Two of its rules matter here.
///
///   * **A file containing `import` is not conclusive.** Its names may
///     legitimately come from another module, and this phase sees one file at a
///     time. `ScopeReport::is_conclusive` says so, and nothing is reported for
///     such a file. Measured: **369** of the ecosystem's `class X : Y` sites are
///     in files that import, and every one of them stays silent.
///   * **Position does not matter at the top level.** A parent declared further
///     down the file resolves, matching the runtime, where a class registers into
///     one global registry and a forward reference works once the declaration has
///     run.
///
/// # Top-level classes only, and that is not laziness
///
/// The rule applies to classes declared at the top level, the same reach the
/// reserved-name rule has. It was written without that restriction first, and
/// `tests/unit_inheritance_errors.sz` rejected it:
///
/// ```serez
/// test("indirect inheritance cycle is rejected", () => {
///     class InheritanceCycleA : InheritanceCycleB {}     // line 15
///     ...
///     class InheritanceCycleB { public int read() { return 7; } }   // line 21
///     assert(new InheritanceCycleA().read() == 7, "forward reference can recover");
/// });
/// ```
///
/// Inside a body, `scopes` models position exactly — a name used before its own
/// declaration is not bound — because that is what a *variable* does. A class is
/// not a variable: it registers globally when its declaration executes, so the
/// forward reference on line 15 resolves by the time anything constructs it, and
/// the fixture asserts precisely that. Position-independence is a property this
/// module can only rely on at the top level, so that is where the rule stops.
///
/// So this catches what §5.39 measured — a file declaring an impossible parent
/// and importing nothing — and deliberately does not catch a parent a real
/// `import` fails to supply. Proving *that* means resolving and parsing modules
/// during the semantic phase, which reads the filesystem at check time and would
/// have to answer what happens under `--eval` lockdown. That is **DEC-M4-007**,
/// registered rather than taken.
///
/// # What this is not
///
/// Not DEC-M4-002. `scopes` reports free `Read`, `Write`, `Call` and `Type` uses
/// as well, and none of them is reported here — whether an unresolved *variable*
/// is a diagnostic is still open. This reports exactly one kind,
/// [`UseKind::Parent`], at one reach, which is what the owner decided.
fn check_inheritance(
    program: &Program,
    report: &scopes::ScopeReport,
    findings: &mut Vec<Diagnostic>,
) {
    let mut inherits: Vec<(&str, Span)> = Vec::new();
    for statement in &program.statements {
        collect_top_level_parent(statement, &mut inherits);
    }
    if inherits.is_empty() || !report.is_conclusive() {
        return;
    }

    for (parent, span) in inherits {
        // Matched by span as well as name: `scopes` records a parent use at the
        // *class declaration's* span, so this pairs each top-level declaration
        // with its own finding and ignores the same name used inside a body.
        let unresolved = report.free.iter().any(|use_site| {
            use_site.kind == UseKind::Parent && use_site.name == parent && use_site.span == span
        });
        if !unresolved {
            continue;
        }
        findings.push(Diagnostic::frontend(
            SZ_SEMANTIC_ERROR,
            Phase::Semantic,
            span,
            format!(
                "parent class '{parent}' is not declared, and this file imports \
                 nothing that could declare it"
            ),
        ));
    }
}

/// A name no enclosing lexical scope accounts for.
///
/// **DEC-M4-002, decided: a name is valid only if it resolves lexically.**
///
/// Serez used to answer `name -> declaration` once, at run time.
/// `ScopeStack::lookup` walks a frame stack that a *call* pushes onto, so a
/// function body reading a name it does not declare picked up whatever the
/// **caller** happened to hold — and the same function was valid or invalid
/// depending on who called it. `MATURITY_AUDIT.md` carried that as its one
/// **critical** entry. There is no dynamic resolution any more: a name must come
/// from a parameter, its own scope, an enclosing lexical scope, or the file's
/// top level.
///
/// # What this reports, and what it leaves to others
///
/// Every free use except [`UseKind::Parent`]. A class's declared parent is
/// [`check_inheritance`]'s rule: it has its own message, and — more importantly —
/// its own *reach*. Position-independence is something this phase can only rely
/// on at the top level, and a class declared inside a body may legitimately name
/// a parent declared further down the same body. Reporting `Parent` here as well
/// would both duplicate the finding and reintroduce that false positive.
///
/// # Where it does not look
///
/// A file containing any `import` is not judged at all — `ScopeReport::is_conclusive`.
/// A name may legitimately come from another module, and this phase sees one file
/// at a time. That is the same rule `check_inheritance` follows and the same one
/// DEC-M4-007 is about, and it is what makes the rule safe for the ecosystem:
/// `serez-ui` calls across files inside its package without importing, and every
/// one of those files is reached through an entry point that imports.
///
/// # The one direction this is deliberately wrong in
///
/// `semantic::scopes` resolves every ambiguity toward "bound", so this
/// under-reports rather than over-reports. For a fatal rule that is the only
/// acceptable asymmetry: a missed name is caught by the runtime exactly as
/// before, an invented one rejects a program that works.
fn check_names(report: &scopes::ScopeReport, findings: &mut Vec<Diagnostic>) {
    if !report.is_conclusive() {
        return;
    }
    for use_site in &report.free {
        if use_site.kind == UseKind::Parent {
            continue;
        }
        let name = &use_site.name;
        let message = match use_site.kind {
            UseKind::Write => format!(
                "cannot assign to '{name}': it is not declared in this scope or any \
                 enclosing one"
            ),
            UseKind::Type => format!(
                "class or interface '{name}' is not declared in this scope or any \
                 enclosing one"
            ),
            // `Read` and `Call` read the same to a user, and splitting the
            // wording would imply a distinction the rule does not make.
            _ => format!("'{name}' is not declared in this scope or any enclosing one"),
        };
        findings.push(Diagnostic::frontend(
            SZ_SEMANTIC_ERROR,
            Phase::Semantic,
            use_site.span,
            message,
        ));
    }
}

/// A top-level name used, at top level, before its declaration has run.
///
/// # Why this is a separate rule from `check_names`
///
/// `check_names` asks whether a name is declared *anywhere*. This asks whether
/// it is declared *yet*, and the two disagree on exactly the programs the
/// runtime rejects:
///
/// ```text
/// $ sz h.sz          # out x; let x = 1;
/// ❌ ERROR [SZ4001]: Variable not found: x
/// ```
///
/// The phase said nothing about that, because `seed_globals` binds every
/// top-level declaration into frame 0 before the walk. Inside a block it
/// reported the same thing correctly — `{ out z; let z = 1; }` is `SZ8000` — so
/// the phase's answer depended on nesting rather than on the language.
///
/// # Not new hoisting rules
///
/// Nothing about what Serez hoists changes; this reports what it already does.
/// `semantic::scopes::Walker::note_order` carries the measurements, including
/// the five forward references that legitimately work and are not reported.
///
/// # Why it is not gated on `is_conclusive`
///
/// An unread module contributes its names *when the import runs*, so it cannot
/// turn a use that precedes every declaration in the file into one that follows
/// a declaration. The rule is about order within this file, and the order is
/// visible whether or not the modules were read.
fn check_declaration_order(report: &scopes::ScopeReport, findings: &mut Vec<Diagnostic>) {
    for use_site in &report.used_before_declared {
        let name = &use_site.name;
        let message = match use_site.kind {
            UseKind::Write => format!(
                "cannot assign to '{name}' here: its declaration has not run yet. \
                 Serez binds a name when its statement executes, so move the \
                 assignment after the declaration."
            ),
            _ => format!(
                "'{name}' is used before its declaration has run. Serez binds a name \
                 when its statement executes, so move the use after the declaration \
                 or the declaration before the use."
            ),
        };
        findings.push(Diagnostic::frontend(
            SZ_SEMANTIC_ERROR,
            Phase::Semantic,
            use_site.span,
            message,
        ));
    }
}

/// Each top-level `class X : Y`, as `(Y, the declaration's span)`.
///
/// Looks through `export`, the way every other rule here does, and does not
/// descend: see the reach note on [`check_inheritance`].
fn collect_top_level_parent<'a>(statement: &'a Statement, out: &mut Vec<(&'a str, Span)>) {
    match statement {
        Statement::Export(inner) => collect_top_level_parent(inner, out),
        Statement::ClassDeclaration(c) => {
            if let Some(parent) = &c.parent {
                out.push((parent.as_str(), c.span));
            }
        }
        _ => {}
    }
}

fn reject_if_reserved(name: &str, what: &str, span: Span, findings: &mut Vec<Diagnostic>) {
    if reserved(name) {
        findings.push(Diagnostic::frontend(
            SZ_SEMANTIC_ERROR,
            Phase::Semantic,
            span,
            format!("'{name}' is a reserved system namespace and cannot be used as {what}"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn program(source: &str) -> Program {
        let mut parser = Parser::new(Lexer::new(source.to_string()));
        parser.set_source(source.lines().map(str::to_string).collect());
        let program = parser.parse_program();
        assert!(!parser.has_errors(), "fixture must parse cleanly");
        program
    }

    #[test]
    fn an_ordinary_program_reports_nothing() {
        assert!(validate(&program("let x = 1;\nout x;\n")).is_empty());
        assert!(
            validate(&program(
                "public class C {\n    public C() { this.v = 1; }\n}\nout new C().v;\n"
            ))
            .is_empty()
        );
    }

    #[test]
    fn a_reserved_namespace_is_rejected_as_a_class_interface_or_enum() {
        for source in [
            "class Task {}\n",
            "interface Task {}\n",
            "enum Task { A, B }\n",
            "export class Gui {}\n",
        ] {
            let findings = validate(&program(source));
            assert_eq!(findings.len(), 1, "one finding for {source:?}");
            assert_eq!(findings[0].code, SZ_SEMANTIC_ERROR);
            assert_eq!(findings[0].phase, Phase::Semantic);
            assert!(
                findings[0].span.line > 0,
                "a semantic finding points at the declaration"
            );
        }
    }

    #[test]
    fn the_list_is_the_seven_it_was_and_not_one_more() {
        // DEC-M4-003 is what may change this, and it is still open. Pinned so
        // the list cannot drift while that decision is unanswered.
        for name in ["Task", "Time", "DateTime", "System", "Gui", "Dec", "Media"] {
            assert!(reserved(name), "{name} is guarded");
        }
        // §5.31: a program may declare `class Math` and still call
        // `Math.floor(3.7)`. That is the hazard DEC-M4-003 is about, and moving
        // the rule did not fix it.
        for name in ["Math", "File", "Socket", "Crypto", "JSON", "OS", "Tensor"] {
            assert!(!reserved(name), "{name} is not guarded today");
        }
    }

    // ── §5.39: a duplicate declaration ────────────────────────────────────

    #[test]
    fn a_class_or_function_declared_twice_in_one_scope_is_rejected() {
        for (source, noun) in [
            (
                "class A { public A() { this.x = 1; } }\nclass A { public A() { this.x = 2; } }\n",
                "class",
            ),
            (
                "fn int f() { return 1; }\nfn int f() { return 2; }\n",
                "function",
            ),
            (
                "interface I { a: int; }\ninterface I { b: int; }\n",
                "interface",
            ),
            ("enum E { A }\nenum E { B }\n", "enum"),
        ] {
            let findings = validate(&program(source));
            assert_eq!(findings.len(), 1, "one finding for {source:?}");
            assert_eq!(findings[0].code, SZ_SEMANTIC_ERROR);
            assert_eq!(findings[0].phase, Phase::Semantic);
            assert!(
                findings[0].message.contains(noun),
                "the message should name the kind: {}",
                findings[0].message
            );
            // The *second* declaration is reported — it is the edit that broke
            // the program — and the message points back at the first.
            assert_eq!(findings[0].span.line, 2, "{source:?}");
            assert!(
                findings[0].message.contains("at line 1"),
                "the message should name the first declaration: {}",
                findings[0].message
            );
        }
    }

    #[test]
    fn a_third_declaration_is_reported_too() {
        // Reported once per redeclaration, not once per name: someone fixing
        // this needs to see every collision, not the first.
        let findings = validate(&program(
            "fn int f() { return 1; }\nfn int f() { return 2; }\nfn int f() { return 3; }\n",
        ));
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].span.line, 2);
        assert_eq!(findings[1].span.line, 3);
    }

    #[test]
    fn an_export_does_not_hide_a_duplicate() {
        // `export` wraps the declaration, and the reserved-name rule already
        // looks through it. So must this one, or `export class A` next to
        // `class A` would slip past.
        let findings = validate(&program(
            "export class A { public A() {} }\nclass A { public A() {} }\n",
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].span.line, 2);
    }

    #[test]
    fn shadowing_between_different_scopes_is_still_legal() {
        // The rule is "same scope", and it walks the top level only. A class
        // declared inside a function body does not collide with one outside it,
        // and most of the corpus depends on that: the unit fixtures declare
        // their classes inside `test(…)` lambdas.
        let findings = validate(&program(
            "class A { public A() {} }\n\
             fn void scope() { class A { public A() {} } }\n",
        ));
        assert!(
            findings.is_empty(),
            "shadowing across scopes was rejected: {:?}",
            findings.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn two_kinds_of_declaration_sharing_a_name_are_not_reported() {
        // DEC-M4-008. A `class` and an `interface` of one name are two
        // registries at run time and both resolve; whether that should be an
        // error is undecided, and 0 of 1,070 corpus and ecosystem files do it.
        // Pinned so the answer is given rather than drifted into.
        let findings = validate(&program(
            "class Shape { public Shape() {} }\ninterface Shape { sides: int; }\n",
        ));
        assert!(
            findings.is_empty(),
            "cross-kind collision is DEC-M4-008 and must stay unreported until \
             it is decided: {:?}",
            findings.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn declaring_each_name_once_reports_nothing() {
        // The positive control. Without it, a rule that reported every
        // declaration would pass every test above.
        assert!(
            validate(&program(
                "class A { public A() {} }\n\
                 class B { public B() {} }\n\
                 fn int f() { return 1; }\n\
                 fn int g() { return 2; }\n\
                 interface I { a: int; }\n\
                 enum E { X }\n",
            ))
            .is_empty()
        );
    }

    // ── DEC-M4-002: a name that does not resolve lexically ────────────────

    #[test]
    fn a_name_that_resolves_nowhere_is_rejected() {
        for (source, fragment) in [
            ("out nope;\n", "'nope'"),
            ("fn void f() { out ghost; }\nf();\n", "'ghost'"),
            ("ghost = 1;\n", "cannot assign to 'ghost'"),
            ("out new Phantom();\n", "class or interface 'Phantom'"),
        ] {
            let findings = validate(&program(source));
            assert_eq!(findings.len(), 1, "one finding for {source:?}");
            assert_eq!(findings[0].code, SZ_SEMANTIC_ERROR);
            assert_eq!(findings[0].phase, Phase::Semantic);
            assert!(
                findings[0].message.contains(fragment),
                "expected {fragment:?} in {:?}",
                findings[0].message
            );
        }
    }

    #[test]
    fn every_way_a_name_can_be_declared_still_resolves() {
        // The positive control, and it is the whole rule read backwards: a name
        // is valid if it comes from a parameter, its own scope, an enclosing
        // lexical scope, or the top level. One case each, plus the builtins.
        for source in [
            // a parameter
            "fn int f(int n) { return n; }\nout f(1);\n",
            // its own scope
            "fn int f() { let x = 1; return x; }\nout f();\n",
            // an enclosing lexical scope, through a closure
            "fn int f() { let x = 1; let g = () => x; return g(); }\nout f();\n",
            // the top level, used before it is written
            "fn int a() { return b(); }\nfn int b() { return 1; }\nout a();\n",
            // a class field through `this`
            "class C { public C() { this.v = 1; } public int read() { return this.v; } }\n\
             out new C().read();\n",
            // a loop binding, a catch binding and a for-in binding
            "for (let i = 0; i < 2; i = i + 1) { out i; }\n",
            "try { out 1; } catch (e) { out e; }\n",
            "let xs = [1];\nfor (let x in xs) { out x; }\n",
            // builtins and runtime namespaces
            "out parseInt(\"1\");\nout Math.floor(1.5);\n",
        ] {
            let findings = validate(&program(source));
            assert!(
                findings.is_empty(),
                "a legitimate program was rejected: {source:?} -> {:?}",
                findings.iter().map(|d| &d.message).collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn dynamic_resolution_through_the_callers_scope_is_gone() {
        // The behaviour DEC-M4-002 removes, and the reason it was critical: this
        // program printed 42 because `leaky` picked up `secret` from whoever
        // called it, so the same function was valid or invalid depending on the
        // caller.
        let findings = validate(&program(
            "fn int leaky() { return secret; }\n\
             fn int caller() { let secret = 42; return leaky(); }\n\
             out caller();\n",
        ));
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].message.contains("'secret'"),
            "{:?}",
            findings[0]
        );
    }

    #[test]
    fn a_file_that_imports_is_not_judged_on_names() {
        // The rule that keeps the ecosystem working. `serez-ui` calls across
        // files inside its package without importing, and every such file is
        // reached through an entry point that does import — so a file with any
        // `import` is not analysed at all.
        assert!(
            validate(&program(
                "import \"serez-ui\";\nout somethingFromTheModule();\n"
            ))
            .is_empty()
        );
    }

    #[test]
    fn a_mutually_recursive_pair_of_nested_functions_is_accepted() {
        // The false positive that had to be fixed in `semantic::scopes` before
        // this rule could be fatal. `tests/unit_functions_adv.sz` runs it.
        assert!(
            validate(&program(
                "fn void outer() {\n\
                 \x20   fn bool isEven(int n) { if (n == 0) { return true; } return isOdd(n - 1); }\n\
                 \x20   fn bool isOdd(int n) { if (n == 0) { return false; } return isEven(n - 1); }\n\
                 \x20   out isEven(4);\n\
                 }\n\
                 outer();\n",
            ))
            .is_empty()
        );
    }

    #[test]
    fn an_unresolvable_parent_is_reported_once_and_by_the_inheritance_rule() {
        // Both rules read the same `free` list, so a parent could be reported
        // twice. It is not: `check_names` skips `UseKind::Parent`, and the
        // message is the inheritance rule's.
        let findings = validate(&program("class Child : Missing { public Child() {} }\n"));
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(
            findings[0].message.contains("parent class 'Missing'"),
            "{:?}",
            findings[0].message
        );
    }

    // ── §5.39: an unresolvable parent class ───────────────────────────────

    #[test]
    fn a_parent_that_does_not_exist_is_rejected_without_being_instantiated() {
        // The finding itself: this program used to run to completion and exit 0,
        // because the parent was only looked up when an instance was built.
        let findings = validate(&program(
            "class Child : Missing { public Child() {} }\nout 1;\n",
        ));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, SZ_SEMANTIC_ERROR);
        assert!(
            findings[0].message.contains("Missing"),
            "{}",
            findings[0].message
        );
        assert_eq!(findings[0].span.line, 1);
    }

    #[test]
    fn a_local_parent_resolves_in_either_order() {
        // Position-independent, matching the runtime: a forward reference to a
        // class declared further down the file works.
        for source in [
            "class Base { public Base() {} }\nclass Child : Base { public Child() { super(); } }\n",
            "class Child : Base { public Child() { super(); } }\nclass Base { public Base() {} }\n",
        ] {
            assert!(
                validate(&program(source)).is_empty(),
                "a local parent was rejected: {source:?}"
            );
        }
    }

    #[test]
    fn a_file_that_imports_reports_no_missing_parent() {
        // "Not declared in this file" is not "does not exist". A file with an
        // `import` cannot be judged one file at a time, so nothing is reported —
        // 369 of the ecosystem's `class X : Y` sites are exactly this shape.
        assert!(
            validate(&program(
                "import \"serez-ui\";\nclass App : Window { public App() { super(); } }\n",
            ))
            .is_empty()
        );
    }

    #[test]
    fn an_import_that_does_not_supply_the_parent_is_still_not_reported() {
        // DEC-M4-007, pinned. Proving that a real import fails to supply a name
        // means resolving and parsing modules during the semantic phase — which
        // reads the filesystem at check time and has to answer what happens
        // under `--eval` lockdown. Registered, not taken.
        //
        // If this ever starts failing, the decision was answered: delete the pin
        // and say so, do not weaken the rule to make it pass again.
        assert!(
            validate(&program(
                "import \"./nowhere\";\nclass Child : NeverDeclared { public Child() {} }\n",
            ))
            .is_empty()
        );
    }

    #[test]
    fn a_parent_declared_inside_an_enclosing_scope_resolves() {
        // Nesting is modelled rather than flattened: `Base` is visible where
        // `Child` is written, so this is not a missing parent.
        assert!(
            validate(&program(
                "fn void build() {\n\
                 \x20   class Base { public Base() {} }\n\
                 \x20   class Child : Base { public Child() { super(); } }\n\
                 }\n",
            ))
            .is_empty()
        );
    }

    #[test]
    fn a_forward_parent_reference_inside_a_body_is_not_reported() {
        // The false positive the first version of this rule produced, and the
        // reason it applies to top-level classes only.
        //
        // `tests/unit_inheritance_errors.sz` caught it: inside a `test(…)`
        // lambda, `class InheritanceCycleA : InheritanceCycleB {}` is written
        // *before* `class InheritanceCycleB` a few lines down, and the fixture
        // asserts "forward reference can recover" — because a class registers
        // into one global registry when its declaration executes, so by the time
        // anything constructs the child, the parent is there.
        //
        // `semantic::scopes` models position exactly inside a body, which is
        // right for a variable and wrong for a class. Rather than teach it a
        // second rule, the inheritance check stops at the top level, where
        // position-independence is something it can actually rely on.
        assert!(
            validate(&program(
                "fn void build() {\n\
                 \x20   class Child : Base { public Child() { super(); } }\n\
                 \x20   class Base { public Base() {} }\n\
                 }\n",
            ))
            .is_empty(),
            "a forward parent reference inside a body was rejected; \
             unit_inheritance_errors.sz depends on it working"
        );
    }

    #[test]
    fn a_builtin_class_is_a_valid_parent() {
        // `Error`, `Set` and `Tensor` are constructed by the runtime without a
        // user declaration. Rejecting them would break working programs.
        assert!(
            validate(&program(
                "class AppError : Error { public AppError() { super(); } }\n",
            ))
            .is_empty()
        );
    }

    #[test]
    fn a_class_with_no_parent_is_never_reported() {
        // The positive control for the inheritance rule.
        assert!(validate(&program("class Plain { public Plain() {} }\n")).is_empty());
    }

    #[test]
    fn a_reserved_name_is_only_rejected_where_it_declares_a_type() {
        // A *variable* named Task was always allowed, and still is. The rule
        // moved phase; its reach did not change.
        assert!(validate(&program("let Task = 1;\nout Task;\n")).is_empty());
    }

    #[test]
    fn it_runs_on_a_tree_rather_than_on_text() {
        // The phase's whole advantage over the parser is that it sees complete
        // nodes. Pinned by construction: `validate` takes a `Program`, so a
        // future rule cannot quietly go back to scanning source.
        let p = program("enum Colour { Red, Green }\n");
        assert!(validate(&p).is_empty());
        assert_eq!(p.statements.len(), 1, "the tree is what was handed over");
    }
}
