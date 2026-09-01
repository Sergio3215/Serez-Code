//! The parser: source tokens in, a syntax tree out.
//!
//! This file is still the whole grammar. M1 is extracting it by responsibility
//! (see `docs/maturity/ROADMAP_STATE.md` §9); what has moved out so far lives
//! in the modules declared below, each extending `Parser` through its own
//! `impl` block, the way `src/evaluator/` already does.

mod branches;
mod classes;
mod cursor;
mod depth;
mod diagnostics;
mod directives;
mod functions;
mod literals;
use literals::{parse_dec_literal, parse_interpolated_string};
mod loops;
mod types;
mod variables;

use depth::DepthGuard;
use types::is_type_keyword;

// Re-exported so `serez_code::parser::MAX_PARSE_DEPTH` keeps meaning what it
// meant when the constant lived in this file: `tests/frontend_robustness.rs`
// and `tests/parser_facade.rs` both name that path.
//
// The `allow` is not cosmetic. `src/lsp_main.rs` declares `mod parser;` of its
// own, so this file is compiled a second time as part of the `sz-lsp` binary —
// where `parser` is a private module of a binary crate and the re-export really
// does reach nobody. One target needs it and the other cannot use it; see
// ROADMAP_STATE.md §5.18.
#[allow(unused_imports)]
pub use depth::{MAX_PARSE_DEPTH, SZ_PARSE_DEPTH_EXCEEDED};
#[allow(unused_imports)]
pub use diagnostics::{ParseError, SZ_PARSE_ERROR};

use crate::ast::*;
use crate::lexer::{LexError, Lexer};
use crate::token::{Token, TokenType};

/// ¿La expresión es una cadena de LECTURAS (`a`, `a.b`, `a[i]`, `a.b[i].c`)?
/// Sólo esas sirven como receptor de una asignación anidada: una llamada
/// (`a.dame().c`) produce un temporal, y escribirle no se ve desde ningún lado.
/// El evaluador vuelve a recorrer la misma forma para encontrar el slot.
fn is_writable_chain(e: &Expression) -> bool {
    match e {
        Expression::Identifier(_) => true,
        Expression::DotCall(d) if d.arguments.is_empty() && !d.has_parens => {
            is_writable_chain(&d.object)
        }
        Expression::Index(ix) => is_writable_chain(&ix.left),
        _ => false,
    }
}

#[derive(PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest,
    Pipe,         // |>
    Ternary,      // ? :
    NullCoalesce, // ??
    LogicalOr,    // ||
    LogicalAnd,   // &&
    BitOr,        // |
    BitXor,       // ^
    BitAnd,       // &
    Equals,       // ==
    LessGreater,  // > or <
    Shift,        // << >>
    Sum,          // +
    Product,      // *
    Power,        // **
    Prefix,       // -X or !X
    Call,         // myFunction(X)
    Index,        // array[index]
}

