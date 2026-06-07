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

fn run_file(file_path: &str, is_check: bool) {
    let input = match fs::read_to_string(file_path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("❌ ERROR reading file '{}': {}", file_path, e);
            return;
        }
    };

    let source_lines: Vec<String> = input.lines().map(|l| l.to_string()).collect();

    let lexer = lexer::Lexer::new(input);
    let mut parser = parser::Parser::new(lexer);
    parser.set_source(source_lines.clone());
    let program = parser.parse_program();

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
    if is_check {
        evaluator.check_program(&program);
    } else {
        evaluator.eval_program(&program);
        if std::env::var("SEREZ_ARENA_STATS").is_ok() {
            let (global, scoped) = evaluator.arena_stats();
            eprintln!("[arena] global={} scoped={}", global, scoped);
        }
    }
    let _ = std::io::stdout().flush();
}

fn run() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        // ── `sz install [pkg@version]` subcommand ─────────────────────────────
        if args[1] == "install" {
            if args.len() >= 3 {
                let spec = &args[2];
                if let Err(e) = package_manager::install_package(spec, true) {
                    eprintln!("❌ ERROR: {}", e);
                }
            } else {
                if let Err(e) = package_manager::install_all() {
                    eprintln!("❌ ERROR: {}", e);
                }
            }
            return;
        }

        // ── `sz uninstall <pkg>` subcommand ───────────────────────────────────
        if args[1] == "uninstall" {
            if args.len() >= 3 {
                let name = &args[2];
                if let Err(e) = package_manager::uninstall_package(name) {
                    eprintln!("❌ ERROR: {}", e);
                }
            } else {
                eprintln!("❌ ERROR: Usage: sz uninstall <package-name>");
            }
            return;
        }

        // ── `sz publish` subcommand ───────────────────────────────────────────
        if args[1] == "publish" {
            if let Err(e) = package_manager::publish_package() {
                eprintln!("❌ ERROR: {}", e);
            }
            return;
        }

        // ── `sz unpublish <pkg>@<version>` subcommand ────────────────────────
        if args[1] == "unpublish" {
            if args.len() >= 3 {
                if let Err(e) = package_manager::unpublish_package_remote(&args[2]) {
                    eprintln!("❌ ERROR: {}", e);
                }
            } else {
                eprintln!("❌ ERROR: Usage: sz unpublish <package>@<version>");
            }
            return;
        }

        // ── `sz info <pkg>` subcommand ────────────────────────────────────────
        if args[1] == "info" {
            if args.len() >= 3 {
                if let Err(e) = package_manager::info_package(&args[2]) {
                    eprintln!("❌ ERROR: {}", e);
                }
            } else {
                eprintln!("❌ ERROR: Usage: sz info <package-name>");
            }
            return;
        }

        // ── `sz init [--y]` subcommand ────────────────────────────────────────
        if args[1] == "init" {
            let yes = args.iter().any(|a| a == "--y");
            if let Err(e) = package_manager::init_project(yes) {
                eprintln!("❌ ERROR: {}", e);
            }
            return;
        }

        // ── `sz run <script>` subcommand ──────────────────────────────────────
        if args[1] == "run" {
            if args.len() >= 3 {
                if let Err(e) = package_manager::run_script(&args[2]) {
                    eprintln!("❌ ERROR: {}", e);
                }
            } else {
                eprintln!("❌ ERROR: Usage: sz run <script-name>");
            }
            return;
        }

        let mut is_check = false;
        let mut is_watch = false;
        let mut file_path = String::new();

        if args.contains(&"--version".to_string()) {
            println!("Serez-Code v{}", env!("CARGO_PKG_VERSION"));
            return;
        }

        for arg in args.iter().skip(1) {
            if arg == "--check" {
                is_check = true;
            } else if arg == "--watch" {
                is_watch = true;
            } else if arg.starts_with("--") {
                eprintln!("❌ ERROR: Unknown flag '{}'", arg);
                return;
            } else if file_path.is_empty() {
                file_path = arg.clone();
            }
        }

        if file_path.is_empty() {
            eprintln!("❌ ERROR: You must provide a .sz file to execute or check.");
            return;
        }

        if !file_path.ends_with(".sz") {
            eprintln!("❌ ERROR: File must have a .sz extension");
            return;
        }

        if is_watch {
            use notify::{EventKind, RecursiveMode, Watcher, recommended_watcher};
            use std::path::Path;
            use std::sync::mpsc;
            use std::time::{Duration, Instant};

            println!("👁  Watching {} — press Ctrl+C to stop", file_path);
            run_file(&file_path, is_check);

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
                            run_file(&file_path, is_check);
                            last_run = Instant::now();
                        }
                    }
                }
            }
        } else {
            run_file(&file_path, is_check);
        }
    } else {
        println!("Hello Sergio! This is the Serez-Code programming language!");
        println!("Feel free to type in commands");
        repl::start();
    }
}

fn main() {
    // Run on a thread with 64 MB stack to support deep recursion in user programs
    let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
    let handler = builder.spawn(run).expect("Failed to spawn interpreter thread");
    handler.join().expect("Interpreter thread panicked");
}
