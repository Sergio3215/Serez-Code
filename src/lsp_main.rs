// sz-lsp — Language Server Protocol server for serez-code (.sz).
//
// A second binary target that reuses the interpreter's frontend modules
// (lexer/parser/type_checker) directly; the `sz` interpreter binary is not
// affected. Editors launch it with stdio transport:
//
//   sz-lsp
//
// Capabilities: live diagnostics (parser + type checker), completion
// (keywords, namespaces + native methods, document symbols), hover,
// go-to-definition and document symbols.
#![allow(dead_code)]
mod ast;
mod lexer;
mod lsp;
mod parser;
mod token;
mod type_checker;

fn main() {
    // Anything a server logs must go to stderr — stdout carries the protocol.
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-v" => {
                println!("sz-lsp v{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "--stdio" => {} // transport flag some clients pass; stdio is the only mode
            other => {
                eprintln!("sz-lsp: unknown argument '{}' (stdio server; flags: --version)", other);
            }
        }
    }
    eprintln!("sz-lsp v{} — serez-code language server (stdio)", env!("CARGO_PKG_VERSION"));
    std::process::exit(lsp::server::run());
}
