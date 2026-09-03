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
use crate::render;
use crate::semantic;
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
        RunOpts {
            permissions: Vec::new(),
            current_file: None,
            lockdown: false,
            check_only: false,
        }
    }
}

impl RunOpts {
    /// Options for running source you did not write: no permissions, no file to
    /// import from, and the self-granting escapes closed. This is what `--eval` uses.
    pub fn sandboxed() -> Self {
        RunOpts {
            lockdown: true,
            ..Default::default()
        }
    }
}

/// Result of a run. `exit_code` follows the CLI contract: 0 on success, 1 on parse
/// error or uncaught exception.
pub struct Outcome {
    pub exit_code: i32,
}

/// Machine-readable reason a source pipeline failed.
#[derive(Debug, Clone)]
pub enum RunFailure {
    Frontend(Vec<parser::ParseError>),
    Runtime(evaluator::RuntimeError),
    UncaughtException {
        message: String,
    },
    InvalidControlFlow(evaluator::InvalidControlFlow),
    /// A legacy runtime-failure producer that has not migrated to a
    /// structured runtime diagnostic yet.
    UnstructuredRuntime,
}

/// Detailed pipeline result for embedders, tests and tooling.
///
/// [`Outcome`] and [`run_source`] remain unchanged for source compatibility;
/// callers that need the failure payload can opt into this richer API.
#[derive(Debug, Clone)]
pub struct DetailedOutcome {
    pub exit_code: i32,
    pub failure: Option<RunFailure>,
}

impl DetailedOutcome {
    pub fn is_success(&self) -> bool {
        self.failure.is_none()
    }
}

/// Lex, parse, type-check and (unless `check_only`) evaluate `src`.
///
/// `name` is only ever shown to the user — it labels parse errors, so it should be
/// the file path when there is one and something obviously synthetic (`<eval>`)
/// when there isn't.
pub fn run_source(src: String, name: &str, opts: RunOpts) -> Outcome {
    let detailed = run_source_detailed(src, name, opts);
    Outcome {
        exit_code: detailed.exit_code,
    }
}

/// Detailed form of [`run_source`]. It preserves the same diagnostics and exit
/// codes while also returning a structured failure to machine consumers.
pub fn run_source_detailed(src: String, name: &str, opts: RunOpts) -> DetailedOutcome {
    let source_lines: Vec<String> = src.lines().map(|l| l.to_string()).collect();

    let lexer = lexer::Lexer::new(src);
    let mut parser = parser::Parser::new(lexer);
    parser.set_source(source_lines.clone());
    parser.set_source_name(name);
    let program = parser.parse_program();
    let parse_failed = parser.has_errors();

    // The semantic phase (DEC-M4-001). Rules about *meaning* that reject a
    // program, between the parser and the advisory type checker.
    //
    // Only on a tree the parser accepted: validating a broken tree reports
    // consequences of the syntax error rather than problems of its own. And
    // before the checker, because this is fatal and the checker is not — a
    // program rejected here must not reach a stage whose findings may be ignored.
    //
    // See `semantic::validate` for what it checks and what it deliberately
    // does not.
    let semantic_findings = if parse_failed {
        Vec::new()
    } else {
        semantic::validate::validate(&program)
    };

    // The checker is skipped when the semantic phase rejected the program, which
    // is DEC-M4-001's rule applied literally: a program rejected on meaning must
    // not reach a stage whose findings may be ignored. Reporting advisory type
    // findings about a program that is not going to run is noise, and it would
    // grow into misleading noise as the phase acquires rules.
    //
    // It is *not* skipped when the parser failed. That is pre-existing behaviour
    // and out of this molecule's scope; changing it is a separate question about
    // what the checker should say about a broken tree.
    let mut checker = type_checker::TypeChecker::new(&program);
    if semantic_findings.is_empty() {
        checker.check();
    }

    let render_lines = source_lines.clone();
    let mut evaluator = evaluator::Evaluator::new();
    evaluator.set_source(source_lines);
    evaluator.set_permissions(opts.permissions);
    evaluator.set_lockdown(opts.lockdown);
    if let Some(ref path) = opts.current_file {
        evaluator.set_current_file(path);
    }

    let failure = if parse_failed {
        // A program with parse errors must not half-run: statements after the
        // broken one would execute against missing definitions.
        eprintln!("❌ Aborted: fix the parse errors above before running.");
        Some(RunFailure::Frontend(parser.take_errors()))
    } else if !semantic_findings.is_empty() {
        // Same reason, one phase later: a program whose meaning is rejected must
        // not run. Rendered here rather than at the producer, because
        // `semantic::validate` is a pure function over the tree and returns its
        // findings instead of printing them — the data/rendering split M3
        // established. They print after every parser diagnostic, which is the
        // phase order, and D6 (§9D.7) governs nothing else about it.
        let context = render::Context {
            source_name: Some(name),
            source_lines: &render_lines,
        };
        for finding in &semantic_findings {
            eprintln!("{}", render::render(finding, &context));
        }
        eprintln!("❌ Aborted: fix the errors above before running.");
        Some(RunFailure::Frontend(semantic_findings))
    } else if opts.check_only {
        evaluator.check_program(&program);
        None
    } else {
        let program_outcome = evaluator.eval_program_outcome(&program);
        evaluator.report_program_outcome(&program_outcome);
        let failure = match program_outcome {
            evaluator::ProgramOutcome::Value(_) => None,
            evaluator::ProgramOutcome::RuntimeError(error) => Some(RunFailure::Runtime(error)),
            evaluator::ProgramOutcome::UncaughtException { message } => {
                Some(RunFailure::UncaughtException { message })
            }
            evaluator::ProgramOutcome::InvalidControlFlow(flow) => {
                Some(RunFailure::InvalidControlFlow(flow))
            }
            evaluator::ProgramOutcome::UnstructuredError => Some(RunFailure::UnstructuredRuntime),
        };
        if std::env::var("SEREZ_ARENA_STATS").is_ok() {
            let (global, scoped) = evaluator.arena_stats();
            eprintln!("[arena] global={} scoped={}", global, scoped);
        }
        failure
    };

    use std::io::Write;
    let _ = std::io::stdout().flush();

    DetailedOutcome {
        exit_code: if failure.is_some() { 1 } else { 0 },
        failure,
    }
}

