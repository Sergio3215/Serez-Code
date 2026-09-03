// sz-lsp — Language Server Protocol server for serez-code (.sz).
//
// A second binary target over the same library the `sz` interpreter uses. It
// declares no modules of its own: the server and everything under it live in
// `serez_code::lsp`, so the lexer, parser, semantic layer and type checker an
// editor sees are the same code, compiled once, that runs a program.
//
// It used to declare `mod ast; mod lexer; mod parser; …` and compile ten
// frontend modules into a second crate — ROADMAP_STATE.md §5.18. Editors launch
// it with stdio transport:
//
//   sz-lsp
//
// Capabilities: live diagnostics (parser + type checker), completion
// (keywords, namespaces + native methods, document symbols), hover,
// go-to-definition and document symbols.
use serez_code::lsp;

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
                eprintln!(
                    "sz-lsp: unknown argument '{}' (stdio server; flags: --version)",
                    other
                );
            }
        }
    }
    eprintln!(
        "sz-lsp v{} — serez-code language server (stdio)",
        env!("CARGO_PKG_VERSION")
    );
    std::process::exit(lsp::server::run());
}
