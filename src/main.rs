//! The `sz` CLI. Argument parsing and the GUI host thread live here; everything
//! that actually runs code lives in the library (see `src/lib.rs`), so `--eval`
//! and the wasm build share one pipeline with `sz file.sz`.

use serez_code::evaluator;
use serez_code::package_manager;
use serez_code::repl;
use serez_code::run::{run_eval, run_file};

use std::env;

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

/// Read the whole of stdin, for `sz --eval -`. Passing a multi-line snippet as an
/// argv string means fighting the shell over quotes, newlines and `$`; a pipe
/// doesn't have that problem.
fn read_stdin() -> Option<String> {
    use std::io::Read;
    let mut buf = String::new();
    match std::io::stdin().read_to_string(&mut buf) {
        Ok(_) => Some(buf),
        Err(e) => {
            eprintln!("❌ ERROR reading stdin: {}", e);
            None
        }
    }
}

/// Usage text for `sz --help`.
///
/// Printed on **stdout** with exit code 0: asking for help is not an error, and
/// a tool that answers `sz --help` on stderr with exit 1 cannot be piped into a
/// pager or checked by a script. `sz` with no arguments starts the REPL, so the
/// only way to discover the surface was to read the source.
fn print_help() {
    println!(
        "\
Serez-Code v{version}

USAGE
  sz <file.sz|file.szx>          Run a program
  sz --check <file>              Type-check and report, without running
  sz --watch <file>              Re-run the file whenever it changes
  sz --eval \"<code>\"             Run a snippet (no manifest, lockdown on)
  sz --eval -                    Read the snippet from stdin
  sz                             Start the REPL
  sz --version                   Print the version
  sz --help                      Print this message

PACKAGES
  sz init [--y]                  Create serez.json in this directory
  sz install [<pkg>[@<ver>]]     Install one package, or every dependency
  sz uninstall <pkg>             Remove a package
  sz update [<pkg>]              Update one package, or all of them
  sz info <pkg>                  Show a package's manifest
  sz run <script> [args...]      Run a script from serez.json
  sz publish                     Publish this package to the registry
  sz unpublish <pkg>@<ver>       Remove a published version
  sz logout                      Forget the stored registry credentials

  -g, --global                   Apply the package command to the global store

EXIT CODES
  0   success
  1   usage error, failed subcommand, parse error, type error,
      runtime error, or uncaught exception

Diagnostics go to stderr and carry a stable code (SZ1xxx lexer, SZ2xxx parser,
SZ3xxx types, SZ4xxx runtime, SZ5xxx modules, SZ6xxx permissions and limits,
SZ7xxx compiler). Match on the code, never on the wording.",
        version = env!("CARGO_PKG_VERSION")
    );
}

/// Process entry point. Returns the exit code: 0 on success, non-zero on any
/// usage error, subcommand failure, parse error, or uncaught runtime exception.
fn run() -> i32 {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        // ── `sz install [pkg@version] [-g/--global]` subcommand ───────────────
        if args[1] == "install" {
            let global = args.iter().any(|a| a == "-g" || a == "--global");
            let spec = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with('-'))
                .map(|s| s.as_str());
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
            let name = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with('-'))
                .map(|s| s.as_str());
            if let Some(n) = name {
                return subcommand_code(package_manager::uninstall_package(n, global));
            }
            if global {
                return subcommand_code(package_manager::uninstall_all_global());
            }
            eprintln!(
                "❌ ERROR: Usage: sz uninstall <package-name> [-g]  (or `sz uninstall -g` to remove all global packages)"
            );
            return 1;
        }

        // ── `sz update [<pkg>] [-g/--global]` subcommand ──────────────────────
        // Updates to the latest PUBLISHED version (queries the remote registry).
        // No name → updates all project deps (or all global packages with -g).
        if args[1] == "update" {
            let global = args.iter().any(|a| a == "-g" || a == "--global");
            let name = args
                .iter()
                .skip(2)
                .find(|a| !a.starts_with('-'))
                .map(|s| s.as_str());
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

        // ── `sz logout` subcommand ────────────────────────────────────────────
        // Removes the stored registry credential; the next `sz publish` asks
        // for username/password again (lets you switch accounts).
        if args[1] == "logout" {
            return subcommand_code(package_manager::logout());
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

        if args.iter().any(|a| a == "--help" || a == "-h") || args[1] == "help" {
            print_help();
            return 0;
        }

        // ── `sz --eval "<code>"` / `sz --eval -` ──────────────────────────────
        // Runs a snippet with no file behind it: no serez.json, so no permissions,
        // and lockdown on. Handled before the flag loop below because its argument
        // is arbitrary source text, not a path.
        if let Some(i) = args.iter().position(|a| a == "--eval" || a == "-e") {
            let is_check = args.iter().any(|a| a == "--check");
            let src = match args.get(i + 1) {
                Some(a) if a == "-" => match read_stdin() {
                    Some(s) => s,
                    None => return 1,
                },
                Some(a) => a.clone(),
                None => {
                    eprintln!(
                        "❌ ERROR: Usage: sz --eval \"<code>\"  (or `sz --eval -` to read the snippet from stdin)"
                    );
                    return 1;
                }
            };
            return run_eval(src, is_check);
        }

        for arg in args.iter().skip(1) {
            if arg == "--check" {
                is_check = true;
            } else if arg == "--watch" {
                is_watch = true;
            } else if arg.starts_with("--") {
                eprintln!(
                    "❌ ERROR: Unknown flag '{}'. Run `sz --help` for usage.",
                    arg
                );
                return 1;
            } else if file_path.is_empty() {
                file_path = arg.clone();
            }
        }

        if file_path.is_empty() {
            eprintln!(
                "❌ ERROR: You must provide a .sz file to execute or check. \
                 Run `sz --help` for usage."
            );
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
    use std::sync::Arc;

    // El intérprete corre en un hilo de 64 MB (recursión profunda). winit EXIGE que
    // el EventLoop viva en el hilo PRINCIPAL (obligatorio en macOS; correcto en
    // Windows/Linux). Por eso el hilo principal hospeda la GUI (gui_host_main_loop) y
    // el intérprete se comunica con la ventana por GUI_HOST. Ver namespaces_gui.rs.
    let host = Arc::new(evaluator::namespaces_gui::GuiHost::new());
    let _ = evaluator::namespaces_gui::GUI_HOST.set(host.clone());

    let builder = std::thread::Builder::new().stack_size(64 * 1024 * 1024);
    let h2 = host.clone();
    let handler = builder
        .spawn(move || {
            // catch_unwind para que un panic del intérprete no cuelgue al host main.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
            let code = result.unwrap_or(101);
            h2.signal_interp_done(code);
        })
        .expect("Failed to spawn interpreter thread");

    // Hilo principal: hospeda el EventLoop de winit. Sale cuando el intérprete acaba.
    evaluator::namespaces_gui::gui_host_main_loop(host.clone());

    let _ = handler.join();
    std::process::exit(host.exit_code());
}