/// Lex/parse/evaluate a `.sz` file. Returns the process exit code: 0 on success,
/// 1 if the file can't be read, fails to parse, or the program ends with an
/// uncaught exception / runtime error.
pub fn run_file(file_path: &str, is_check: bool) -> i32 {
    // .szx files (serez-ui JSX) are translated to .sz first, then run.
    if file_path.ends_with(".szx") {
        return run_szx_file(file_path, is_check);
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
        let dir = if dir == std::path::Path::new("") {
            std::path::Path::new(".")
        } else {
            dir
        };
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

/// Run a `.szx` (serez-ui JSX) file: translate it to `.sz` with serez-ui's
/// translator, run the result, then clean up. This is what the old `szx.ps1` /
/// `szx.sh` wrappers did — now the runtime does it itself, so `sz app.szx` just
/// works (and opens the UI).
///
/// This lived in `szx.rs` and moved here to break two dependency cycles
/// (ROADMAP_STATE.md §5.6 and §5.38): the call to [`run_file`] on the last line
/// made the translator module depend on the entry point, and the entry point
/// already depended on the translator. Which door a file extension goes through
/// is an entry-point question; `szx` answers what a `.szx` file *says*, and
/// [`crate::szx::translate_szx_beside_source`] is the half of the old function
/// that stayed there.
fn run_szx_file(szx_path: &str, is_check: bool) -> i32 {
    let out_sz = match crate::szx::translate_szx_beside_source(szx_path) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("{message}");
            return 1;
        }
    };

    let code = run_file(out_sz.to_string_lossy().as_ref(), is_check);
    if code != 0 {
        // Diagnostics from a translated program carry the translated file's
        // name, line numbers and source snippet, and that file is removed a
        // line below — so the message named a path the reader could not open
        // and quoted a line they never wrote. Say so.
        eprintln!(
            "\u{2139}\u{fe0f}  the diagnostics above refer to the translated form of '{szx_path}', not to the source as written."
        );
    }
    let _ = std::fs::remove_file(&out_sz); // best-effort cleanup
    code
}

/// Run a snippet handed in as a string (`sz --eval`). No file, so no `serez.json`
/// and no permissions; lockdown is on because the source did not necessarily come
/// from whoever is running it.
pub fn run_eval(src: String, is_check: bool) -> i32 {
    run_source(
        src,
        "<eval>",
        RunOpts {
            check_only: is_check,
            ..RunOpts::sandboxed()
        },
    )
    .exit_code
}
