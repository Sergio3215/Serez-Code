mod ast;
mod lexer;
mod parser;
mod repl;
mod token;

use std::env;

fn main() {
    let user = env::var("USERNAME").unwrap_or_else(|_| "User".to_string());
    println!("Hello {}! This is the Monkey programming language!", user);
    println!("Feel free to type in commands");
    repl::start();
}
