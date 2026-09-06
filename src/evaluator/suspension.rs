//! Logical suspension and resumption framework for Serez Code v11.1.0 (DEC-ASYNC-001).
//!
//! Models suspended executions without blocking the Evaluator thread or leaking state.
//! Captures the continuation frames across statements, functions, and try/catch blocks
//! so the Evaluator can yield control to the scheduler and resume exactly once when I/O completes.

use super::async_runtime::AsyncOperationId;
use crate::ast::{self, Statement};

/// Continuation frame for a statement that yielded during expression evaluation.
#[derive(Debug, Clone)]
pub enum StatementContinuation {
    /// Resuming a `let` statement: `let <name>: <type?> = <awaited_value>;`
    Let { name: String, is_const: bool },
    /// Resuming an assignment statement: `<name> = <awaited_value>;`
    Assign { name: String },
    /// Resuming an expression statement: `<awaited_value>;`
    Expression,
    /// Resuming a `return` statement: `return <awaited_value>;`
    Return,
    /// Resuming an `out` statement: `out <awaited_value>;`
    Out,
}

/// Continuation frame for a try/catch/finally block.
#[derive(Debug, Clone)]
pub struct TryContinuation {
    pub catch_var: Option<String>,
    pub catch_body: Option<ast::BlockStatement>,
    pub finally_body: Option<ast::BlockStatement>,
    pub remaining_try_statements: Vec<Statement>,
}

/// Continuation frame representing a function call boundary.
#[derive(Debug, Clone)]
pub struct FunctionContinuation {
    pub call_name: String,
    pub return_type: Option<String>,
    pub is_async: bool,
    pub remaining_caller_statements: Vec<Statement>,
    pub caller_statement: Option<Box<StatementContinuation>>,
}

/// A frame in the suspension unwind stack.
#[derive(Debug, Clone)]
pub enum ContinuationFrame {
    Statement(StatementContinuation),
    Try(TryContinuation),
    Function(FunctionContinuation),
    Block {
        remaining_statements: Vec<Statement>,
    },
}

/// Context capturing a suspended execution state on an `Evaluator`.
pub struct SuspendedContext {
    pub op_id: AsyncOperationId,
    pub frames: Vec<ContinuationFrame>,
    pub full: bool,
    pub binary: bool,
}

impl SuspendedContext {
    pub fn new(op_id: AsyncOperationId, full: bool, binary: bool) -> Self {
        Self {
            op_id,
            frames: Vec::new(),
            full,
            binary,
        }
    }

    pub fn push_frame(&mut self, frame: ContinuationFrame) {
        self.frames.push(frame);
    }
}
