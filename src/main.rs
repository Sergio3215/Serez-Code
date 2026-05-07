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
        let file_path = &args[1];
        if !file_path.ends_with(".sz") {
            println!("❌ ERROR: El archivo debe tener la extensión .sz");
            return;
        }

        let input = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                println!("❌ ERROR leyendo el archivo '{}': {}", file_path, e);
                return;
            }
        };

        let lexer = lexer::Lexer::new(input);
        let mut parser = parser::Parser::new(lexer);
        let program = parser.parse_program();

        // Ejecutamos el archivo de una sola vez
        let mut evaluator = evaluator::Evaluator::new();
        if let Some(r) = evaluator.eval_program(&program) {
            println!("{}", evaluator.display(r));
        }
    } else {
        println!("Hello Sergio! This is the Serez-Code programming language!");
        println!("Feel free to type in commands");
        repl::start();
    }
}
