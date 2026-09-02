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
pub mod diagnostic;
pub mod evaluator;
pub mod handles;
pub mod lexer;
pub mod modules;
pub mod package_manager;
pub mod parser;
pub mod permissions;
pub mod region;
pub mod render;
pub mod repl;
pub mod run;
pub mod scope;
pub mod semantic;
pub mod span;
pub mod szx;
pub mod token;
pub mod type_checker;

// AOT pipeline. HIR/MIR and their validation are always compiled and tested;
// only the actual LLVM implementation is selected by the `llvm` feature.
// The backend is still experimental and is not wired to a CLI verb.
pub mod compiler;
