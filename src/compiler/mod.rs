/// Compiler pipeline: AST → HIR → MIR → LLVM IR → native binary.
///
/// The interpreter in `evaluator/` walks the AST and executes it directly.
/// This module instead *lowers* the AST through several intermediate
/// representations until it reaches LLVM IR, which is compiled to machine code.
///
/// Pipeline stages (each in its own submodule):
///   types      — compile-time type system (SzType)
///   hir        — High-level IR: desugared AST with resolved types
///   mir        — Mid-level IR: three-address code with basic blocks
///   llvm_emit  — LLVM IR emission via inkwell

pub mod types;
