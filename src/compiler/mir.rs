/// Mid-level IR — Three-Address Code (TAC) with basic blocks.
///
/// Every instruction has at most one operation and one result.
/// Control flow is expressed as explicit jumps between named basic blocks.
/// This representation maps directly onto LLVM IR.
///
/// Example:
///   let x = (a + b) * c;
///   ─────────────────────────────────────────
///   t0 = Load "a"
///   t1 = Load "b"
///   t2 = BinOp Add t0 t1
///   t3 = Load "c"
///   t4 = BinOp Mul t2 t3
///   Store "x" t4
use crate::compiler::{
    hir::{HirBinOp, HirUnaryOp},
    types::SzType,
};

// ── Temporaries ───────────────────────────────────────────────────────────────

/// An SSA-style temporary register — `t0`, `t1`, `t2` …
pub type Temp = usize;

// ── Values ────────────────────────────────────────────────────────────────────

/// An operand that can appear on the right-hand side of a MIR instruction.
#[derive(Debug, Clone)]
pub enum MirVal {
    /// Result of a previous instruction
    Temp(Temp),
    ConstInt(i64),
    ConstDecimal(f64),
    ConstBool(bool),
    ConstStr(String),
    Null,
}

// ── Instructions ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum MirInstr {
    /// t = val  — copy / move
    Copy(Temp, MirVal),
    /// t = var  — load named variable into temp
    Load(Temp, String),
    /// var = val — store val into named variable
    Store(String, MirVal),
    /// t = lhs op rhs
    BinOp(Temp, HirBinOp, MirVal, MirVal),
    /// t = op val
    UnaryOp(Temp, HirUnaryOp, MirVal),
    /// t = fn(args)
    Call(Option<Temp>, String, Vec<MirVal>),
    /// t = obj.method(args)
    MethodCall(Option<Temp>, MirVal, String, Vec<MirVal>),
    /// t = arr[idx]
    IndexLoad(Temp, MirVal, MirVal),
    /// arr[idx] = val
    IndexStore(MirVal, MirVal, MirVal),
    /// t = obj.field
    FieldLoad(Temp, MirVal, String),
    /// obj.field = val
    FieldStore(MirVal, String, MirVal),
    /// t = new Class(args)
    New(Temp, String, Vec<MirVal>),
    /// out val — runtime print
    Out(MirVal),
}

// ── Terminators ───────────────────────────────────────────────────────────────

/// Every basic block ends with exactly one terminator.
#[derive(Debug, Clone)]
pub enum Terminator {
    /// Unconditional jump
    Jump(String),
    /// Conditional branch: if val → true_label else false_label
    Branch(MirVal, String, String),
    /// Return from function, optionally with a value
    Return(Option<MirVal>),
}

// ── Basic blocks and functions ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: String,
    pub instrs: Vec<MirInstr>,
    pub term: Terminator,
}

#[derive(Debug, Clone)]
pub struct MirFunction {
    pub name: String,
    pub params: Vec<(String, SzType)>,
    pub ret_type: SzType,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug, Clone)]
pub struct MirProgram {
    pub functions: Vec<MirFunction>,
}
