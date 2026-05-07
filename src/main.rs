mod ast;
mod evaluator;
mod lexer;
mod parser;
mod region;
mod repl;
mod scope;
mod token;

use std::fs;

fn main() {
    println!("Hello Sergio! This is the Serez-Code programming language!");
    println!("Feel free to type in commands");

    repl::start();
}
