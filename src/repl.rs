use crate::lexer::Lexer;
use crate::token::TokenType;
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

        let mut l = Lexer::new(input);

        loop {
            let tok = l.next_token();
            if tok.token_type == TokenType::Eof {
                break;
            }
            println!("{:?}", tok);
        }
    }
}
