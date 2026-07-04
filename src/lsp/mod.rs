// sz-lsp: Language Server Protocol implementation for serez-code.
// Reuses the interpreter's lexer/parser/type-checker for diagnostics and a
// token-level scanner for symbols; speaks JSON-RPC over stdio.
pub mod analysis;
pub mod builtins;
pub mod builtins_gen;
pub mod rpc;
pub mod server;