pub fn token_precedence(token_type: &TokenType) -> Precedence {
    match token_type {
        TokenType::Pipe => Precedence::Pipe,
        TokenType::Question => Precedence::Ternary,
        TokenType::NullCoalesce => Precedence::NullCoalesce,
        TokenType::Or => Precedence::LogicalOr,
        TokenType::And => Precedence::LogicalAnd,
        TokenType::BitOr => Precedence::BitOr,
        TokenType::BitXor => Precedence::BitXor,
        TokenType::BitAnd => Precedence::BitAnd,
        TokenType::Eq | TokenType::NotEq => Precedence::Equals,
        TokenType::KwIs => Precedence::LessGreater,
        TokenType::Lt | TokenType::Gt | TokenType::LtEq | TokenType::GtEq => {
            Precedence::LessGreater
        }
        TokenType::Shl | TokenType::Shr => Precedence::Shift,
        TokenType::Plus | TokenType::Minus => Precedence::Sum,
        TokenType::Slash | TokenType::Asterisk | TokenType::Percent => Precedence::Product,
        TokenType::Power => Precedence::Power,
        TokenType::LParen => Precedence::Call,
        TokenType::Dot | TokenType::QuestionDot => Precedence::Call,
        TokenType::LBracket => Precedence::Index,
        _ => Precedence::Lowest,
    }
}

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    peek_token: Token,
    source_lines: Vec<String>,
    /// File (or module) the source came from — prefixes error messages so
    /// errors inside imports are attributable to their file.
    source_name: Option<String>,
    /// Set whenever any parse error is reported. `Cell` so `parser_error(&self)`
    /// can flip it. Lets callers (main) fail with a non-zero exit code.
    had_error: std::cell::Cell<bool>,
    /// Every error reported via `parser_error`, in order. `RefCell` for the
    /// same reason as `had_error`.
    errors: std::cell::RefCell<Vec<ParseError>>,
    /// Diagnostics produced while the owned lexer advances. They are flushed
    /// into `errors` once parsing finishes, after source labels/lines are set.
    lexer_errors: std::cell::RefCell<Vec<LexError>>,
    /// Current recursive-descent nesting level. See [`MAX_PARSE_DEPTH`].
    depth: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Parser {
        let current_token = lexer.next_token();
        let peek_token = lexer.next_token();
        let lexer_errors = lexer.take_errors();
        Parser {
            lexer,
            current_token,
            peek_token,
            source_lines: Vec::new(),
            source_name: None,
            had_error: std::cell::Cell::new(false),
            errors: std::cell::RefCell::new(Vec::new()),
            lexer_errors: std::cell::RefCell::new(lexer_errors),
            depth: std::rc::Rc::new(std::cell::Cell::new(0)),
        }
    }

    fn is_reserved_name(&self, name: &str) -> bool {
        matches!(
            name,
            "Task" | "Time" | "DateTime" | "System" | "Gui" | "Dec" | "Media"
        )
    }

    pub fn parse_program(&mut self) -> Program {
        let mut program = Program {
            statements: Vec::new(),
        };

        while self.current_token.token_type != TokenType::Eof {
            // Stray ';' is an empty statement (e.g. after `return;` inside a
            // one-line block, or `unsafe { ... };`), not an error.
            if self.current_token.token_type == TokenType::Semicolon {
                self.next_token();
                continue;
            }
            match self.parse_statement() {
                Some(stmt) => program.statements.push(stmt),
                None => self.synchronize(),
            }
            self.next_token();
        }
        self.flush_lexer_errors();
        program
    }

    fn synchronize(&mut self) {
        while self.current_token.token_type != TokenType::Eof {
            match self.current_token.token_type {
                TokenType::Semicolon | TokenType::RBrace => return,
                TokenType::Let
                | TokenType::Return
                | TokenType::Out
                | TokenType::Function
                | TokenType::While
                | TokenType::For
                | TokenType::KwClass
                | TokenType::KwInterface
                | TokenType::KwPublic
                | TokenType::KwPrivate
                | TokenType::KwBreak
                | TokenType::KwContinue
                | TokenType::KwSwitch
                | TokenType::KwTry
                | TokenType::KwThrow
                | TokenType::KwConst
                | TokenType::KwEnum
                | TokenType::KwAbstract
                | TokenType::KwSealed
                | TokenType::KwDo => return,
                _ => self.next_token(),
            }
        }
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        // Nested blocks recurse through here without ever building an
        // expression, so statements need their own accounting.
        let _depth = self.enter_depth()?;
        match self.current_token.token_type {
            TokenType::Let | TokenType::KwConst => self.parse_let_statement(),
            TokenType::Return => self.parse_return_statement(),
            TokenType::Out => self.parse_out_statement(),
            TokenType::LBrace => self.parse_block_statement(),
            TokenType::Function => self.parse_function_statement(),
            TokenType::While => self.parse_while_statement(),
            TokenType::KwDo => self.parse_do_while_statement(),
            TokenType::For => self.parse_for_statement(),
            TokenType::KwBreak => {
                if self.peek_token.token_type == TokenType::Ident {
                    self.next_token(); // current = label name
                    let label = self.current_token.literal.clone();
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    Some(Statement::BreakLabel(label))
                } else {
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    Some(Statement::Break)
                }
            }
            TokenType::KwContinue => {
                if self.peek_token.token_type == TokenType::Ident {
                    self.next_token(); // current = label name
                    let label = self.current_token.literal.clone();
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    Some(Statement::ContinueLabel(label))
                } else {
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    Some(Statement::Continue)
                }
            }
            TokenType::KwEnum => self.parse_enum_declaration(),
            TokenType::KwClass => self.parse_class_declaration(true, false, false),
            TokenType::KwInterface => self.parse_interface_declaration(true),
            TokenType::KwPublic | TokenType::KwPrivate => self.parse_visibility_statement(),
            TokenType::KwAbstract => self.parse_abstract_or_sealed_class(true, false),
            TokenType::KwSealed => self.parse_abstract_or_sealed_class(false, true),
            TokenType::KwSwitch => self.parse_switch_statement(),
            TokenType::KwTry => self.parse_try_statement(),
            TokenType::KwThrow => self.parse_throw_statement(),
            TokenType::KwUnsafe => self.parse_unsafe_statement(),
            TokenType::KwNative => self.parse_native_declaration(),
            TokenType::KwImport => self.parse_import_statement(),
            TokenType::KwExport => self.parse_export_statement(),
            TokenType::KwUse => self.parse_use_permissions(),
            TokenType::KwYield => {
                self.next_token(); // consume 'yield', current = first token of expr
                let expr = self.parse_expression(Precedence::Lowest)?;
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Yield(expr))
            }
            // Labeled loop: label: while/for { ... }
            TokenType::Ident if self.peek_token.token_type == TokenType::Colon => {
                self.parse_labeled_statement()
            }
            TokenType::Ident if self.peek_token.token_type == TokenType::Assign => {
                self.parse_assign_statement()
            }
            TokenType::Ident if self.is_compound_assign(&self.peek_token.token_type) => {
                self.parse_compound_assign_statement()
            }
            TokenType::Ident if self.peek_token.token_type == TokenType::LBracket => {
                self.parse_index_assign_or_expr_statement()
            }
            // Postfix: i++  →  i = i + 1
            TokenType::Ident if self.peek_token.token_type == TokenType::PlusPlus => {
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                self.next_token(); // '++'
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "+".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line,
                        column: col,
                    }),
                }))
            }
            // Postfix: i--  →  i = i - 1
            TokenType::Ident if self.peek_token.token_type == TokenType::MinusMinus => {
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                self.next_token(); // '--'
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "-".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line,
                        column: col,
                    }),
                }))
            }
            // Prefix: ++i  →  i = i + 1
            TokenType::PlusPlus => {
                self.next_token(); // current = identifier
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "+".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line,
                        column: col,
                    }),
                }))
            }
            // Prefix: --i  →  i = i - 1
            TokenType::MinusMinus => {
                self.next_token(); // current = identifier
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "-".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line,
                        column: col,
                    }),
                }))
            }
            _ => self.parse_expression_statement(),
        }
    }

    fn is_compound_assign(&self, tt: &TokenType) -> bool {
        matches!(
            tt,
            TokenType::PlusEq
                | TokenType::MinusEq
                | TokenType::StarEq
                | TokenType::SlashEq
                | TokenType::PercentEq
        )
    }

    fn compound_op(tt: &TokenType) -> &'static str {
        match tt {
            TokenType::PlusEq => "+",
            TokenType::MinusEq => "-",
            TokenType::StarEq => "*",
            TokenType::SlashEq => "/",
            TokenType::PercentEq => "%",
            _ => unreachable!(),
        }
    }

    /// Desugar `x += rhs` → `x = x + rhs`
    fn parse_compound_assign_statement(&mut self) -> Option<Statement> {
        let name = self.current_token.literal.clone();
        let line = self.current_token.line;
        let column = self.current_token.column;
        let op = Self::compound_op(&self.peek_token.token_type).to_string();
        self.next_token(); // compound token
        self.next_token(); // first token of rhs
        let rhs = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        let value = Expression::Infix(InfixExpression {
            left: Box::new(Expression::Identifier(name.clone())),
            operator: op,
            right: Box::new(rhs),
            line,
            column,
        });
        Some(Statement::Assign(AssignStatement { name, value }))
    }

    fn parse_block_statement(&mut self) -> Option<Statement> {
        self.next_token();
        let mut statements = Vec::new();

        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            // Stray ';' is an empty statement, not an error.
            if self.current_token.token_type == TokenType::Semicolon {
                self.next_token();
                continue;
            }
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            self.next_token();
        }

        Some(Statement::Block(BlockStatement { statements }))
    }

    fn parse_sizeof_expression(&mut self) -> Option<Expression> {
        use crate::ast::SizeOfTarget;
        if self.peek_token.token_type != TokenType::LParen {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected '(' after 'sizeof'");
            return None;
        }
        self.next_token(); // consume '('
        self.next_token(); // move to the argument

        let type_names = [
            "int", "decimal", "dec", "bool", "string", "null", "void", "any",
        ];
        let target = if matches!(
            self.current_token.token_type,
            TokenType::KwInt
                | TokenType::KwDecimal
                | TokenType::KwDec
                | TokenType::KwBool
                | TokenType::KwString
                | TokenType::KwNull
                | TokenType::KwVoid
                | TokenType::KwAny
        ) {
            let name = self.current_token.literal.clone();
            self.next_token(); // consume type keyword
            SizeOfTarget::Type(name)
        } else if self.current_token.token_type == TokenType::Ident
            && type_names.contains(&self.current_token.literal.as_str())
        {
            let name = self.current_token.literal.clone();
            self.next_token();
            SizeOfTarget::Type(name)
        } else {
            let expr = self.parse_expression(Precedence::Lowest)?;
            SizeOfTarget::Expr(Box::new(expr))
        };

        if self.current_token.token_type != TokenType::RParen {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected ')' to close sizeof");
            return None;
        }
        Some(Expression::SizeOf(target))
    }

    fn parse_unsafe_statement(&mut self) -> Option<Statement> {
        if self.peek_token.token_type != TokenType::LBrace {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected '{{' after 'unsafe'");
            return None;
        }
        self.next_token(); // current = '{'
        self.next_token(); // skip '{'
        let mut statements = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            // Stray ';' is an empty statement, not an error.
            if self.current_token.token_type == TokenType::Semicolon {
                self.next_token();
                continue;
            }
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            self.next_token();
        }
        Some(Statement::Unsafe(BlockStatement { statements }))
    }

    fn parse_expression_statement(&mut self) -> Option<Statement> {
        let expr = self.parse_expression(Precedence::Lowest)?;

        let is_assign = self.peek_token.token_type == TokenType::Assign;
        let is_compound = self.is_compound_assign(&self.peek_token.token_type);

        if is_assign || is_compound {
            // *ptr = val
            if is_assign {
                if let Expression::Deref(ref ptr_expr) = expr {
                    let ptr_clone = ptr_expr.clone();
                    self.next_token(); // consume '='
                    self.next_token(); // first token of rhs
                    let value = self.parse_expression(Precedence::Lowest)?;
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    return Some(Statement::DerefAssign {
                        ptr: ptr_clone,
                        value,
                    });
                }
            }

            // obj.field = val  or  obj.field += val
            if let Expression::DotCall(ref dot) = expr {
                if dot.arguments.is_empty() {
                    if let Expression::Identifier(ref obj_name) = *dot.object {
                        let object = obj_name.clone();
                        let field = dot.method.clone();
                        let line = dot.line;
                        let column = dot.column;
                        let op_str = if is_compound {
                            Some(Self::compound_op(&self.peek_token.token_type).to_string())
                        } else {
                            None
                        };
                        self.next_token(); // '=' or compound token
                        self.next_token(); // first token of rhs
                        let rhs = self.parse_expression(Precedence::Lowest)?;
                        let value = if let Some(op) = op_str {
                            Expression::Infix(InfixExpression {
                                left: Box::new(Expression::DotCall(DotCallExpression {
                                    object: Box::new(Expression::Identifier(object.clone())),
                                    method: field.clone(),
                                    arguments: vec![],
                                    has_parens: false,
                                    is_optional: false,
                                    line,
                                    column,
                                })),
                                operator: op,
                                right: Box::new(rhs),
                                line,
                                column,
                            })
                        } else {
                            rhs
                        };
                        if self.peek_token.token_type == TokenType::Semicolon {
                            self.next_token();
                        }
                        return Some(Statement::FieldAssign(FieldAssignStatement {
                            object,
                            field,
                            value,
                        }));
                    }

                    if let Some(st) = self.try_build_nested_field_assign(dot, is_compound) {
                        return Some(st);
                    }
                }
            }

            // expr[idx] = val  or  expr[idx] += val
            if let Expression::Index(_) = &expr {
                if is_assign {
                    return self.try_build_index_assign(expr);
                } else {
                    return self.try_build_index_compound_assign(expr);
                }
            }
        }

        // obj.field++  /  this.field++  /  obj.field--  /  this.field--
        // Also catches Index targets arriving via expression_statement path (e.g. this.arr[i]++)
        let is_incr = self.peek_token.token_type == TokenType::PlusPlus;
        let is_decr = self.peek_token.token_type == TokenType::MinusMinus;
        if is_incr || is_decr {
            let op = if is_incr { "+" } else { "-" };
            let line = self.current_token.line;
            let column = self.current_token.column;

            if let Expression::DotCall(ref dot) = expr {
                if dot.arguments.is_empty() {
                    if let Expression::Identifier(ref obj_name) = *dot.object {
                        let object = obj_name.clone();
                        let field = dot.method.clone();
                        let dline = dot.line;
                        let dcol = dot.column;
                        self.next_token(); // ++ or --
                        if self.peek_token.token_type == TokenType::Semicolon {
                            self.next_token();
                        }
                        let value = Expression::Infix(InfixExpression {
                            left: Box::new(Expression::DotCall(DotCallExpression {
                                object: Box::new(Expression::Identifier(object.clone())),
                                method: field.clone(),
                                arguments: vec![],
                                has_parens: false,
                                is_optional: false,
                                line: dline,
                                column: dcol,
                            })),
                            operator: op.to_string(),
                            right: Box::new(Expression::Integer(1)),
                            line,
                            column,
                        });
                        return Some(Statement::FieldAssign(FieldAssignStatement {
                            object,
                            field,
                            value,
                        }));
                    }
                }
            }

            if let Expression::Index(ref idx_expr) = expr {
                let target = (*idx_expr.left).clone();
                let index = (*idx_expr.index).clone();
                self.next_token(); // ++ or --
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                let value = Expression::Infix(InfixExpression {
                    left: Box::new(expr.clone()),
                    operator: op.to_string(),
                    right: Box::new(Expression::Integer(1)),
                    line,
                    column,
                });
                return Some(Statement::IndexAssign(IndexAssignStatement {
                    target,
                    index,
                    value,
                }));
            }
        }

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Expression(expr))
    }

    fn parse_assign_statement(&mut self) -> Option<Statement> {
        let name = self.current_token.literal.clone();
        self.next_token(); // '='
        self.next_token(); // first token of value

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Assign(AssignStatement { name, value }))
    }

    fn parse_return_statement(&mut self) -> Option<Statement> {
        // Bare `return` followed by `}`, `;`, or EOF — return null without consuming the delimiter
        if matches!(
            self.peek_token.token_type,
            TokenType::Semicolon | TokenType::RBrace | TokenType::Eof
        ) {
            return Some(Statement::Return(ReturnStatement {
                return_value: Expression::Null,
            }));
        }

        self.next_token();

        // Bare `return;` — no expression, return null
        if self.current_token.token_type == TokenType::Semicolon {
            return Some(Statement::Return(ReturnStatement {
                return_value: Expression::Null,
            }));
        }

        let return_value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Return(ReturnStatement { return_value }))
    }

    fn parse_out_statement(&mut self) -> Option<Statement> {
        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Out(OutStatement { value }))
    }

    fn parse_index_assign_or_expr_statement(&mut self) -> Option<Statement> {
        let expr = self.parse_expression(Precedence::Lowest)?;
        if self.is_compound_assign(&self.peek_token.token_type) {
            return self.try_build_index_compound_assign(expr);
        }
        // arr[i]++  /  arr[i]--
        if matches!(
            self.peek_token.token_type,
            TokenType::PlusPlus | TokenType::MinusMinus
        ) {
            if let Expression::Index(ref idx_expr) = expr {
                let target = (*idx_expr.left).clone();
                let index = (*idx_expr.index).clone();
                let line = self.current_token.line;
                let column = self.current_token.column;
                let op = if self.peek_token.token_type == TokenType::PlusPlus {
                    "+"
                } else {
                    "-"
                };
                self.next_token(); // ++ or --
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                let value = Expression::Infix(InfixExpression {
                    left: Box::new(expr),
                    operator: op.to_string(),
                    right: Box::new(Expression::Integer(1)),
                    line,
                    column,
                });
                return Some(Statement::IndexAssign(IndexAssignStatement {
                    target,
                    index,
                    value,
                }));
            }
        }
        self.try_build_index_assign(expr)
    }

    fn try_build_index_assign(&mut self, expr: Expression) -> Option<Statement> {
        let is_assign = self.peek_token.token_type == TokenType::Assign;
        let is_compound = self.is_compound_assign(&self.peek_token.token_type);
        if is_assign {
            if let Expression::Index(idx_expr) = &expr {
                let target = (*idx_expr.left).clone();
                let index = (*idx_expr.index).clone();
                self.next_token(); // '='
                self.next_token(); // first token of value
                let value = self.parse_expression(Precedence::Lowest)?;
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                return Some(Statement::IndexAssign(IndexAssignStatement {
                    target,
                    index,
                    value,
                }));
            }
        }
        // `objs[1].campo = x` entra por acá (la sentencia arranca con `ident[`,
        // no por parse_expression_statement) y termina siendo un DotCall, no un
        // Index. Sin esto el '=' quedaba sin consumir y era un error de parseo.
        if is_assign || is_compound {
            if let Expression::DotCall(ref dot) = expr {
                if let Some(st) = self.try_build_nested_field_assign(dot, is_compound) {
                    return Some(st);
                }
            }
        }
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::Expression(expr))
    }

    /// `a.b.c = val` (y su forma compuesta) — el receptor es una CADENA, no un
    /// nombre suelto, así que no entra en `FieldAssignStatement`. Antes moría en
    /// el parser con "Unexpected token '='" y había que rearmar y reasignar el
    /// objeto intermedio entero.
    ///
    /// Sólo una cadena de lecturas es destino válido: `a.dame().c = x` escribe
    /// sobre un temporal y no se vería desde ningún lado. Devuelve `None` sin
    /// consumir nada si la forma no aplica, para que el llamador siga probando.
    fn try_build_nested_field_assign(
        &mut self,
        dot: &DotCallExpression,
        is_compound: bool,
    ) -> Option<Statement> {
        if dot.has_parens || !dot.arguments.is_empty() {
            return None;
        }
        if !is_writable_chain(&dot.object) {
            return None;
        }
        // Un solo salto sobre una variable ya lo cubre FieldAssign.
        if matches!(*dot.object, Expression::Identifier(_)) {
            return None;
        }

        let object = (*dot.object).clone();
        let field = dot.method.clone();
        let line = dot.line;
        let column = dot.column;
        let op_str = if is_compound {
            Some(Self::compound_op(&self.peek_token.token_type).to_string())
        } else {
            None
        };
        self.next_token(); // '=' or compound token
        self.next_token(); // first token of rhs
        let rhs = self.parse_expression(Precedence::Lowest)?;
        let value = if let Some(op) = op_str {
            Expression::Infix(InfixExpression {
                left: Box::new(Expression::DotCall(DotCallExpression {
                    object: Box::new(object.clone()),
                    method: field.clone(),
                    arguments: vec![],
                    has_parens: false,
                    is_optional: false,
                    line,
                    column,
                })),
                operator: op,
                right: Box::new(rhs),
                line,
                column,
            })
        } else {
            rhs
        };
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::NestedFieldAssign(NestedFieldAssignStatement {
            object,
            field,
            value,
        }))
    }

    /// Desugar `arr[i] += rhs` → `arr[i] = arr[i] + rhs`
    fn try_build_index_compound_assign(&mut self, expr: Expression) -> Option<Statement> {
        if let Expression::Index(ref idx_expr) = expr {
            let target = (*idx_expr.left).clone();
            let index = (*idx_expr.index).clone();
            let line = self.current_token.line;
            let column = self.current_token.column;
            let op = Self::compound_op(&self.peek_token.token_type).to_string();
            self.next_token(); // compound token
            self.next_token(); // first token of rhs
            let rhs = self.parse_expression(Precedence::Lowest)?;
            let value = Expression::Infix(InfixExpression {
                left: Box::new(expr.clone()),
                operator: op,
                right: Box::new(rhs),
                line,
                column,
            });
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }
            return Some(Statement::IndexAssign(IndexAssignStatement {
                target,
                index,
                value,
            }));
        }
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::Expression(expr))
    }

    fn parse_call_arguments(&mut self) -> Option<Vec<Expression>> {
        let mut args = Vec::new();

        if self.peek_token.token_type == TokenType::RParen {
            self.next_token();
            return Some(args);
        }

        self.next_token();

        // Handle spread in first argument position
        if self.current_token.token_type == TokenType::DotDotDot {
            self.next_token();
            let inner = self.parse_expression(Precedence::Lowest)?;
            args.push(Expression::Spread(Box::new(inner)));
        } else {
            args.push(self.parse_expression(Precedence::Lowest)?);
        }

        while self.peek_token.token_type == TokenType::Comma {
            self.next_token();
            self.next_token();
            if self.current_token.token_type == TokenType::DotDotDot {
                self.next_token();
                let inner = self.parse_expression(Precedence::Lowest)?;
                args.push(Expression::Spread(Box::new(inner)));
            } else {
                args.push(self.parse_expression(Precedence::Lowest)?);
            }
        }

        if self.peek_token.token_type != TokenType::RParen {
            return None;
        }
        self.next_token();

        Some(args)
    }

    // ── Lambda parsing ────────────────────────────────────────────────────────

    // ── Expression parsing ────────────────────────────────────────────────────

    /// Continues the infix chain starting from an already-parsed left expression.
    /// Used by both parse_expression and the lambda fallback grouped-expr case.
    fn parse_infix_chain(
        &mut self,
        mut left_exp: Option<Expression>,
        precedence: Precedence,
    ) -> Option<Expression> {
        // Every iteration appends one level to the left spine of the tree being
        // built. The guard holds all of them until the chain is finished, so
        // the ceiling bounds the tree's depth and not merely the parser's own
        // recursion. See `charge_depth`.
        let mut chain = DepthGuard::empty(&self.depth);
        while self.peek_token.token_type != TokenType::Semicolon
            && precedence < self.peek_precedence()
        {
            let is_infix = match self.peek_token.token_type {
                TokenType::Plus
                | TokenType::Minus
                | TokenType::Slash
                | TokenType::Asterisk
                | TokenType::Percent
                | TokenType::Eq
                | TokenType::NotEq
                | TokenType::Lt
                | TokenType::Gt
                | TokenType::LtEq
                | TokenType::GtEq
                | TokenType::And
                | TokenType::Or
                | TokenType::NullCoalesce
                | TokenType::Question
                | TokenType::LParen
                | TokenType::Dot
                | TokenType::QuestionDot
                | TokenType::LBracket
                | TokenType::Power
                | TokenType::BitAnd
                | TokenType::BitOr
                | TokenType::BitXor
                | TokenType::Shl
                | TokenType::Shr
                | TokenType::KwIs
                | TokenType::Pipe => true,
                _ => false,
            };

            if !is_infix {
                return left_exp;
            }

            self.charge_depth(&mut chain)?;

            self.next_token();

            let operator = self.current_token.literal.clone();
            let current_precedence = self.current_precedence();

            if self.current_token.token_type == TokenType::LParen {
                if let Some(left) = left_exp {
                    let call_line = self.current_token.line;
                    let call_column = self.current_token.column;

                    if let Some(args) = self.parse_call_arguments() {
                        left_exp = Some(Expression::Call(CallExpression {
                            function: Box::new(left),
                            arguments: args,
                            line: call_line,
                            column: call_column,
                        }));
                    } else {
                        return None;
                    }
                }
            } else if self.current_token.token_type == TokenType::LBracket {
                if let Some(left) = left_exp {
                    self.next_token();
                    if let Some(index) = self.parse_expression(Precedence::Lowest) {
                        if self.peek_token.token_type != TokenType::RBracket {
                            self.parser_error("Expected ']' after array index");
                            return None;
                        }
                        self.next_token();
                        left_exp = Some(Expression::Index(IndexExpression {
                            left: Box::new(left),
                            index: Box::new(index),
                        }));
                    } else {
                        return None;
                    }
                }
            } else if self.current_token.token_type == TokenType::Question {
                // Ternary: condition ? then_expr : else_expr
                if let Some(condition) = left_exp {
                    self.next_token(); // first token of then_expr
                    let then_expr = match self.parse_expression(Precedence::Lowest) {
                        Some(e) => e,
                        None => return None,
                    };
                    if self.peek_token.token_type != TokenType::Colon {
                        self.parser_error("Expected ':' in ternary expression after '?'");
                        return None;
                    }
                    self.next_token(); // ':'
                    self.next_token(); // first token of else_expr
                    let else_expr = match self.parse_expression(Precedence::Lowest) {
                        Some(e) => e,
                        None => return None,
                    };
                    left_exp = Some(Expression::Ternary(TernaryExpression {
                        condition: Box::new(condition),
                        then_expr: Box::new(then_expr),
                        else_expr: Box::new(else_expr),
                    }));
                }
            } else if self.current_token.token_type == TokenType::KwIs {
                // `expr is TypeName` → Infix("is", expr, Identifier("type_name"))
                let op_line = self.current_token.line;
                let op_column = self.current_token.column;
                self.next_token(); // consume type name token (KwInt, KwString, Ident, etc.)
                let type_name = self.current_token.literal.clone();
                if let Some(left) = left_exp {
                    left_exp = Some(Expression::Infix(InfixExpression {
                        left: Box::new(left),
                        operator: "is".to_string(),
                        right: Box::new(Expression::Identifier(type_name)),
                        line: op_line,
                        column: op_column,
                    }));
                }
            } else if self.current_token.token_type == TokenType::Dot
                || self.current_token.token_type == TokenType::QuestionDot
            {
                let is_optional = self.current_token.token_type == TokenType::QuestionDot;
                let dot_line = self.current_token.line;
                let dot_column = self.current_token.column;

                // After '.', accept identifiers AND keyword tokens as method names
                // (e.g. tensor.get(), dict.set(), obj.new() should work)
                if !self.peek_token_is_name() {
                    self.parser_error("Expected method name after '.'");
                    return left_exp;
                }
                self.next_token();
                let method = self.current_token.literal.clone();

                let has_parens = self.peek_token.token_type == TokenType::LParen;
                let arguments = if has_parens {
                    self.next_token();
                    self.parse_call_arguments().unwrap_or_default()
                } else {
                    Vec::new()
                };

                if let Some(left) = left_exp {
                    left_exp = Some(Expression::DotCall(DotCallExpression {
                        object: Box::new(left),
                        method,
                        arguments,
                        has_parens,
                        is_optional,
                        line: dot_line,
                        column: dot_column,
                    }));
                }
            } else if self.current_token.token_type == TokenType::Pipe {
                // |> desugars: left |> fn  →  fn(left)
                let call_line = self.current_token.line;
                let call_column = self.current_token.column;
                self.next_token(); // advance to the function expression
                if let Some(left) = left_exp {
                    if let Some(func) = self.parse_expression(current_precedence) {
                        left_exp = Some(Expression::Call(CallExpression {
                            function: Box::new(func),
                            arguments: vec![left],
                            line: call_line,
                            column: call_column,
                        }));
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                let op_line = self.current_token.line;
                let op_column = self.current_token.column;

                self.next_token();

                // `**` is right-associative (2 ** 3 ** 2 == 2 ** (3 ** 2)), matching
                // math/Python. Parse its right operand one level below Power so a
                // following `**` binds into the right side. All other operators stay
                // left-associative.
                let right_precedence = if current_precedence == Precedence::Power {
                    Precedence::Product
                } else {
                    current_precedence
                };

                if let Some(left) = left_exp {
                    if let Some(right) = self.parse_expression(right_precedence) {
                        left_exp = Some(Expression::Infix(InfixExpression {
                            left: Box::new(left),
                            operator,
                            right: Box::new(right),
                            line: op_line,
                            column: op_column,
                        }));
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        }

        left_exp
    }

    fn parse_expression(&mut self, precedence: Precedence) -> Option<Expression> {
        // Every nested sub-expression re-enters here, so this is the one place
        // that has to hold the line against runaway nesting. See MAX_PARSE_DEPTH.
        let _depth = self.enter_depth()?;
        // ── PREFIX ────────────────────────────────────────────────────────────
        let left_exp = match self.current_token.token_type {
            // Single-param lambda: item => body
            TokenType::Ident if self.peek_token.token_type == TokenType::Arrow => {
                let param = self.current_token.literal.clone();
                self.next_token(); // consume '=>'
                let body = self.parse_lambda_body()?;
                Some(Expression::Lambda(LambdaExpression {
                    params: vec![param],
                    body,
                }))
            }

            TokenType::Ident => Some(Expression::Identifier(self.current_token.literal.clone())),

            TokenType::Int => {
                if let Ok(num) = self.current_token.literal.parse::<i64>() {
                    Some(Expression::Integer(num))
                } else {
                    self.parser_error(&format!(
                        "Invalid integer literal '{}' (out of 64-bit range?)",
                        self.current_token.literal
                    ));
                    None
                }
            }

            TokenType::Decimal => {
                if let Ok(num) = self.current_token.literal.parse::<f64>() {
                    Some(Expression::Decimal(num))
                } else {
                    self.parser_error(&format!(
                        "Invalid decimal literal '{}'",
                        self.current_token.literal
                    ));
                    None
                }
            }

            TokenType::Dec => match parse_dec_literal(&self.current_token.literal) {
                Some(d) => Some(Expression::Dec(d)),
                None => {
                    self.parser_error(&format!(
                        "Invalid dec literal '{}'",
                        self.current_token.literal
                    ));
                    None
                }
            },

            TokenType::String => {
                let s = self.current_token.literal.clone();
                if s.contains('{') {
                    let parsed = parse_interpolated_string(&s, self.source_name.as_deref());
                    if parsed.is_none() {
                        self.had_error.set(true);
                    }
                    parsed
                } else {
                    // Replace \{ sentinel (\x01) with literal { in non-interpolated strings
                    Some(Expression::String(s.replace('\x01', "{")))
                }
            }

            // Raw string r"..." — already literal (braces not interpolated).
            TokenType::RawString => Some(Expression::String(self.current_token.literal.clone())),

            TokenType::True => Some(Expression::Boolean(true)),
            TokenType::False => Some(Expression::Boolean(false)),
            TokenType::KwNull => Some(Expression::Null),

            TokenType::Bang | TokenType::Minus | TokenType::BitNot => {
                let operator = self.current_token.literal.clone();
                self.next_token();
                let right = self.parse_expression(Precedence::Prefix)?;
                Some(Expression::Prefix(operator, Box::new(right)))
            }

            // &varname — address-of
            TokenType::BitAnd => {
                self.next_token();
                let inner = self.parse_expression(Precedence::Prefix)?;
                Some(Expression::AddressOf(Box::new(inner)))
            }

            // *ptr — dereference
            TokenType::Asterisk => {
                self.next_token();
                let inner = self.parse_expression(Precedence::Prefix)?;
                Some(Expression::Deref(Box::new(inner)))
            }

            // sizeof(type | expr)
            TokenType::KwSizeof => self.parse_sizeof_expression(),

            // Zero-param lambda: () => body
            TokenType::LParen if self.peek_token.token_type == TokenType::RParen => {
                self.next_token(); // consume ')'
                if self.peek_token.token_type == TokenType::Arrow {
                    self.next_token(); // consume '=>'
                    let body = self.parse_lambda_body()?;
                    Some(Expression::Lambda(LambdaExpression {
                        params: vec![],
                        body,
                    }))
                } else {
                    self.parser_error("Empty parentheses '()' are not a valid expression");
                    None
                }
            }

            // Multi-param lambda: (a, b) => body  /  (a) => body  /  (expr)
            TokenType::LParen if self.peek_token.token_type == TokenType::Ident => {
                self.next_token(); // consume '(' → current = first ident
                let first_name = self.current_token.literal.clone();

                match self.peek_token.token_type {
                    // (a, b, ...) => body
                    TokenType::Comma => {
                        let mut params = vec![first_name];
                        while self.peek_token.token_type == TokenType::Comma {
                            self.next_token(); // ','
                            self.next_token(); // next ident
                            if self.current_token.token_type != TokenType::Ident {
                                self.parser_error("Expected identifier in lambda parameters");
                                return None;
                            }
                            params.push(self.current_token.literal.clone());
                        }
                        if self.peek_token.token_type != TokenType::RParen {
                            self.parser_error("Expected ')' after lambda parameters");
                            return None;
                        }
                        self.next_token(); // ')'
                        if self.peek_token.token_type != TokenType::Arrow {
                            self.parser_error("Expected '=>' after lambda parameters");
                            return None;
                        }
                        self.next_token(); // '=>'
                        let body = self.parse_lambda_body()?;
                        Some(Expression::Lambda(LambdaExpression { params, body }))
                    }

                    // (a) => body  or  just (a)
                    TokenType::RParen => {
                        self.next_token(); // ')'
                        if self.peek_token.token_type == TokenType::Arrow {
                            self.next_token(); // '=>'
                            let body = self.parse_lambda_body()?;
                            Some(Expression::Lambda(LambdaExpression {
                                params: vec![first_name],
                                body,
                            }))
                        } else {
                            Some(Expression::Identifier(first_name))
                        }
                    }

                    // (x => body) — single-param lambda wrapped in parentheses.
                    // (x) => body is handled by the RParen arm; this is the case
                    // where the param has no inner parens: ( x => ... ). (B-84)
                    TokenType::Arrow => {
                        self.next_token(); // current = '=>'
                        let body = self.parse_lambda_body()?;
                        if self.peek_token.token_type != TokenType::RParen {
                            self.parser_error("Expected ')' after parenthesized lambda");
                            return None;
                        }
                        self.next_token(); // ')'
                        Some(Expression::Lambda(LambdaExpression {
                            params: vec![first_name],
                            body,
                        }))
                    }

                    // (ident op ...) — grouped expression starting with an identifier
                    _ => {
                        let first = Some(Expression::Identifier(first_name));
                        let inner = self.parse_infix_chain(first, Precedence::Lowest)?;
                        if self.peek_token.token_type != TokenType::RParen {
                            self.parser_error("Expected ')' in grouped expression");
                            return None;
                        }
                        self.next_token(); // ')'
                        Some(inner)
                    }
                }
            }

            // Regular grouped expression: (expr)
            TokenType::LParen => {
                self.next_token();
                let exp = self.parse_expression(Precedence::Lowest);
                if self.peek_token.token_type != TokenType::RParen {
                    return None;
                }
                self.next_token();
                exp
            }

            TokenType::LBracket => self.parse_array_literal(),
            TokenType::LBrace => self.parse_brace_expression(),
            TokenType::If => self.parse_if_expression(),
            TokenType::KwNew => self.parse_new_expression(),

            TokenType::KwVoid
            | TokenType::KwInt
            | TokenType::KwDecimal
            | TokenType::KwDec
            | TokenType::KwString
            | TokenType::KwBool
            | TokenType::KwAny => self.parse_arrow_function(),

            TokenType::Function => {
                let mut return_type = None;
                if is_type_keyword(&self.peek_token.token_type) {
                    self.next_token();
                    return_type = Some(self.current_token.literal.clone());
                }

                if self.peek_token.token_type != TokenType::LParen {
                    return None;
                }
                self.next_token();

                let parameters = self.parse_function_parameters()?;

                if self.peek_token.token_type != TokenType::LBrace {
                    return None;
                }
                self.next_token();

                let body_stmt = self.parse_block_statement()?;
                let body = match body_stmt {
                    Statement::Block(b) => b,
                    _ => return None,
                };

                Some(Expression::FunctionLiteral(FunctionLiteral {
                    return_type,
                    parameters,
                    body,
                    is_generator: false,
                }))
            }

            TokenType::KwMatch => self.parse_match_expression(),

            // unsafe { ... } as an expression (returns last value of block)
            TokenType::KwUnsafe => {
                self.next_token(); // consume 'unsafe'
                if self.current_token.token_type != TokenType::LBrace {
                    self.had_error.set(true);
                    eprintln!("❌ PARSE ERROR: expected '{{' after 'unsafe'");
                    return None;
                }
                let block_stmt = self.parse_block_statement()?;
                let block = match block_stmt {
                    Statement::Block(b) => b,
                    _ => return None,
                };
                Some(Expression::UnsafeBlock(block))
            }

            TokenType::Illegal => None, // the lexer already emitted an SZ1xxx diagnostic

            _ => {
                match self.current_token.token_type {
                    TokenType::Eof => {
                        self.parser_error("Unexpected end of file: expected an expression")
                    }
                    TokenType::Semicolon => self.parser_error("Expected an expression before ';'"),
                    _ => self.parser_error(&format!(
                        "Unexpected token '{}': expected an expression",
                        self.current_token.literal
                    )),
                }
                None
            }
        };

        // ── INFIX ─────────────────────────────────────────────────────────────
        self.parse_infix_chain(left_exp, precedence)
    }

    // ── new expression ────────────────────────────────────────────────────────
    fn parse_new_expression(&mut self) -> Option<Expression> {
        // current = 'new'
        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected class name after 'new'");
            return None;
        }
        self.next_token();
        let class_name = self.current_token.literal.clone();

        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after class name in 'new'");
            return None;
        }
        self.next_token(); // '('

        // Distinguish interface { field: val } from positional args
        if self.peek_token.token_type == TokenType::LBrace {
            self.next_token(); // '{'
            self.next_token(); // first field name or '}'

            let mut fields: Vec<(String, Expression)> = Vec::new();
            while self.current_token.token_type != TokenType::RBrace
                && self.current_token.token_type != TokenType::Eof
            {
                if self.current_token.token_type != TokenType::Ident {
                    self.parser_error("Expected field name in 'new' interface literal");
                    return None;
                }
                let field_name = self.current_token.literal.clone();
                if self.peek_token.token_type != TokenType::Colon {
                    self.parser_error("Expected ':' after field name in 'new'");
                    return None;
                }
                self.next_token(); // ':'
                self.next_token(); // value
                let value = self.parse_expression(Precedence::Lowest)?;
                fields.push((field_name, value));

                match self.peek_token.token_type {
                    TokenType::Comma => {
                        self.next_token(); // ','
                        if self.peek_token.token_type == TokenType::RBrace {
                            self.next_token(); // '}'
                            break;
                        }
                        self.next_token(); // next field
                    }
                    TokenType::RBrace => {
                        self.next_token(); // '}'
                        break;
                    }
                    _ => {
                        self.parser_error("Expected ',' or '}}' in interface fields");
                        return None;
                    }
                }
            }
            if self.peek_token.token_type != TokenType::RParen {
                self.parser_error("Expected ')' after '}}' in 'new'");
                return None;
            }
            self.next_token(); // ')'
            Some(Expression::New(NewExpression {
                class_name,
                args: NewArgs::Fields(fields),
            }))
        } else {
            let args = self.parse_call_arguments()?;
            Some(Expression::New(NewExpression {
                class_name,
                args: NewArgs::Positional(args),
            }))
        }
    }

    // ── match expr { pattern => body, ... } ──────────────────────────────────

    // ── throw expr; ───────────────────────────────────────────────────────────
    fn parse_throw_statement(&mut self) -> Option<Statement> {
        self.next_token(); // first token of expr
        let expr = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::Throw(expr))
    }
}
