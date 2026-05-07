mod ast;
mod evaluator;
mod lexer;
mod parser;
mod region;
mod repl;
mod scope;
mod token;

use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let mut is_check = false;
        let mut file_path = String::new();

        if args.contains(&"--version".to_string()) {
            println!("Serez-Code v{}", env!("CARGO_PKG_VERSION"));
            return;
        }

        for arg in args.iter().skip(1) {
            if arg == "--check" {
                is_check = true;
            } else if file_path.is_empty() {
                file_path = arg.clone();
            }
        }

        if file_path.is_empty() {
            println!("❌ ERROR: You must provide a .sz file to execute or check.");
            return;
        }

        if !file_path.ends_with(".sz") {
            println!("❌ ERROR: File must have a .sz extension");
            return;
        }

        let input = match fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(e) => {
                println!("❌ ERROR reading file '{}': {}", file_path, e);
                return;
            }
        };

        let lexer = lexer::Lexer::new(input);
        let mut parser = parser::Parser::new(lexer);
        let program = parser.parse_program();

        let mut evaluator = evaluator::Evaluator::new();
        if is_check {
            evaluator.check_program(&program);
        } else {
            if let Some(r) = evaluator.eval_program(&program) {
                println!("{}", evaluator.display(r));
            }
        }
    } else {
        println!("Hello Sergio! This is the Serez-Code programming language!");
        println!("Feel free to type in commands");
        repl::start();
    }
}
