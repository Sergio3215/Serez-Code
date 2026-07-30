//! The one pipeline every entry point funnels through: lex → parse → type-check → eval.
//!
//! A `.sz` file is just a string that happened to come from disk. Past the lexer
//! nothing downstream can tell the difference, so the path is only good for three
//! things: reading the bytes, labelling error messages, and locating the
//! `serez.json` that grants permissions. [`run_source`] takes those as explicit
//! options instead, which is what lets `sz --eval` run a snippet straight from a
//! string rather than inventing a temp file to satisfy the CLI.

use crate::evaluator;
use crate::lexer;
use crate::parser;
use crate::type_checker;

/// Everything the pipeline needs that used to be implied by "there is a file".
pub struct RunOpts {
    /// Permissions granted before the first statement runs. Normally read from the
    /// `serez.json` next to the entry file; empty for `--eval`.
    pub permissions: Vec<String>,
    /// Base for resolving relative `import`s. `None` means there is no file to be
    /// relative to, so imports have nothing to resolve against.
    pub current_file: Option<std::path::PathBuf>,
    /// Treat the source as untrusted. Closes `use permissions { .. }` (which
    /// otherwise inserts straight into the evaluator's permission set at runtime,
    /// so any program can grant itself everything) plus the three capabilities that
    /// reach the disk with no permission declared at all: `File`, `import`, and
    /// Autodiff's weight files.
    ///
    /// The network is deliberately NOT part of this — see `eval_fetch`. Off for
    /// normal CLI runs: someone running their own file is supposed to be able to
    /// declare permissions inline.
    pub lockdown: bool,
    /// Type-check and report, don't evaluate (`--check`).
    pub check_only: bool,
}

impl Default for RunOpts {
    fn default() -> Self {
        RunOpts { permissions: Vec::new(), current_file: None, lockdown: false, check_only: false }
    }
}

impl RunOpts {
    /// Options for running source you did not write: no permissions, no file to
    /// import from, and the self-granting escapes closed. This is what `--eval` uses.
    pub fn sandboxed() -> Self {
        RunOpts { lockdown: true, ..Default::default() }
    }
}

/// Result of a run. `exit_code` follows the CLI contract: 0 on success, 1 on parse
/// error or uncaught exception.
pub struct Outcome {
    pub exit_code: i32,
}

/// Lex, parse, type-check and (unless `check_only`) evaluate `src`.
///
/// `name` is only ever shown to the user — it labels parse errors, so it should be
/// the file path when there is one and something obviously synthetic (`<eval>`)
/// when there isn't.
pub fn run_source(src: String, name: &str, opts: RunOpts) -> Outcome {
    let source_lines: Vec<String> = src.lines().map(|l| l.to_string()).collect();

    let lexer = lexer::Lexer::new(src);
    let mut parser = parser::Parser::new(lexer);
    parser.set_source(source_lines.clone());
    parser.set_source_name(name);
    let program = parser.parse_program();
    let parse_failed = parser.has_errors();

    let mut checker = type_checker::TypeChecker::new(&program);
    checker.check();

    let mut evaluator = evaluator::Evaluator::new();
    evaluator.set_source(source_lines);
    evaluator.set_permissions(opts.permissions);
    evaluator.set_lockdown(opts.lockdown);
    if let Some(ref path) = opts.current_file {
        evaluator.set_current_file(path);
    }

    let mut run_failed = false;
    if parse_failed {
        // A program with parse errors must not half-run: statements after the
        // broken one would execute against missing definitions.
        eprintln!("❌ Aborted: fix the parse errors above before running.");
    } else if opts.check_only {
        evaluator.check_program(&program);
    } else {
        // eval_program returns None on uncaught exception / runtime / flash-scope error
        if evaluator.eval_program(&program).is_none() {
            run_failed = true;
        }
        if std::env::var("SEREZ_ARENA_STATS").is_ok() {
            let (global, scoped) = evaluator.arena_stats();
            eprintln!("[arena] global={} scoped={}", global, scoped);
        }
    }

    use std::io::Write;
    let _ = std::io::stdout().flush();

    Outcome { exit_code: if parse_failed || run_failed { 1 } else { 0 } }
}

/// Lex/parse/evaluate a `.sz` file. Returns the process exit code: 0 on success,
/// 1 if the file can't be read, fails to parse, or the program ends with an
/// uncaught exception / runtime error.
pub fn run_file(file_path: &str, is_check: bool) -> i32 {
    // .szx files (serez-ui JSX) are translated to .sz first, then run.
    if file_path.ends_with(".szx") {
        return crate::szx::run_szx_file(file_path, is_check);
    }

    let input = match std::fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("❌ ERROR reading file '{}': {}", file_path, e);
            return 1;
        }
    };

    let file_path_obj = std::path::Path::new(file_path);

    // Permissions come from the serez.json sitting next to the file, if any.
    let mut permissions = Vec::new();
    if let Some(dir) = file_path_obj.parent() {
        let dir = if dir == std::path::Path::new("") { std::path::Path::new(".") } else { dir };
        match crate::package_manager::SerezManifest::load(dir) {
            Ok(manifest) => permissions = manifest.permissions,
            Err(e) => {
                // A serez.json that EXISTS but doesn't parse must not fail
                // silently: the program would run with zero permissions and
                // the user would only see a confusing permission error.
                if dir.join("serez.json").exists() {
                    eprintln!(
                        "⚠️  WARNING: serez.json found but not loaded ({}); running WITHOUT permissions.",
                        e
                    );
                }
            }
        }
    }

    run_source(
        input,
        file_path,
        RunOpts {
            permissions,
            current_file: Some(file_path_obj.to_path_buf()),
            lockdown: false,
            check_only: is_check,
        },
    )
    .exit_code
}

/// Run a snippet handed in as a string (`sz --eval`). No file, so no `serez.json`
/// and no permissions; lockdown is on because the source did not necessarily come
/// from whoever is running it.
pub fn run_eval(src: String, is_check: bool) -> i32 {
    run_source(src, "<eval>", RunOpts { check_only: is_check, ..RunOpts::sandboxed() }).exit_code
}
