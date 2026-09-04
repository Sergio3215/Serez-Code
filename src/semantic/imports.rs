//! What an `import` brings into scope, worked out without running anything.
//!
//! # The false negative this removes
//!
//! `semantic::scopes` reported `has_imports`, and `validate::check_names`
//! returned early on it: a file with any `import` had its names left unchecked,
//! because a name it could not see might legitimately come from a module.
//!
//! That is safe against false positives and useless against real ones. Measured,
//! with a module declaring `helper` and a file using a name nothing declares:
//!
//! ```text
//! $ sz no_import.sz
//! ❌ SEMANTIC ERROR [SZ8000] [no_import.sz 1:5]: 'totallyUndefined' is not
//!    declared in this scope or any enclosing one
//!
//! $ sz with_import.sz
//! 4
//! ❌ ERROR [SZ4001]: Variable not found: totallyUndefined
//! ```
//!
//! The same undefined name: rejected before execution in one file, and in the
//! other only after `out helper(2)` had already printed `4`. One `import`
//! anywhere in a file turned a phase that rejects programs into one that does
//! not, for every name in it.
//!
//! # Why this is resolvable rather than a heuristic
//!
//! Serez has no named imports, no aliases and no namespaces: `import "path"`
//! executes the module in the same evaluator, and its top-level declarations
//! land in the importing file's global scope. What a module contributes is
//! therefore a static question, and `eval_import` answers it with a rule this
//! module mirrors exactly:
//!
//!   * a module that uses **no** `export` contributes every top-level name it
//!     declares — `eval_import`'s "backwards compat" branch;
//!   * a module that uses `export` at all contributes only its exported names;
//!   * **either way** it also contributes everything its own imports brought in,
//!     because `eval_import` only removes names the module itself declared.
//!
//! Measured, against a module with one exported and two unexported names:
//!
//! ```text
//! $ sz vis.sz
//! 1
//! ❌ ERROR [SZ4001]: Variable not found: notExported
//! ```
//!
//! # Three answers, not two
//!
//! A name is now **local**, **imported**, or **unresolved**, and only the third
//! silences the check — per file, for a reason, rather than for every file that
//! contains the word `import`. A module is unresolved when it is fetched over
//! HTTP, when `modules::resolve` cannot find it, when it cannot be read, when it
//! does not parse, or when it is a `.szx` — which would mean running the
//! translator as a subprocess during static analysis.
//!
//! In each of those cases this reports *less* confidence, never a diagnostic. A
//! module that does not resolve is already a runtime `ModuleNotFound`, and
//! inventing a second diagnostic for it here would be a language decision rather
//! than a fix.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::ast::{Program, Statement};

/// How many modules one file's import graph may pull in before this gives up.
///
/// Not a language limit: exceeding it makes the file *unresolved*, which is
/// exactly the behaviour every file had before this module existed. It exists so
/// that static analysis of a pathological tree cannot take unbounded time, and
/// it is two orders of magnitude past the largest graph in the ecosystem
/// corpus.
const MAX_MODULES: usize = 1000;

/// Everything an entry file's imports make visible, and whether that is all of
/// it.
#[derive(Debug, Default)]
pub struct ImportedNames {
    names: HashSet<String>,
    /// Module specifiers this could not read. Non-empty means the file's free
    /// names cannot be judged.
    unresolved: Vec<String>,
}

impl ImportedNames {
    pub fn contains(&self, name: &str) -> bool {
        self.names.contains(name)
    }

    /// Whether every `import` in the graph was read.
    pub fn is_complete(&self) -> bool {
        self.unresolved.is_empty()
    }

    /// The specifiers that could not be read, for a caller that wants to say so.
    pub fn unresolved(&self) -> &[String] {
        &self.unresolved
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> {
        self.names.iter()
    }
}

/// Resolve a file's imports, relative to the directory it came from.
///
/// `specs` is `ScopeReport::import_specs` — every module specifier the file
/// imports, **wherever it appears**. Not just at the top level: `import` is an
/// ordinary statement, and the corpus puts one inside a lambda. It still lands
/// in the global scope when it runs, so a top-level-only walk misses the names
/// and reports them as undeclared; that was measured as three corpus failures
/// and one ecosystem failure before this took the walker's list instead.
///
/// `current_dir` is the entry file's parent and `entry` is the file itself,
/// which is what `Evaluator::set_current_file` gives the runtime. The runtime
/// marks the entry as already-loaded before anything runs, so a module importing
/// it back is a no-op; this mirrors that rather than reading it a second time.
pub fn resolve(
    specs: &[String],
    current_dir: Option<&Path>,
    entry: Option<&Path>,
) -> ImportedNames {
    let mut found = ImportedNames::default();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    if let Some(path) = entry.and_then(|p| p.canonicalize().ok()) {
        seen.insert(path);
    }
    for spec in specs {
        import(spec, current_dir, &mut seen, &mut found);
    }
    found
}

fn import(spec: &str, dir: Option<&Path>, seen: &mut HashSet<PathBuf>, found: &mut ImportedNames) {
    // An HTTP module is fetched and cached at runtime. Resolving it here would
    // mean a network request during static analysis, which is not something a
    // compile phase should do.
    if spec.starts_with("http://") || spec.starts_with("https://") {
        found.unresolved.push(spec.to_string());
        return;
    }

    if seen.len() >= MAX_MODULES {
        found.unresolved.push(spec.to_string());
        return;
    }

    let Some(canonical) = crate::modules::resolve(spec, dir) else {
        // Already a runtime `ModuleNotFound`. Saying so twice, in a phase that
        // rejects programs, would be a new rule rather than a fix.
        found.unresolved.push(spec.to_string());
        return;
    };

    // Recorded before the body is read, which is what makes a cycle terminate —
    // the same order `eval_import` uses, and for the same reason.
    if !seen.insert(canonical.clone()) {
        return;
    }

    // `.szx` is JSX and `modules::load_source` translates it by running the
    // translator as a subprocess. Static analysis does not start processes.
    if canonical
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("szx"))
    {
        found.unresolved.push(spec.to_string());
        return;
    }

    let Ok(source) = crate::modules::load_source(&canonical) else {
        found.unresolved.push(spec.to_string());
        return;
    };

    let lexer = crate::lexer::Lexer::new(source);
    let mut parser = crate::parser::Parser::new(lexer);
    let module = parser.parse_program();
    if parser.has_errors() {
        // The import will abort at runtime with `Import aborted: fix the parse
        // errors`. Nothing can be said about what it would have declared.
        found.unresolved.push(spec.to_string());
        return;
    }

    for name in contributed(&module) {
        found.names.insert(name);
    }

    // The module's own imports, found the same way the entry file's were: one
    // scope walk, which sees a nested `import` as readily as a top-level one.
    let nested = crate::semantic::scopes::analyze(&module);
    let parent = canonical.parent();
    for spec in &nested.import_specs {
        import(spec, parent, seen, found);
    }
}

/// The names a module makes visible to whoever imports it.
///
/// Mirrors `eval_import`: exports win when there are any, and every declaration
/// is visible when there are none. Nested imports are handled by the caller
/// continuing the walk, which is also how the runtime keeps them — `eval_import`
/// only removes names the module itself declared.
fn contributed(module: &Program) -> Vec<String> {
    let exported: Vec<String> = module
        .statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::Export(inner) => crate::semantic::scopes::declared_name(inner),
            _ => None,
        })
        .collect();

    if exported.is_empty() {
        crate::semantic::scopes::declared_names(&module.statements)
    } else {
        exported
    }
}
