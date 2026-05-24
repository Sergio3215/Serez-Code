/// High-level IR — desugared, typed AST.
///
/// All syntactic sugar from the source AST has been eliminated:
///   - ForEach        → index-based For loop
///   - Switch         → if/else chain
///   - DoWhile        → body + While
///   - Ternary / ??   → HirExpr::If
///   - ?.             → null-checked MethodCall
///   - Interpolation  → string concatenation chain
///
/// Every expression node carries an SzType resolved during lowering.
use crate::compiler::types::SzType;

// ── Program ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HirProgram {
    pub functions: Vec<HirFunction>,
}

// ── Function ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct HirFunction {
    pub name: String,
    pub params: Vec<HirParam>,
    pub ret_type: SzType,
    pub body: Vec<HirStmt>,
}

#[derive(Debug, Clone)]
pub struct HirParam {
    pub name: String,
    pub ty: SzType,
}

// ── Statements ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum HirStmt {
    Let { name: String, ty: SzType, value: HirExpr, is_const: bool },
    Assign(HirLValue, HirExpr),
    If { cond: HirExpr, then_body: Vec<HirStmt>, else_body: Vec<HirStmt> },
    While { cond: HirExpr, body: Vec<HirStmt> },
    For {
        init: Box<HirStmt>,
        cond: HirExpr,
        update: Box<HirStmt>,
        body: Vec<HirStmt>,
    },
    Return(Option<HirExpr>),
    Out(HirExpr),
    Block(Vec<HirStmt>),
    Break,
    Continue,
    ExprStmt(HirExpr),
}

#[derive(Debug, Clone)]
pub enum HirLValue {
    Var(String),
    Index { array: Box<HirExpr>, index: Box<HirExpr> },
    Field { object: Box<HirExpr>, field: String },
}

// ── Expressions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum HirExpr {
    Var(String, SzType),
    LitInt(i64),
    LitDecimal(f64),
    LitBool(bool),
    LitStr(String),
    Null,
    BinOp { op: HirBinOp, left: Box<HirExpr>, right: Box<HirExpr>, ty: SzType },
    UnaryOp { op: HirUnaryOp, operand: Box<HirExpr>, ty: SzType },
    Call { name: String, args: Vec<HirExpr>, ty: SzType },
    MethodCall { object: Box<HirExpr>, method: String, args: Vec<HirExpr>, ty: SzType },
    Index { array: Box<HirExpr>, index: Box<HirExpr>, ty: SzType },
    Field { object: Box<HirExpr>, name: String, ty: SzType },
    New { class: String, args: Vec<HirExpr> },
    Array { elements: Vec<HirExpr>, elem_ty: SzType },
    /// Conditional expression — produced by ternary, ??, and if-expressions.
    If { cond: Box<HirExpr>, then_expr: Box<HirExpr>, else_expr: Box<HirExpr>, ty: SzType },
}

impl HirExpr {
    pub fn ty(&self) -> SzType {
        match self {
            HirExpr::Var(_, t)                  => t.clone(),
            HirExpr::LitInt(_)                  => SzType::Int,
            HirExpr::LitDecimal(_)              => SzType::Decimal,
            HirExpr::LitBool(_)                 => SzType::Bool,
            HirExpr::LitStr(_)                  => SzType::Str,
            HirExpr::Null                       => SzType::Null,
            HirExpr::BinOp { ty, .. }           => ty.clone(),
            HirExpr::UnaryOp { ty, .. }         => ty.clone(),
            HirExpr::Call { ty, .. }            => ty.clone(),
            HirExpr::MethodCall { ty, .. }      => ty.clone(),
            HirExpr::Index { ty, .. }           => ty.clone(),
            HirExpr::Field { ty, .. }           => ty.clone(),
            HirExpr::New { class, .. }          => SzType::Class(class.clone()),
            HirExpr::Array { elem_ty, .. }      => SzType::Array(Box::new(elem_ty.clone())),
            HirExpr::If { ty, .. }              => ty.clone(),
        }
    }
}

// ── Operators ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum HirBinOp {
    Add, Sub, Mul, Div, Mod, Pow,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    BitAnd, BitOr, BitXor, Shl, Shr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HirUnaryOp {
    Neg,
    Not,
}
