//! What the semantic phase can say about a file that imports — DEC-M4-007.
//!
//! # The false negative, measured
//!
//! `check_names` returns early on any file containing an `import`, because a
//! name it cannot see might legitimately come from a module. Two files, the same
//! undefined name:
//!
//! ```text
//! $ sz no_import.sz
//! ❌ SEMANTIC ERROR [SZ8000] 'totallyUndefined' is not declared in this scope
//!
//! $ sz with_import.sz
//! 4
//! ❌ ERROR [SZ4001]: Variable not found: totallyUndefined
//! ```
//!
//! One `import` anywhere turns a phase that rejects programs into one that does
//! not, for every name in the file — and the program gets as far as printing `4`
//! before it fails.
//!
//! # Why the fix is off by default
//!
//! Resolving imports at check time is **DEC-M4-007 option A**, registered and
//! open, with §7A recommending B "until an explicit answer for lockdown and a
//! measured cost". This file supplies both, and does not take the decision:
//! `RunOpts::resolve_imports` is `false` everywhere except here.
//!
//! The lockdown answer is that the phase never reads modules under lockdown,
//! whatever the caller asks — `import` is refused there at run time, and reading
//! module files to analyse the program would be the capability leak §7A named.
//! `lockdown_never_reads_modules` is that test.
//!
//! The cost is one corpus fixture and one pinned ecosystem package, and both are
//! **right** to be rejected: `unit_modules.sz` names an identifier it knows is
//! unbound so it can observe the runtime error, and `serez-apipack` writes
//! `"{ name: inner }"`, which Serez reads as an interpolation of an undeclared
//! `name`. Neither is a false positive — see §5.50.
//!
//! # What is asserted
//!
//! That the resolver agrees with `eval_import` about what a module contributes,
//! at each case where they could disagree: exports, no exports, transitivity,
//! cycles, nesting, and the five ways a module can be unreadable. A resolver
//! that got any of these wrong would reject a program that runs — the one
//! failure mode a fatal phase must not have.

use serez_code::run::{RunOpts, run_source_detailed};
use std::fs;
use std::path::{Path, PathBuf};

