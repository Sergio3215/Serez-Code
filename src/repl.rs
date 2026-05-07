use crate::{lexer::Lexer, parser::Parser, evaluator::Evaluator};
use std::io::{self, Write};

const PROMPT: &str = ">> ";

pub fn start() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    let mut evaluator = Evaluator::new();

    loop {
        print!("{}", PROMPT);
        stdout.flush().unwrap();

        let mut input = String::new();
        let bytes_read = stdin.read_line(&mut input).unwrap();

        if bytes_read == 0 {
            return;
        }

        let lexer = Lexer::new(input);
        let mut parser = Parser::new(lexer);
        let program = parser.parse_program();

        // eval_program retorna Option<ObjectRef> — sin clonar datos
        if let Some(obj_ref) = evaluator.eval_program(&program) {
            println!("{}", evaluator.display(obj_ref));
        }
    }
}
