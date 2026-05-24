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

#[cfg(test)]
mod tests {
    use super::*;

    // ── HirExpr::ty() ────────────────────────────────────────────────────────

    #[test]
    fn literal_types() {
        assert_eq!(HirExpr::LitInt(0).ty(),               SzType::Int);
        assert_eq!(HirExpr::LitInt(-999).ty(),            SzType::Int);
        assert_eq!(HirExpr::LitDecimal(3.14).ty(),        SzType::Decimal);
        assert_eq!(HirExpr::LitBool(true).ty(),           SzType::Bool);
        assert_eq!(HirExpr::LitBool(false).ty(),          SzType::Bool);
        assert_eq!(HirExpr::LitStr("hi".to_string()).ty(), SzType::Str);
        assert_eq!(HirExpr::Null.ty(),                    SzType::Null);
    }

    #[test]
    fn var_preserves_its_type() {
        let e = HirExpr::Var("score".to_string(), SzType::Decimal);
        assert_eq!(e.ty(), SzType::Decimal);

        let e2 = HirExpr::Var("flag".to_string(), SzType::Bool);
        assert_eq!(e2.ty(), SzType::Bool);
    }

    #[test]
    fn binop_carries_result_type() {
        let add = HirExpr::BinOp {
            op: HirBinOp::Add,
            left:  Box::new(HirExpr::LitInt(1)),
            right: Box::new(HirExpr::LitInt(2)),
            ty: SzType::Int,
        };
        assert_eq!(add.ty(), SzType::Int);

        let cmp = HirExpr::BinOp {
            op: HirBinOp::Lt,
            left:  Box::new(HirExpr::LitInt(1)),
            right: Box::new(HirExpr::LitInt(2)),
            ty: SzType::Bool,
        };
        assert_eq!(cmp.ty(), SzType::Bool);
    }

    #[test]
    fn unary_op_carries_type() {
        let neg = HirExpr::UnaryOp {
            op:      HirUnaryOp::Neg,
            operand: Box::new(HirExpr::LitInt(5)),
            ty:      SzType::Int,
        };
        assert_eq!(neg.ty(), SzType::Int);

        let not = HirExpr::UnaryOp {
            op:      HirUnaryOp::Not,
            operand: Box::new(HirExpr::LitBool(true)),
            ty:      SzType::Bool,
        };
        assert_eq!(not.ty(), SzType::Bool);
    }

    #[test]
    fn call_returns_declared_type() {
        let e = HirExpr::Call {
            name: "parse".to_string(),
            args: vec![HirExpr::LitStr("42".to_string())],
            ty:   SzType::Int,
        };
        assert_eq!(e.ty(), SzType::Int);
    }

    #[test]
    fn method_call_returns_declared_type() {
        let e = HirExpr::MethodCall {
            object: Box::new(HirExpr::Var("s".to_string(), SzType::Str)),
            method: "length".to_string(),
            args:   vec![],
            ty:     SzType::Int,
        };
        assert_eq!(e.ty(), SzType::Int);
    }

    #[test]
    fn index_returns_declared_element_type() {
        let e = HirExpr::Index {
            array: Box::new(HirExpr::Var("arr".to_string(), SzType::Array(Box::new(SzType::Int)))),
            index: Box::new(HirExpr::LitInt(0)),
            ty:    SzType::Int,
        };
        assert_eq!(e.ty(), SzType::Int);
    }

    #[test]
    fn field_returns_declared_type() {
        let e = HirExpr::Field {
            object: Box::new(HirExpr::Var("p".to_string(), SzType::Class("Point".to_string()))),
            name:   "x".to_string(),
            ty:     SzType::Decimal,
        };
        assert_eq!(e.ty(), SzType::Decimal);
    }

    #[test]
    fn new_type_is_class_with_name() {
        let e = HirExpr::New { class: "Vec2".to_string(), args: vec![] };
        assert_eq!(e.ty(), SzType::Class("Vec2".to_string()));
    }

    #[test]
    fn array_literal_type_wraps_element() {
        let e = HirExpr::Array {
            elements: vec![HirExpr::LitInt(1), HirExpr::LitInt(2)],
            elem_ty:  SzType::Int,
        };
        assert_eq!(e.ty(), SzType::Array(Box::new(SzType::Int)));
    }

    #[test]
    fn if_expr_carries_branch_type() {
        let e = HirExpr::If {
            cond:      Box::new(HirExpr::LitBool(true)),
            then_expr: Box::new(HirExpr::LitDecimal(1.0)),
            else_expr: Box::new(HirExpr::LitDecimal(0.0)),
            ty:        SzType::Decimal,
        };
        assert_eq!(e.ty(), SzType::Decimal);
    }
}
