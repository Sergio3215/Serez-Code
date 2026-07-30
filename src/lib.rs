//! Serez-Code as a library.
//!
//! The interpreter used to live entirely inside the `sz` binary, which meant the
//! only way to run code was to hand the CLI a path. Everything is here instead so
//! there can be more than one door onto the same pipeline:
//!
//!   * `sz file.sz`      → [`run::run_file`]  (reads disk, permissions from serez.json)
//!   * `sz --eval "..."` → [`run::run_eval`]  (source as a string, no permissions)
//!
//! Both land in [`run::run_source`]; nothing below the lexer knows which door it
//! came through.

#![allow(dead_code)]

pub mod ast;
pub mod evaluator;
pub mod lexer;
pub mod package_manager;
pub mod parser;
pub mod region;
pub mod repl;
pub mod run;
pub mod scope;
pub mod szx;
pub mod token;
pub mod type_checker;

// AOT backend (AST->HIR->MIR->LLVM IR): compiled only with `--features llvm`.
// Phase 1; not wired to any CLI verb yet - gating it keeps ~3k lines and the
// inkwell dependency out of the default build until the backend is resumed.
#[cfg(feature = "llvm")]
pub mod compiler;