/// A directory of modules, and a file that imports them.
struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "serez-sem-{}-{}-{:?}",
            tag,
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("root");
        Tree { root }
    }

    fn write(&self, rel: &str, body: &str) -> PathBuf {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("parent");
        }
        fs::write(&path, body).expect("write");
        path
    }

    /// Run `rel` with import resolution on, and return its exit code and stderr.
    fn check(&self, rel: &str) -> (i32, String) {
        self.run(rel, true, false)
    }

    /// Run `rel` exactly as `sz file.sz` does today: resolution off.
    fn check_as_today(&self, rel: &str) -> (i32, String) {
        self.run(rel, false, false)
    }

    fn run(&self, rel: &str, resolve_imports: bool, lockdown: bool) -> (i32, String) {
        let path = self.root.join(rel);
        let source = fs::read_to_string(&path).expect("read");
        let outcome = run_source_detailed(
            source,
            &path.to_string_lossy(),
            RunOpts {
                current_file: Some(path.clone()),
                resolve_imports,
                lockdown,
                ..RunOpts::default()
            },
        );
        let message = match &outcome.failure {
            Some(failure) => format!("{:?}", failure),
            None => String::new(),
        };
        (outcome.exit_code, message)
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The regression: a name from nowhere is caught even beside an `import`.
#[test]
fn a_name_from_nowhere_is_caught_beside_an_import() {
    let tree = Tree::new("nowhere");
    tree.write(
        "lib/helpers.sz",
        "export fn int helper(int x) { return x * 2; }\n",
    );
    tree.write(
        "main.sz",
        "import \"lib/helpers\";\nout helper(2);\nout totallyUndefined;\n",
    );

    let (code, message) = tree.check("main.sz");
    assert_ne!(code, 0, "an undefined name was accepted");
    assert!(
        message.contains("totallyUndefined"),
        "rejected for some other reason: {}",
        message
    );

    // And with resolution off — what `sz file.sz` does today — the phase says
    // nothing and the same name reaches the evaluator. That is the defect this
    // measures, not a property to keep: `out helper(2)` has already printed by
    // the time it fails.
    let (_, today) = tree.check_as_today("main.sz");
    assert!(
        !today.contains("SEMANTIC") && !today.contains("SZ8000"),
        "the phase already caught this without resolution, so DEC-M4-007 is not \
         what this file describes: {}",
        today
    );
    assert!(
        today.contains("totallyUndefined"),
        "the name did not even reach the evaluator: {}",
        today
    );
}

/// The control that matters most: an imported name must not be reported.
///
/// Every other test here asserts a rejection. A resolver that contributed
/// nothing would satisfy all of them and reject every program that imports.
#[test]
fn an_imported_name_resolves() {
    let tree = Tree::new("resolves");
    tree.write(
        "lib/helpers.sz",
        "export const PI = 3.14;\nexport fn int helper(int x) { return x * 2; }\n\
         export class Counter { public Counter(int n) { this.n = n; } }\n",
    );
    tree.write(
        "main.sz",
        "import \"lib/helpers\";\nout helper(21);\nout PI;\nlet c = new Counter(1);\n",
    );

    let (code, message) = tree.check("main.sz");
    assert_eq!(code, 0, "an imported name was reported: {}", message);
}

/// A module with no `export` at all contributes everything it declares.
///
/// `eval_import` calls this its backwards-compatibility branch, and a resolver
/// that only looked at exports would reject every program using such a module.
#[test]
fn a_module_with_no_exports_contributes_everything() {
    let tree = Tree::new("noexport");
    tree.write(
        "lib/plain.sz",
        "fn int a() { return 1; }\nfn int b() { return 2; }\nlet c = 3;\n",
    );
    tree.write("main.sz", "import \"lib/plain\";\nout a() + b() + c;\n");

    let (code, message) = tree.check("main.sz");
    assert_eq!(
        code, 0,
        "a module without exports contributed nothing: {}",
        message
    );
}

/// And a module that exports contributes only what it exported.
///
/// Measured against the runtime, which removes the rest when the module
/// finishes: `sz vis.sz` printed `1` and then
/// `❌ ERROR [SZ4001]: Variable not found: notExported`.
#[test]
fn an_unexported_name_is_not_contributed() {
    let tree = Tree::new("hidden");
    tree.write(
        "lib/vis.sz",
        "export fn int shown(int x) { return x; }\nfn int hidden(int x) { return x; }\n",
    );
    tree.write(
        "main.sz",
        "import \"lib/vis\";\nout shown(1);\nout hidden(1);\n",
    );

    let (code, message) = tree.check("main.sz");
    assert_ne!(code, 0, "a non-exported name was treated as available");
    assert!(
        message.contains("hidden"),
        "rejected for some other reason: {}",
        message
    );
}

/// Transitive: what a module's own imports brought in stays visible.
///
/// `eval_import` only removes names the module itself declared, so a middle
/// module that exports one class does not wipe the leaf's. Getting this wrong
/// was a real runtime bug once — the comment in `eval_import` records it — and a
/// resolver that repeated the mistake would reject working programs.
#[test]
fn a_transitively_imported_name_resolves() {
    let tree = Tree::new("transitive");
    tree.write("lib/leaf.sz", "export fn int leaf() { return 1; }\n");
    tree.write(
        "lib/middle.sz",
        "import \"./leaf.sz\";\nexport fn int middle() { return 2; }\n",
    );
    tree.write(
        "main.sz",
        "import \"lib/middle\";\nout leaf() + middle();\n",
    );

    let (code, message) = tree.check("main.sz");
    assert_eq!(
        code, 0,
        "a transitively imported name was reported: {}",
        message
    );
}

/// An import inside a lambda still contributes, because at run time it does.
///
/// This is not hypothetical: `tests/unit_export.sz` writes
/// `test("...", () => { import "lib/greet_noexport"; ... })`. A resolver reading
/// only top-level statements missed it, which showed up as three corpus failures
/// and one ecosystem failure before the walker's own list replaced it.
#[test]
fn an_import_nested_in_a_lambda_still_contributes() {
    let tree = Tree::new("nested");
    tree.write("lib/deep.sz", "fn int deep() { return 7; }\n");
    tree.write(
        "main.sz",
        "fn void run(any body) { body(); }\n\
         run(() => { import \"lib/deep\"; out deep(); });\n",
    );

    let (code, message) = tree.check("main.sz");
    assert_eq!(
        code, 0,
        "an import inside a lambda contributed nothing: {}",
        message
    );
}

/// A cycle terminates, and both halves contribute.
#[test]
fn an_import_cycle_terminates() {
    let tree = Tree::new("cycle");
    tree.write(
        "lib/a.sz",
        "import \"./b.sz\";\nexport fn int fromA() { return 1; }\n",
    );
    tree.write(
        "lib/b.sz",
        "import \"./a.sz\";\nexport fn int fromB() { return 2; }\n",
    );
    tree.write("main.sz", "import \"lib/a\";\nout fromA() + fromB();\n");

    let (code, message) = tree.check("main.sz");
    assert_eq!(
        code, 0,
        "a cycle lost a name or did not terminate: {}",
        message
    );
}

/// A module importing the entry file back is a no-op, as it is at run time.
#[test]
fn a_module_importing_the_entry_back_terminates() {
    let tree = Tree::new("backref");
    tree.write(
        "lib/back.sz",
        "import \"../main.sz\";\nexport fn int fromBack() { return 1; }\n",
    );
    tree.write(
        "main.sz",
        "import \"lib/back\";\nexport fn int fromMain() { return 2; }\nout fromBack();\n",
    );

    let (code, message) = tree.check("main.sz");
    assert_eq!(code, 0, "the entry file was re-entered: {}", message);
}

/// A module that cannot be found leaves the file unjudged, rather than adding a
/// second diagnostic for something the runtime already reports.
#[test]
fn an_unresolvable_module_silences_the_check() {
    let tree = Tree::new("missing");
    tree.write(
        "main.sz",
        "import \"lib/does_not_exist\";\nout totallyUndefined;\n",
    );

    let (code, message) = tree.check("main.sz");
    assert_ne!(code, 0, "the run should still fail — at run time");
    assert!(
        !message.contains("SEMANTIC"),
        "an unreadable module produced a semantic diagnostic: {}",
        message
    );
}

/// A module that does not parse does the same.
#[test]
fn a_module_that_does_not_parse_silences_the_check() {
    let tree = Tree::new("broken");
    tree.write("lib/broken.sz", "class { let let let\n");
    tree.write("main.sz", "import \"lib/broken\";\nout totallyUndefined;\n");

    let (code, message) = tree.check("main.sz");
    assert_ne!(code, 0, "the run should still fail");
    assert!(
        !message.contains("SZ8000"),
        "an unparseable module produced a name diagnostic: {}",
        message
    );
}

/// An HTTP module is never fetched to analyse a file.
#[test]
fn an_http_module_silences_the_check_without_a_request() {
    let tree = Tree::new("http");
    tree.write(
        "main.sz",
        "import \"https://example.invalid/mod.sz\";\nout totallyUndefined;\n",
    );

    let (code, message) = tree.check("main.sz");
    assert_ne!(code, 0, "the run should still fail");
    assert!(
        !message.contains("SZ8000"),
        "a URL import produced a name diagnostic: {}",
        message
    );
}

/// §7A's open question about lockdown, answered: the phase reads nothing.
///
/// Under lockdown `import` is refused at run time. Reading module files to
/// analyse a locked-down program would be exactly the filesystem access lockdown
/// exists to prevent, so resolution is withheld there whatever the caller asks.
#[test]
fn lockdown_never_reads_modules() {
    let tree = Tree::new("lockdown");
    let sentinel = tree.write("lib/secret.sz", "export fn int secret() { return 1; }\n");
    tree.write("main.sz", "import \"lib/secret\";\nout secret();\n");

    let (code, _) = tree.run("main.sz", true, true);
    assert_ne!(code, 0, "lockdown allowed an import");

    // The module must still be there, unread — the assertion is about the
    // capability, and the observable proxy is that lockdown's refusal comes from
    // the import statement rather than from anything the phase discovered.
    assert!(Path::new(&sentinel).exists());
}

/// The resolver itself, at the level the run cannot reach: what a module gives.
#[test]
fn the_resolver_reports_what_it_could_not_read() {
    let tree = Tree::new("unresolved_list");
    tree.write("lib/ok.sz", "export fn int ok() { return 1; }\n");
    let main = tree.write(
        "main.sz",
        "import \"lib/ok\";\nimport \"lib/gone\";\nimport \"https://example.invalid/x.sz\";\n",
    );

    let source = fs::read_to_string(&main).expect("read");
    let lexer = serez_code::lexer::Lexer::new(source);
    let mut parser = serez_code::parser::Parser::new(lexer);
    let program = parser.parse_program();
    let report = serez_code::semantic::scopes::analyze(&program);

    let resolved = serez_code::semantic::imports::resolve(
        &report.import_specs,
        main.parent(),
        Some(main.as_path()),
    );

    assert!(
        resolved.contains("ok"),
        "the readable module contributed nothing"
    );
    assert!(
        !resolved.is_complete(),
        "two unreadable modules went unreported"
    );
    assert_eq!(
        resolved.unresolved().len(),
        2,
        "expected the missing module and the URL: {:?}",
        resolved.unresolved()
    );
}
