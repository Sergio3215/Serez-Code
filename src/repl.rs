use crate::token::TokenType;
use crate::{lexer::Lexer, parser::Parser};
use std::io::{self, Write};

const PROMPT: &str = ">> ";

pub fn start() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("{}", PROMPT);
        stdout.flush().unwrap();

        let mut input = String::new();
        let bytes_read = stdin.read_line(&mut input).unwrap();

        if bytes_read == 0 {
            return;
        }

        let lexer = Lexer::new(input);

        // 2. Le pasamos el Lexer al Parser
        let mut p = Parser::new(lexer);

        // 3. Le pedimos al Parser que construya el AST
        let program = p.parse_program();

        // 4. Imprimimos el Árbol de Sintaxis Abstracta estructurado
        println!("{:#?}", program);
    }
}
