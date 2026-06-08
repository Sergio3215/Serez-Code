#![allow(dead_code)]
mod ast;
mod compiler;
mod evaluator;
mod lexer;
mod package_manager;
mod parser;
mod region;
mod repl;
mod scope;
mod token;
mod type_checker;

use std::env;
use std::fs;
use std::io::Write;

/// Lex/parse/evaluate a `.sz` file. Returns the process exit code:
/// 0 on success, 1 if the file can't be read, fails to parse, or the program
/// ends with an uncaught exception / runtime error.
fn run_file(file_path: &str, is_check: bool) -> i32 {
    let input = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("❌ ERROR reading file '{}': {}", file_path, e);
            return 1;
        }
    };

    let source_lines: Vec<String> = input.lines().map(|l| l.to_string()).collect();

    let lexer = lexer::Lexer::new(input);
    let mut parser = parser::Parser::new(lexer);
    parser.set_source(source_lines.clone());
    let program = parser.parse_program();
    let parse_failed = parser.has_errors();

    let mut checker = type_checker::TypeChecker::new(&program);
    checker.check();

    let mut evaluator = evaluator::Evaluator::new();
    evaluator.set_source(source_lines);

    // Load permissions from serez.json if present in the file's directory
    let file_path_obj = std::path::Path::new(file_path);
    if let Some(dir) = file_path_obj.parent() {
        let dir = if dir == std::path::Path::new("") { std::path::Path::new(".") } else { dir };
        if let Ok(manifest) = package_manager::SerezManifest::load(dir) {
            evaluator.set_permissions(manifest.permissions);
        }
    }

    evaluator.set_current_file(file_path_obj);
    let mut run_failed = false;
    if is_check {
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
    let _ = std::io::stdout().flush();

    if parse_failed || run_failed { 1 } else { 0 }
}

/// Print a subcommand error (if any) and map it to a process exit code.
fn subcommand_code(result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("❌ ERROR: {}", e);
            1
        }
    }
}

/// Process entry point. Returns the exit code: 0 on success, non-zero on any
/// usage error, subcommand failure, parse error, or uncaught runtime exception.
fn run() -> i32 {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        // ── `sz install [pkg@version]` subcommand ─────────────────────────────
        if args[1] == "install" {
            return subcommand_code(if args.len() >= 3 {
                package_manager::install_package(&args[2], true)
            } else {
                package_manager::install_all()
            });
        }

        // ── `sz uninstall <pkg>` subcommand ───────────────────────────────────
        if args[1] == "uninstall" {
            if args.len() >= 3 {
                return subcommand_code(package_manager::uninstall_package(&args[2]));
            }
            eprintln!("❌ ERROR: Usage: sz uninstall <package-name>");
            return 1;
        }

        // ── `sz publish` subcommand ───────────────────────────────────────────
        if args[1] == "publish" {
            return subcommand_code(package_manager::publish_package());
        }

        // ── `sz unpublish <pkg>@<version>` subcommand ────────────────────────
        if args[1] == "unpublish" {
            if args.len() >= 3 {
                return subcommand_code(package_manager::unpublish_package_remote(&args[2]));
            }
            eprintln!("❌ ERROR: Usage: sz unpublish <package>@<version>");
            return 1;
        }

        // ── `sz info <pkg>` subcommand ────────────────────────────────────────
        if args[1] == "info" {
            if args.len() >= 3 {
                return subcommand_code(package_manager::info_package(&args[2]));
            }
            eprintln!("❌ ERROR: Usage: sz info <package-name>");
            return 1;
        }

        // ── `sz init [--y]` subcommand ────────────────────────────────────────
        if args[1] == "init" {
            let yes = args.iter().any(|a| a == "--y");
            return subcommand_code(package_manager::init_project(yes));
        }

        // ── `sz run <script-or-command> [args...]` subcommand ─────────────────
        if args[1] == "run" {
            if args.len() >= 3 {
                return subcommand_code(package_manager::run_script(&args[2], &args[3..]));
            }
            eprintln!("❌ ERROR: Usage: sz run <script-or-command> [args...]");
            return 1;
        }

        let mut is_check = false;
        let mut is_watch = false;
        let mut file_path = String::new();

        if args.contains(&"--version".to_string()) {
            println!("Serez-Code v{}", env!("CARGO_PKG_VERSION"));
            return 0;
        }

        for arg in args.iter().skip(1) {
            if arg == "--check" {
                is_check = true;
            } else if arg == "--watch" {
                is_watch = true;
            } else if arg.starts_with("--") {
                eprintln!("❌ ERROR: Unknown flag '{}'", arg);
                return 1;
            } else if file_path.is_empty() {
                file_path = arg.clone();
            }
        }

        if file_path.is_empty() {
            eprintln!("❌ ERROR: You must provide a .sz file to execute or check.");
            return 1;
        }

        if !file_path.ends_with(".sz") {
            eprintln!("❌ ERROR: File must have a .sz extension");
            return 1;
        }

        if is_watch {
            use notify::{EventKind, RecursiveMode, Watcher, recommended_watcher};
            use std::path::Path;
            use std::sync::mpsc;
            use std::time::{Duration, Instant};

            println!("👁  Watching {} — press Ctrl+C to stop", file_path);
            let _ = run_file(&file_path, is_check);

            let (tx, rx) = mpsc::channel();
            let mut watcher = recommended_watcher(tx).expect("Failed to create watcher");
            watcher
                .watch(Path::new(&file_path), RecursiveMode::NonRecursive)
                .expect("Failed to watch file");

            let mut last_run = Instant::now();
            loop {
                if let Ok(Ok(event)) = rx.recv() {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        // debounce: ignore events within 50ms of the last run
                        if last_run.elapsed() > Duration::from_millis(50) {
                            print!("\x1B[2J\x1B[1;1H"); // clear screen
                            println!("👁  Watching {} — press Ctrl+C to stop\n", file_path);
                            let _ = run_file(&file_path, is_check);
                            last_run = Instant::now();
                        }
                    }
                }
            }
        } else {
            run_file(&file_path, is_check)
        }
    } else {
        println!("Hello Sergio! This is the Serez-Code programming language!");
        println!("Feel free to type in commands");
        repl::start();
        0
    }
}

fn main() {
    // Run on a thread with 64 MB stack to support deep recursion in user programs
    let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
    let handler = builder.spawn(run).expect("Failed to spawn interpreter thread");
    let code = handler.join().expect("Interpreter thread panicked");
    std::process::exit(code);
}
