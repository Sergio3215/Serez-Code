mod ast;
mod evaluator;
mod lexer;
mod parser;
mod region;
mod repl;
mod scope;
mod token;
mod type_checker;

use std::env;
use std::fs;

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

    let mut checker = type_checker::TypeChecker::new(program.clone());
    checker.check();

    let mut evaluator = evaluator::Evaluator::new();
    evaluator.set_source(source_lines);
    if is_check {
        evaluator.check_program(&program);
    } else {
        evaluator.eval_program(&program);
    }
}

fn run() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
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
