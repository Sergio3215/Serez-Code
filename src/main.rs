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
    // .szx files (serez-ui JSX) are translated to .sz first, then run.
    if file_path.ends_with(".szx") {
        return run_szx_file(file_path, is_check);
    }

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

/// Locate serez-ui's `.szx → .sz` translator (`tools/translate.sz`), searching
/// the local project packages, the source file's packages, the global store, and
/// the executable's directory (for packaged apps that bundle serez-ui).
fn find_szx_translator(szx: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.join("packages"));
    }
    if let Some(dir) = szx.parent() {
        let dir = if dir == std::path::Path::new("") { std::path::Path::new(".") } else { dir };
        roots.push(dir.join("packages"));
    }
    roots.push(package_manager::packages_dir());
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            roots.push(d.to_path_buf());
        }
    }
    for r in roots {
        let cand = r.join("serez-ui").join("tools").join("translate.sz");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

/// Run a `.szx` (serez-ui JSX) file directly: translate it to `.sz` with
/// serez-ui's translator, run the result, then clean up. This is what the old
/// `szx.ps1` / `szx.sh` wrappers did — now the runtime does it itself, so
/// `sz app.szx` just works (and opens the UI).
fn run_szx_file(szx_path: &str, is_check: bool) -> i32 {
    let szx = std::path::Path::new(szx_path);
    if !szx.exists() {
        eprintln!("❌ ERROR reading file '{}': not found", szx_path);
        return 1;
    }
    let translator = match find_szx_translator(szx) {
        Some(t) => t,
        None => {
            eprintln!(
                "❌ ERROR: cannot run '{}': serez-ui not found. Install it with `sz install serez-ui` to run .szx files.",
                szx_path
            );
            return 1;
        }
    };
    let sz_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ ERROR: cannot locate the sz executable: {}", e);
            return 1;
        }
    };
    // Translate next to the source so the app's relative imports still resolve.
    let out_sz = szx.with_extension("szx.sz");
    let mut cmd = std::process::Command::new(&sz_exe);
    cmd.arg(&translator)
        .arg(szx)
        .arg(&out_sz)
        .stdout(std::process::Stdio::null()); // hide the translator's own chatter
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW); // never pop a console for the translate step
    }
    let status = cmd.status();
    let ok = matches!(status, Ok(ref s) if s.success()) && out_sz.exists();
    if !ok {
        let _ = std::fs::remove_file(&out_sz);
        eprintln!(
            "❌ ERROR: could not translate '{}' (is it valid .szx, and is serez-ui's translator present?)",
            szx_path
        );
        return 1;
    }
    let code = run_file(out_sz.to_string_lossy().as_ref(), is_check);
    let _ = std::fs::remove_file(&out_sz); // best-effort cleanup
    code
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
        // ── `sz install [pkg@version] [-g/--global]` subcommand ───────────────
        if args[1] == "install" {
            let global = args.iter().any(|a| a == "-g" || a == "--global");
            let spec = args.iter().skip(2).find(|a| !a.starts_with('-')).map(|s| s.as_str());
            return subcommand_code(match spec {
                Some(s) => package_manager::install_package(s, !global, global),
                None => package_manager::install_all(),
            });
        }

        // ── `sz uninstall [<pkg>] [-g/--global]` subcommand ───────────────────
        // `sz uninstall <pkg>`     → remove from ./packages (and serez.json)
        // `sz uninstall <pkg> -g`  → remove from the global store
        // `sz uninstall -g`        → remove ALL global packages
        if args[1] == "uninstall" {
            let global = args.iter().any(|a| a == "-g" || a == "--global");
            let name = args.iter().skip(2).find(|a| !a.starts_with('-')).map(|s| s.as_str());
            if let Some(n) = name {
                return subcommand_code(package_manager::uninstall_package(n, global));
            }
            if global {
                return subcommand_code(package_manager::uninstall_all_global());
            }
            eprintln!("❌ ERROR: Usage: sz uninstall <package-name> [-g]  (or `sz uninstall -g` to remove all global packages)");
            return 1;
        }

        // ── `sz update [<pkg>] [-g/--global]` subcommand ──────────────────────
        // Updates to the latest PUBLISHED version (queries the remote registry).
        // No name → updates all project deps (or all global packages with -g).
        if args[1] == "update" {
            let global = args.iter().any(|a| a == "-g" || a == "--global");
            let name = args.iter().skip(2).find(|a| !a.starts_with('-')).map(|s| s.as_str());
            return subcommand_code(match name {
                Some(n) => package_manager::update_package(n, global),
                None => package_manager::update_all(global),
            });
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

        if !file_path.ends_with(".sz") && !file_path.ends_with(".szx") {
            eprintln!("❌ ERROR: File must have a .sz extension (or .szx for serez-ui)");
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
