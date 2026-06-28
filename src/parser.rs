use crate::ast::*;
use crate::lexer::Lexer;
use crate::token::{Token, TokenType};

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
        TokenType::Pipe         => Precedence::Pipe,
        TokenType::Question     => Precedence::Ternary,
        TokenType::NullCoalesce => Precedence::NullCoalesce,
        TokenType::Or => Precedence::LogicalOr,
        TokenType::And => Precedence::LogicalAnd,
        TokenType::BitOr => Precedence::BitOr,
        TokenType::BitXor => Precedence::BitXor,
        TokenType::BitAnd => Precedence::BitAnd,
        TokenType::Eq | TokenType::NotEq => Precedence::Equals,
        TokenType::KwIs => Precedence::LessGreater,
        TokenType::Lt | TokenType::Gt | TokenType::LtEq | TokenType::GtEq => Precedence::LessGreater,
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
    /// Set whenever any parse error is reported. `Cell` so `parser_error(&self)`
    /// can flip it. Lets callers (main) fail with a non-zero exit code.
    had_error: std::cell::Cell<bool>,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Parser {
        let current_token = lexer.next_token();
        let peek_token = lexer.next_token();
        Parser {
            lexer,
            current_token,
            peek_token,
            source_lines: Vec::new(),
            had_error: std::cell::Cell::new(false),
        }
    }

    fn is_reserved_name(&self, name: &str) -> bool {
        matches!(name, "Task" | "Time" | "DateTime" | "System" | "Gui" | "Dec")
    }

    /// Whether any parse error was reported while building the program.
    pub fn has_errors(&self) -> bool {
        self.had_error.get()
    }

    pub fn set_source(&mut self, lines: Vec<String>) {
        self.source_lines = lines;
    }

    fn parser_error(&self, msg: &str) {
        self.had_error.set(true);
        let line = self.current_token.line;
        let col  = self.current_token.column;
        eprintln!("❌ PARSER ERROR [line {}:{}]: {}", line, col, msg);
        if let Some(src) = self.source_lines.get(line.saturating_sub(1)) {
            let ln = line.to_string();
            eprintln!("  {} | {}", ln, src.trim_end());
            eprintln!("  {}   {}^", " ".repeat(ln.len()), " ".repeat(col.saturating_sub(1)));
        }
    }

    pub fn next_token(&mut self) {
        self.current_token = self.peek_token.clone();
        self.peek_token = self.lexer.next_token();
    }

    fn peek_precedence(&self) -> Precedence {
        token_precedence(&self.peek_token.token_type)
    }

    /// Returns true if the peek token is a valid method/field name (identifier or keyword).
    /// After '.', keywords like 'get', 'set', 'in', etc. are valid method names.
    fn peek_token_is_name(&self) -> bool {
        Self::token_type_is_name(&self.peek_token.token_type)
    }

    fn current_token_is_name(&self) -> bool {
        Self::token_type_is_name(&self.current_token.token_type)
    }

    fn token_type_is_name(tt: &TokenType) -> bool {
        !matches!(
            tt,
            TokenType::Illegal | TokenType::Eof
            | TokenType::Int | TokenType::Decimal | TokenType::String
            | TokenType::Assign | TokenType::Plus | TokenType::Minus | TokenType::Bang
            | TokenType::Asterisk | TokenType::Slash | TokenType::Percent
            | TokenType::Lt | TokenType::Gt | TokenType::LtEq | TokenType::GtEq
            | TokenType::Eq | TokenType::NotEq | TokenType::And | TokenType::Or
            | TokenType::Arrow | TokenType::NullCoalesce
            | TokenType::PlusEq | TokenType::MinusEq | TokenType::StarEq
            | TokenType::SlashEq | TokenType::PercentEq
            | TokenType::Comma | TokenType::Semicolon
            | TokenType::LParen | TokenType::RParen | TokenType::LBrace
            | TokenType::RBrace | TokenType::LBracket | TokenType::RBracket
            | TokenType::Dot | TokenType::Colon | TokenType::Question
            | TokenType::PlusPlus | TokenType::MinusMinus | TokenType::DotDotDot
            | TokenType::Power | TokenType::BitAnd | TokenType::BitOr | TokenType::BitXor
            | TokenType::BitNot | TokenType::Shl | TokenType::Shr | TokenType::QuestionDot
        )
    }

    fn current_precedence(&self) -> Precedence {
        token_precedence(&self.current_token.token_type)
    }

    pub fn parse_program(&mut self) -> Program {
        let mut program = Program {
            statements: Vec::new(),
        };

        while self.current_token.token_type != TokenType::Eof {
            match self.parse_statement() {
                Some(stmt) => program.statements.push(stmt),
                None => self.synchronize(),
            }
            self.next_token();
        }
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
                    if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
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
                    if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
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
                if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
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
                let col  = self.current_token.column;
                self.next_token(); // '++'
                if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "+".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line, column: col,
                    }),
                }))
            }
            // Postfix: i--  →  i = i - 1
            TokenType::Ident if self.peek_token.token_type == TokenType::MinusMinus => {
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col  = self.current_token.column;
                self.next_token(); // '--'
                if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "-".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line, column: col,
                    }),
                }))
            }
            // Prefix: ++i  →  i = i + 1
            TokenType::PlusPlus => {
                self.next_token(); // current = identifier
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col  = self.current_token.column;
                if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "+".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line, column: col,
                    }),
                }))
            }
            // Prefix: --i  →  i = i - 1
            TokenType::MinusMinus => {
                self.next_token(); // current = identifier
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col  = self.current_token.column;
                if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "-".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line, column: col,
                    }),
                }))
            }
            _ => self.parse_expression_statement(),
        }
    }

    fn is_compound_assign(&self, tt: &TokenType) -> bool {
        matches!(tt, TokenType::PlusEq | TokenType::MinusEq | TokenType::StarEq | TokenType::SlashEq | TokenType::PercentEq)
    }

    fn compound_op(tt: &TokenType) -> &'static str {
        match tt {
            TokenType::PlusEq    => "+",
            TokenType::MinusEq   => "-",
            TokenType::StarEq    => "*",
            TokenType::SlashEq   => "/",
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
        if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
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
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            self.next_token();
        }

        Some(Statement::Block(BlockStatement { statements }))
    }

    fn parse_native_declaration(&mut self) -> Option<Statement> {
        use crate::ast::NativeFnDeclaration;
        // native fn [return_type] name(params);
        if self.peek_token.token_type != TokenType::Function {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected 'fn' after 'native'");
            return None;
        }
        self.next_token(); // consume 'fn'

        // optional return type
        let mut return_type = None;
        if is_type_keyword(&self.peek_token.token_type) {
            self.next_token();
            return_type = self.parse_type_string();
        }

        // Disambiguate: native fn ClassName funcName  vs  native fn funcName
        if self.peek_token.token_type != TokenType::Ident {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected function name after 'native fn'");
            return None;
        }
        self.next_token();
        let first = self.current_token.literal.clone();
        let name = if self.peek_token.token_type == TokenType::Ident {
            return_type = Some(first);
            self.next_token();
            self.current_token.literal.clone()
        } else {
            first
        };

        if self.peek_token.token_type != TokenType::LParen {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected '(' after native function name");
            return None;
        }
        self.next_token();
        let parameters = self.parse_function_parameters()?;

        // allow trailing {} (empty body) or just ;
        if self.peek_token.token_type == TokenType::LBrace {
            self.next_token();
            self.next_token(); // skip '{'
            // consume until '}'
            while self.current_token.token_type != TokenType::RBrace
                && self.current_token.token_type != TokenType::Eof
            {
                self.next_token();
            }
        } else if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::NativeDeclaration(NativeFnDeclaration { name, return_type, parameters }))
    }

    fn parse_export_statement(&mut self) -> Option<Statement> {
        // export <declaration>  —  wraps any top-level declaration
        self.next_token(); // consume 'export', move to the inner keyword
        let inner = self.parse_statement()?;
        Some(Statement::Export(Box::new(inner)))
    }

    fn parse_use_permissions(&mut self) -> Option<Statement> {
        // use permissions { Terminal, OS.exec, File.delete }
        if self.peek_token.token_type != TokenType::Ident || self.peek_token.literal != "permissions" {
            self.parser_error("expected 'permissions' after 'use'");
            return None;
        }
        self.next_token(); // current = "permissions"
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("expected '{' after 'use permissions'");
            return None;
        }
        self.next_token(); // current = '{'
        let mut perms: Vec<String> = Vec::new();
        loop {
            if self.peek_token.token_type == TokenType::RBrace || self.peek_token.token_type == TokenType::Eof {
                self.next_token();
                break;
            }
            self.next_token(); // current = permission name (Ident)
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error("expected permission name inside 'use permissions { }'");
                return None;
            }
            let mut perm = self.current_token.literal.clone();
            // Handle dotted names: OS.exec, File.delete
            while self.peek_token.token_type == TokenType::Dot {
                self.next_token(); // current = '.'
                if self.peek_token.token_type != TokenType::Ident {
                    self.parser_error("expected identifier after '.' in permission name");
                    return None;
                }
                self.next_token(); // current = sub-name
                perm.push('.');
                perm.push_str(&self.current_token.literal);
            }
            perms.push(perm);
            if self.peek_token.token_type == TokenType::Comma {
                self.next_token(); // consume ','
            } else if self.peek_token.token_type == TokenType::RBrace {
                self.next_token(); // consume '}'
                break;
            } else {
                self.parser_error("expected ',' or '}' in 'use permissions'");
                return None;
            }
        }
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::UsePermissions(perms))
    }

    fn parse_import_statement(&mut self) -> Option<Statement> {
        // import "path/to/module";
        if self.peek_token.token_type != TokenType::String {
            self.parser_error("expected string path after 'import'");
            return None;
        }
        self.next_token(); // current = string literal
        let path = self.current_token.literal.clone();
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::Import(path))
    }

    // Parse `[a, _, b, ...rest]` — caller must be positioned at `[`.
    // Returns (slots, rest_name). Leaves current_token at `]`.
    fn parse_array_destructure_pattern(&mut self) -> Option<(Vec<Option<String>>, Option<String>)> {
        // current = '['
        let mut slots: Vec<Option<String>> = Vec::new();
        let mut rest: Option<String> = None;

        // empty pattern []
        if self.peek_token.token_type == TokenType::RBracket {
            self.next_token(); // current = ']'
            return Some((slots, rest));
        }

        loop {
            self.next_token(); // current = name | _ | ... | ]
            match self.current_token.token_type.clone() {
                TokenType::RBracket => break,
                TokenType::Ident => {
                    let name = self.current_token.literal.clone();
                    slots.push(if name == "_" { None } else { Some(name) });
                }
                TokenType::DotDotDot => {
                    // rest element
                    if self.peek_token.token_type != TokenType::Ident {
                        self.parser_error("Expected identifier after '...' in destructure");
                        return None;
                    }
                    self.next_token();
                    rest = Some(self.current_token.literal.clone());
                    // must be followed by ']'
                    if self.peek_token.token_type != TokenType::RBracket {
                        self.parser_error("Rest element must be last in array destructure");
                        return None;
                    }
                    self.next_token(); // current = ']'
                    break;
                }
                _ => {
                    self.parser_error("Expected identifier or '...' in array destructure pattern");
                    return None;
                }
            }
            // after a slot: expect ',' or ']'
            match self.peek_token.token_type {
                TokenType::Comma      => { self.next_token(); } // consume ','
                TokenType::RBracket   => { self.next_token(); break; } // consume ']'
                _ => {
                    self.parser_error("Expected ',' or ']' in array destructure pattern");
                    return None;
                }
            }
        }
        Some((slots, rest))
    }

    // Parse `{key, key: alias}` — caller must be positioned at `{`.
    // Returns Vec<(key, local_alias)>. Leaves current_token at `}`.
    fn parse_dict_destructure_pattern(&mut self) -> Option<Vec<(String, Option<String>)>> {
        // current = '{'
        let mut fields: Vec<(String, Option<String>)> = Vec::new();

        if self.peek_token.token_type == TokenType::RBrace {
            self.next_token(); // current = '}'
            return Some(fields);
        }

        loop {
            self.next_token(); // current = key name
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error("Expected property name in dict destructure pattern");
                return None;
            }
            let key = self.current_token.literal.clone();

            // optional rename: {key: alias}
            let alias = if self.peek_token.token_type == TokenType::Colon {
                self.next_token(); // consume ':'
                if self.peek_token.token_type != TokenType::Ident {
                    self.parser_error("Expected identifier after ':' in dict destructure");
                    return None;
                }
                self.next_token();
                Some(self.current_token.literal.clone())
            } else {
                None
            };
            fields.push((key, alias));

            match self.peek_token.token_type {
                TokenType::Comma    => { self.next_token(); }
                TokenType::RBrace   => { self.next_token(); break; }
                _ => {
                    self.parser_error("Expected ',' or '}}' in dict destructure pattern");
                    return None;
                }
            }
        }
        Some(fields)
    }

    fn parse_sizeof_expression(&mut self) -> Option<Expression> {
        use crate::ast::{SizeOfTarget};
        if self.peek_token.token_type != TokenType::LParen {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected '(' after 'sizeof'");
            return None;
        }
        self.next_token(); // consume '('
        self.next_token(); // move to the argument

        let type_names = ["int", "decimal", "dec", "bool", "string", "null", "void", "any"];
        let target = if matches!(self.current_token.token_type,
            TokenType::KwInt | TokenType::KwDecimal | TokenType::KwDec | TokenType::KwBool |
            TokenType::KwString | TokenType::KwNull | TokenType::KwVoid | TokenType::KwAny)
        {
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
                    if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                    return Some(Statement::DerefAssign { ptr: ptr_clone, value });
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
                        } else { None };
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
                                    line, column,
                                })),
                                operator: op,
                                right: Box::new(rhs),
                                line, column,
                            })
                        } else { rhs };
                        if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                        return Some(Statement::FieldAssign(FieldAssignStatement { object, field, value }));
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
            let op  = if is_incr { "+" } else { "-" };
            let line   = self.current_token.line;
            let column = self.current_token.column;

            if let Expression::DotCall(ref dot) = expr {
                if dot.arguments.is_empty() {
                    if let Expression::Identifier(ref obj_name) = *dot.object {
                        let object = obj_name.clone();
                        let field  = dot.method.clone();
                        let dline  = dot.line;
                        let dcol   = dot.column;
                        self.next_token(); // ++ or --
                        if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                        let value = Expression::Infix(InfixExpression {
                            left: Box::new(Expression::DotCall(DotCallExpression {
                                object:      Box::new(Expression::Identifier(object.clone())),
                                method:      field.clone(),
                                arguments:   vec![],
                                has_parens:  false,
                                is_optional: false,
                                line: dline, column: dcol,
                            })),
                            operator: op.to_string(),
                            right: Box::new(Expression::Integer(1)),
                            line, column,
                        });
                        return Some(Statement::FieldAssign(FieldAssignStatement { object, field, value }));
                    }
                }
            }

            if let Expression::Index(ref idx_expr) = expr {
                let target = (*idx_expr.left).clone();
                let index  = (*idx_expr.index).clone();
                self.next_token(); // ++ or --
                if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                let value = Expression::Infix(InfixExpression {
                    left:     Box::new(expr.clone()),
                    operator: op.to_string(),
                    right:    Box::new(Expression::Integer(1)),
                    line, column,
                });
                return Some(Statement::IndexAssign(IndexAssignStatement { target, index, value }));
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
            return Some(Statement::Return(ReturnStatement { return_value: Expression::Null }));
        }

        self.next_token();

        // Bare `return;` — no expression, return null
        if self.current_token.token_type == TokenType::Semicolon {
            return Some(Statement::Return(ReturnStatement { return_value: Expression::Null }));
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

    fn parse_while_statement_with_label(&mut self, label: Option<String>) -> Option<Statement> {
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'while'");
            return None;
        }
        self.next_token();
        self.next_token();

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after condition in 'while'");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' to start 'while' body");
            return None;
        }
        self.next_token();

        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Statement::While(WhileStatement { condition, body, label }))
    }

    fn parse_while_statement(&mut self) -> Option<Statement> {
        self.parse_while_statement_with_label(None)
    }

    fn parse_do_while_statement(&mut self) -> Option<Statement> {
        // current = 'do'
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after 'do'");
            return None;
        }
        self.next_token(); // current = '{'
        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };
        // current = '}', peek = 'while'
        if self.peek_token.token_type != TokenType::While {
            self.parser_error("Expected 'while' after 'do' body");
            return None;
        }
        self.next_token(); // current = 'while'
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'while' in do-while");
            return None;
        }
        self.next_token(); // current = '('
        self.next_token(); // current = first token of condition
        let condition = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after condition in do-while");
            return None;
        }
        self.next_token(); // current = ')'
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token(); // consume ';'
        }
        Some(Statement::DoWhile(WhileStatement { condition, body, label: None }))
    }

    fn parse_index_assign_or_expr_statement(&mut self) -> Option<Statement> {
        let expr = self.parse_expression(Precedence::Lowest)?;
        if self.is_compound_assign(&self.peek_token.token_type) {
            return self.try_build_index_compound_assign(expr);
        }
        // arr[i]++  /  arr[i]--
        if matches!(self.peek_token.token_type, TokenType::PlusPlus | TokenType::MinusMinus) {
            if let Expression::Index(ref idx_expr) = expr {
                let target = (*idx_expr.left).clone();
                let index  = (*idx_expr.index).clone();
                let line   = self.current_token.line;
                let column = self.current_token.column;
                let op = if self.peek_token.token_type == TokenType::PlusPlus { "+" } else { "-" };
                self.next_token(); // ++ or --
                if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                let value = Expression::Infix(InfixExpression {
                    left: Box::new(expr),
                    operator: op.to_string(),
                    right: Box::new(Expression::Integer(1)),
                    line, column,
                });
                return Some(Statement::IndexAssign(IndexAssignStatement { target, index, value }));
            }
        }
        self.try_build_index_assign(expr)
    }

    fn try_build_index_assign(&mut self, expr: Expression) -> Option<Statement> {
        if self.peek_token.token_type == TokenType::Assign {
            if let Expression::Index(idx_expr) = &expr {
                let target = (*idx_expr.left).clone();
                let index = (*idx_expr.index).clone();
                self.next_token(); // '='
                self.next_token(); // first token of value
                let value = self.parse_expression(Precedence::Lowest)?;
                if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
                return Some(Statement::IndexAssign(IndexAssignStatement { target, index, value }));
            }
        }
        if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
        Some(Statement::Expression(expr))
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
                line, column,
            });
            if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
            return Some(Statement::IndexAssign(IndexAssignStatement { target, index, value }));
        }
        if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
        Some(Statement::Expression(expr))
    }

    fn parse_for_statement_with_label(&mut self, label: Option<String>) -> Option<Statement> {
        self.parse_for_inner(label)
    }

    fn parse_for_statement(&mut self) -> Option<Statement> {
        self.parse_for_inner(None)
    }

    fn parse_for_inner(&mut self, label: Option<String>) -> Option<Statement> {
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'for'");
            return None;
        }
        self.next_token(); // current = '('
        self.next_token(); // current = 'let'

        if self.current_token.token_type != TokenType::Let {
            self.parser_error("Expected 'let' as for-loop initializer");
            return None;
        }

        // ── ForEach with array destructuring: for (let [a, b] in ...) ─────────
        if self.peek_token.token_type == TokenType::LBracket {
            self.next_token(); // current = '['
            let (slots, rest) = self.parse_array_destructure_pattern()?;
            // current is now ']'
            if self.peek_token.token_type != TokenType::KwIn {
                self.parser_error("Expected 'in' after destructure pattern in for");
                return None;
            }
            self.next_token(); // current = 'in'
            self.next_token(); // current = first token of iterable
            let iterable = self.parse_expression(Precedence::Lowest)?;
            if self.peek_token.token_type != TokenType::RParen {
                self.parser_error("Expected ')' after for-in iterable");
                return None;
            }
            self.next_token(); // current = ')'
            if self.peek_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' to start for-in body");
                return None;
            }
            self.next_token();
            let body = match self.parse_block_statement()? {
                Statement::Block(b) => b,
                _ => return None,
            };
            return Some(Statement::ForEach(ForEachStatement {
                var: ForEachVar::Array(slots, rest),
                iterable, body, label: label.clone(),
            }));
        }

        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected identifier after 'let' in for");
            return None;
        }
        self.next_token(); // current = var_name
        let var_name = self.current_token.literal.clone();

        // ── ForEach: for (let x in iterable) { body } ────────────────────────
        if self.peek_token.token_type == TokenType::KwIn {
            self.next_token(); // current = 'in'
            self.next_token(); // current = first token of iterable
            let iterable = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type != TokenType::RParen {
                self.parser_error("Expected ')' after for-in iterable");
                return None;
            }
            self.next_token(); // current = ')'

            if self.peek_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' to start for-in body");
                return None;
            }
            self.next_token(); // current = '{'

            let body = match self.parse_block_statement()? {
                Statement::Block(b) => b,
                _ => return None,
            };

            return Some(Statement::ForEach(ForEachStatement {
                var: ForEachVar::Name(var_name),
                iterable, body, label: label.clone(),
            }));
        }

        // ── Classic for: for (let i = 0; i < n; i = i + 1) { body } ─────────
        if self.peek_token.token_type != TokenType::Assign {
            self.parser_error("Expected '=' or 'in' after variable name in for");
            return None;
        }
        self.next_token(); // current = '='
        self.next_token(); // current = first token of init value
        let init_value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token(); // current = ';'
        } else if self.current_token.token_type != TokenType::Semicolon {
            self.parser_error("Expected ';' after for-loop initializer");
            return None;
        }
        self.next_token(); // current = first token of condition

        let init = LetStatement { name: var_name, value: init_value, is_const: false };

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::Semicolon {
            self.parser_error("Expected ';' after for-loop condition");
            return None;
        }
        self.next_token();
        self.next_token();

        if self.current_token.token_type != TokenType::Ident {
            self.parser_error("Expected assignment as for-loop update");
            return None;
        }
        let update = match self.peek_token.token_type {
            TokenType::Assign => match self.parse_assign_statement()? {
                Statement::Assign(a) => a,
                _ => return None,
            },
            TokenType::PlusPlus | TokenType::MinusMinus => {
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col  = self.current_token.column;
                let op   = if self.peek_token.token_type == TokenType::PlusPlus { "+" } else { "-" };
                self.next_token(); // consume ++ / --
                AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: op.to_string(),
                        right: Box::new(Expression::Integer(1)),
                        line, column: col,
                    }),
                }
            }
            ref tt if self.is_compound_assign(&tt.clone()) => {
                match self.parse_compound_assign_statement()? {
                    Statement::Assign(a) => a,
                    _ => return None,
                }
            }
            _ => {
                self.parser_error("Expected assignment as for-loop update");
                return None;
            }
        };

        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after for-loop update");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' to start for-loop body");
            return None;
        }
        self.next_token();

        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Statement::For(ForStatement { init, condition, update, body, label }))
    }

    fn parse_if_expression(&mut self) -> Option<Expression> {
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'if'");
            return None;
        }
        self.next_token();
        self.next_token();

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after 'if' condition");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' to start 'if' consequence");
            return None;
        }
        self.next_token();

        let consequence = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        let mut alternative = None;

        if self.peek_token.token_type == TokenType::Else {
            self.next_token();

            if self.peek_token.token_type == TokenType::If {
                self.next_token();

                if let Some(if_expr) = self.parse_if_expression() {
                    alternative = Some(BlockStatement {
                        statements: vec![Statement::Expression(if_expr)],
                    });
                }
            } else {
                if self.peek_token.token_type != TokenType::LBrace {
                    self.parser_error("Expected '{{' or 'if' after 'else'");
                    return None;
                }
                self.next_token();
                alternative = match self.parse_block_statement()? {
                    Statement::Block(b) => Some(b),
                    _ => None,
                };
            }
        }

        Some(Expression::If(IfExpression {
            condition: Box::new(condition),
            consequence,
            alternative,
        }))
    }

    fn parse_let_statement(&mut self) -> Option<Statement> {
        let is_const = self.current_token.token_type == TokenType::KwConst;

        // Array destructuring: let [a, b, ...rest] = expr;
        if self.peek_token.token_type == TokenType::LBracket {
            self.next_token(); // current = '['
            let (names, rest) = self.parse_array_destructure_pattern()?;
            // current is now ']'
            if self.peek_token.token_type != TokenType::Assign {
                self.parser_error("Expected '=' after array destructure pattern");
                return None;
            }
            self.next_token(); // '='
            self.next_token(); // first token of value
            let value = self.parse_expression(Precedence::Lowest)?;
            if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
            return Some(Statement::LetDestructureArray(LetDestructureArray { names, rest, value, is_const }));
        }

        // Dict destructuring: let {key, key: alias} = expr;
        if self.peek_token.token_type == TokenType::LBrace {
            self.next_token(); // current = '{'
            let fields = self.parse_dict_destructure_pattern()?;
            // current is now '}'
            if self.peek_token.token_type != TokenType::Assign {
                self.parser_error("Expected '=' after dict destructure pattern");
                return None;
            }
            self.next_token(); // '='
            self.next_token(); // first token of value
            let value = self.parse_expression(Precedence::Lowest)?;
            if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
            return Some(Statement::LetDestructureDict(LetDestructureDict { fields, value, is_const }));
        }

        if self.peek_token.token_type != TokenType::Ident {
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();

        // Typed array annotation: let name [type] = [...]
        if self.peek_token.token_type == TokenType::LBracket {
            self.next_token(); // consume '['
            self.next_token(); // move to type keyword
            if !is_type_keyword(&self.current_token.token_type) {
                self.parser_error("Expected type keyword inside '[...]' array annotation");
                return None;
            }
            let element_type = self.parse_type_string()?;
            if self.peek_token.token_type != TokenType::RBracket {
                self.parser_error("Expected ']' after array type annotation");
                return None;
            }
            self.next_token(); // consume ']'
            if self.peek_token.token_type != TokenType::Assign {
                self.parser_error("Expected '=' after array type annotation");
                return None;
            }
            self.next_token(); // '='
            self.next_token(); // first token of RHS
            let mut value = self.parse_expression(Precedence::Lowest)?;
            match &mut value {
                Expression::ArrayLiteral(arr) => arr.element_type = Some(element_type),
                _ => {
                    self.parser_error("Expected '[...]' array literal after typed array annotation");
                    return None;
                }
            }
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }
            return Some(Statement::Let(LetStatement { name, value, is_const }));
        }

        if self.peek_token.token_type == TokenType::Lt {
            let (key_type, value_type) = self.parse_dict_type_annotation()?;

            if self.peek_token.token_type != TokenType::Assign {
                self.parser_error("Expected '=' after dict type annotation");
                return None;
            }
            self.next_token();
            self.next_token();

            if self.current_token.token_type != TokenType::LParen {
                self.parser_error("Expected '(' to start dict literal");
                return None;
            }

            let value = self.parse_dict_literal(key_type, value_type)?;

            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }

            return Some(Statement::Let(LetStatement { name, value, is_const }));
        }

        if self.peek_token.token_type != TokenType::Assign {
            return None;
        }
        self.next_token(); // '='
        self.next_token(); // first token of value

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Let(LetStatement { name, value, is_const }))
    }

    fn parse_function_statement(&mut self) -> Option<Statement> {
        // fn* generator syntax: consume the '*'
        let is_generator = self.peek_token.token_type == TokenType::Asterisk;
        if is_generator { self.next_token(); } // consume '*'

        let mut return_type = None;
        if is_type_keyword(&self.peek_token.token_type) {
            self.next_token();
            return_type = self.parse_type_string();
        } else if self.peek_token.token_type == TokenType::LBracket {
            self.next_token(); // '['
            self.next_token(); // type keyword
            if !is_type_keyword(&self.current_token.token_type) {
                self.parser_error("Expected type keyword inside '[...]' return type");
                return None;
            }
            let elem_type = self.parse_type_string()?;
            if self.peek_token.token_type != TokenType::RBracket {
                self.parser_error("Expected ']' after return type annotation");
                return None;
            }
            self.next_token(); // ']'
            return_type = Some(format!("[{}]", elem_type));
        }

        if self.peek_token.token_type == TokenType::Ident {
            self.next_token();
            // parse_type_string also consumes an optional '?' (for nullable class types)
            let first = self.parse_type_string().unwrap_or_default();

            // Disambiguate: fn ClassName[?] funcName(...) vs fn funcName(...)
            let name = if self.peek_token.token_type == TokenType::Ident {
                return_type = Some(first);
                self.next_token();
                self.current_token.literal.clone()
            } else {
                first
            };

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

            let function = FunctionLiteral { return_type, parameters, body, is_generator };

            Some(Statement::FunctionDeclaration(FunctionDeclaration { name, function }))
        } else {
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

            let function = FunctionLiteral { return_type, parameters, body, is_generator };

            Some(Statement::Expression(Expression::FunctionLiteral(function)))
        }
    }

    fn parse_function_parameters(&mut self) -> Option<Vec<Parameter>> {
        let mut parameters = Vec::new();

        if self.peek_token.token_type == TokenType::RParen {
            self.next_token();
            return Some(parameters);
        }

        self.next_token();

        loop {
            let mut type_name = None;

            if self.current_token.token_type == TokenType::LBracket {
                // [type] array parameter annotation
                self.next_token(); // move to type keyword
                if !is_type_keyword(&self.current_token.token_type) {
                    self.parser_error("Expected type keyword inside '[...]' parameter annotation");
                    return None;
                }
                let elem_type = self.parse_type_string()?;
                if self.peek_token.token_type != TokenType::RBracket {
                    self.parser_error("Expected ']' after array parameter type");
                    return None;
                }
                self.next_token(); // consume ']'
                type_name = Some(format!("[{}]", elem_type));
                self.next_token(); // advance to param name
            } else if is_type_keyword(&self.current_token.token_type) {
                type_name = self.parse_type_string();
                self.next_token();
            } else if self.current_token.token_type == TokenType::Ident
                && (self.peek_token.token_type == TokenType::Ident
                    || self.peek_token.token_type == TokenType::Question)
            {
                // Class type annotation (possibly nullable): fn void f(ClassName[?] param)
                type_name = self.parse_type_string();
                self.next_token();
            }

            // Check for rest parameter `...name`
            let is_rest = if self.current_token.token_type == TokenType::DotDotDot {
                self.next_token(); // advance to param name
                true
            } else {
                false
            };

            let name = if self.current_token.token_type == TokenType::Ident {
                self.current_token.literal.clone()
            } else {
                return None;
            };

            // Optional default value: param = expr
            let default_value = if !is_rest && self.peek_token.token_type == TokenType::Assign {
                self.next_token(); // '='
                self.next_token(); // first token of default expr
                Some(self.parse_expression(Precedence::Lowest)?)
            } else {
                None
            };

            parameters.push(Parameter { name, type_name, is_rest, default_value });

            if is_rest {
                // Rest param must be last — break after
                break;
            }

            if self.peek_token.token_type == TokenType::Comma {
                self.next_token();
                self.next_token();
            } else {
                break;
            }
        }

        if self.peek_token.token_type != TokenType::RParen {
            return None;
        }
        self.next_token();

        Some(parameters)
    }

    fn parse_arrow_function(&mut self) -> Option<Expression> {
        let return_type = self.parse_type_string();

        if self.peek_token.token_type != TokenType::LParen {
            return None;
        }
        self.next_token();

        let parameters = self.parse_function_parameters()?;

        if self.peek_token.token_type != TokenType::Arrow {
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            return None;
        }
        self.next_token();

        let body_stmt = self.parse_block_statement()?;
        let body = match body_stmt {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Expression::FunctionLiteral(FunctionLiteral { return_type, parameters, body, is_generator: false }))
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

    fn parse_lambda_body(&mut self) -> Option<LambdaBody> {
        // current = '=>'
        if self.peek_token.token_type == TokenType::LBrace {
            self.next_token(); // '{'
            let block = match self.parse_block_statement()? {
                Statement::Block(b) => b,
                _ => return None,
            };
            Some(LambdaBody::Block(block))
        } else {
            self.next_token(); // first token of expression
            let expr = self.parse_expression(Precedence::Lowest)?;
            Some(LambdaBody::Expr(Box::new(expr)))
        }
    }

    // ── Expression parsing ────────────────────────────────────────────────────

    /// Continues the infix chain starting from an already-parsed left expression.
    /// Used by both parse_expression and the lambda fallback grouped-expr case.
    fn parse_infix_chain(
        &mut self,
        mut left_exp: Option<Expression>,
        precedence: Precedence,
    ) -> Option<Expression> {
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
                let op_line   = self.current_token.line;
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
                let call_line   = self.current_token.line;
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
        // ── PREFIX ────────────────────────────────────────────────────────────
        let left_exp = match self.current_token.token_type {
            // Single-param lambda: item => body
            TokenType::Ident if self.peek_token.token_type == TokenType::Arrow => {
                let param = self.current_token.literal.clone();
                self.next_token(); // consume '=>'
                let body = self.parse_lambda_body()?;
                Some(Expression::Lambda(LambdaExpression { params: vec![param], body }))
            }

            TokenType::Ident => {
                Some(Expression::Identifier(self.current_token.literal.clone()))
            }

            TokenType::Int => {
                if let Ok(num) = self.current_token.literal.parse::<i64>() {
                    Some(Expression::Integer(num))
                } else {
                    None
                }
            }

            TokenType::Decimal => {
                if let Ok(num) = self.current_token.literal.parse::<f64>() {
                    Some(Expression::Decimal(num))
                } else {
                    None
                }
            }

            TokenType::Dec => {
                match parse_dec_literal(&self.current_token.literal) {
                    Some(d) => Some(Expression::Dec(d)),
                    None => None,
                }
            }

            TokenType::String => {
                let s = self.current_token.literal.clone();
                if s.contains('{') {
                    let parsed = parse_interpolated_string(&s);
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
                    Some(Expression::Lambda(LambdaExpression { params: vec![], body }))
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
                        Some(Expression::Lambda(LambdaExpression { params: vec![first_name], body }))
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

                Some(Expression::FunctionLiteral(FunctionLiteral { return_type, parameters, body, is_generator: false }))
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

            _ => None,
        };

        // ── INFIX ─────────────────────────────────────────────────────────────
        self.parse_infix_chain(left_exp, precedence)
    }

    fn parse_dict_type_annotation(&mut self) -> Option<(String, String)> {
        self.next_token(); // '<'
        self.next_token(); // key_type

        if !is_type_keyword(&self.current_token.token_type) {
            self.parser_error(&format!("Expected type keyword for dict key type, got '{}'", self.current_token.literal));
            return None;
        }
        let key_type = self.current_token.literal.clone();

        if self.peek_token.token_type != TokenType::Comma {
            self.parser_error("Expected ',' between key and value types in dict annotation");
            return None;
        }
        self.next_token(); // ','
        self.next_token(); // value_type

        if !is_type_keyword(&self.current_token.token_type) {
            self.parser_error(&format!("Expected type keyword for dict value type, got '{}'", self.current_token.literal));
            return None;
        }
        let value_type = self.current_token.literal.clone();

        if self.peek_token.token_type != TokenType::Gt {
            self.parser_error("Expected '>' to close dict type annotation");
            return None;
        }
        self.next_token(); // '>'

        Some((key_type, value_type))
    }

    fn parse_dict_literal(&mut self, key_type: String, value_type: String) -> Option<Expression> {
        let mut entries = Vec::new();

        if self.peek_token.token_type == TokenType::RParen {
            self.next_token();
            return Some(Expression::DictLiteral(DictLiteral { key_type, value_type, entries }));
        }

        self.next_token(); // first '{'

        loop {
            if self.current_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' to start dict entry");
                return None;
            }
            self.next_token();

            let key = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type != TokenType::Comma {
                self.parser_error("Expected ',' between key and value in dict entry");
                return None;
            }
            self.next_token();
            self.next_token();

            let value = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type != TokenType::RBrace {
                self.parser_error("Expected '}}' to close dict entry");
                return None;
            }
            self.next_token(); // '}'

            entries.push((key, value));

            if self.peek_token.token_type == TokenType::RParen {
                self.next_token();
                break;
            }

            if self.peek_token.token_type != TokenType::Comma {
                self.parser_error("Expected ',' or ')' after dict entry");
                return None;
            }
            self.next_token(); // ','
            self.next_token(); // next '{'
        }

        Some(Expression::DictLiteral(DictLiteral { key_type, value_type, entries }))
    }

    fn parse_entry_literal(&mut self) -> Option<Expression> {
        self.next_token();

        let key = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::Comma {
            self.parser_error("Expected ',' between key and value in entry literal");
            return None;
        }
        self.next_token();
        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RBrace {
            self.parser_error("Expected '}}' to close entry literal");
            return None;
        }
        self.next_token();

        Some(Expression::EntryLiteral(Box::new(key), Box::new(value)))
    }

    // ── { ... } disambiguation ────────────────────────────────────────────────
    // When '{' appears in expression context:
    //   - If next token is Ident and next-next is ':' → ObjectPatch { field: val, ... }
    //   - Otherwise → EntryLiteral {key, value} (for dict method args)
    fn parse_brace_expression(&mut self) -> Option<Expression> {
        if self.peek_token.token_type == TokenType::Ident {
            // Consume '{', now current = Ident
            self.next_token();
            if self.peek_token.token_type == TokenType::Colon {
                // ObjectPatch: { field: val, ... }
                return self.parse_object_patch_from_ident();
            } else {
                // Entry literal: { ident, value }
                let key = Expression::Identifier(self.current_token.literal.clone());
                return self.parse_entry_literal_from_key(key);
            }
        }
        self.parse_entry_literal()
    }

    // current = first field name (already consumed '{' and Ident)
    fn parse_object_patch_from_ident(&mut self) -> Option<Expression> {
        let mut fields = Vec::new();
        loop {
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error("Expected field name in object literal");
                return None;
            }
            let name = self.current_token.literal.clone();
            if self.peek_token.token_type != TokenType::Colon {
                self.parser_error("Expected ':' after field name in object literal");
                return None;
            }
            self.next_token(); // ':'
            self.next_token(); // value
            let value = self.parse_expression(Precedence::Lowest)?;
            fields.push((name, value));

            match self.peek_token.token_type {
                TokenType::Comma => {
                    self.next_token(); // ','
                    if self.peek_token.token_type == TokenType::RBrace {
                        self.next_token(); // '}'
                        break;
                    }
                    self.next_token(); // next field name
                }
                TokenType::RBrace => {
                    self.next_token(); // '}'
                    break;
                }
                _ => {
                    self.parser_error("Expected ',' or '}}' in object literal");
                    return None;
                }
            }
        }
        Some(Expression::ObjectPatch(fields))
    }

    // current = first ident of key (already consumed '{'); continue parsing full key expression
    fn parse_entry_literal_from_key(&mut self, key_start: Expression) -> Option<Expression> {
        // The key might be more than just the ident (e.g. nombres[i])
        let key = self.parse_infix_chain(Some(key_start), Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::Comma {
            self.parser_error("Expected ',' between key and value in entry literal");
            return None;
        }
        self.next_token(); // ','
        self.next_token(); // value
        let value = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type != TokenType::RBrace {
            self.parser_error("Expected '}}' to close entry literal");
            return None;
        }
        self.next_token(); // '}'
        Some(Expression::EntryLiteral(Box::new(key), Box::new(value)))
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

    // ── Interface declaration ─────────────────────────────────────────────────
    fn parse_interface_declaration(&mut self, is_public: bool) -> Option<Statement> {
        // current = 'interface'
        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected interface name after 'interface'");
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();
        if self.is_reserved_name(&name) {
            self.parser_error(&format!("'{}' is a reserved system namespace and cannot be used as an interface name", name));
            return None;
        }

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after interface name");
            return None;
        }
        self.next_token(); // '{'
        self.next_token(); // first field or '}'

        let mut fields = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error("Expected field name in interface body");
                return None;
            }
            let field_name = self.current_token.literal.clone();

            if self.peek_token.token_type != TokenType::Colon {
                self.parser_error(&format!("Expected ':' after field name '{}' in interface", field_name));
                return None;
            }
            self.next_token(); // ':'
            self.next_token(); // type

            let type_name = if self.current_token.token_type == TokenType::LBracket {
                // Array field type: [int], [string], [ClassName], etc.
                self.next_token(); // elem type
                let elem = if is_type_keyword(&self.current_token.token_type) {
                    self.parse_type_string().unwrap_or_default()
                } else if self.current_token.token_type == TokenType::Ident {
                    self.current_token.literal.clone()
                } else {
                    self.parser_error(&format!("Expected element type inside '[...]' for field '{}' in interface", field_name));
                    return None;
                };
                if self.peek_token.token_type != TokenType::RBracket {
                    self.parser_error("Expected ']' after array field type");
                    return None;
                }
                self.next_token(); // ']'
                format!("[{}]", elem)
            } else if is_type_keyword(&self.current_token.token_type) {
                match self.parse_type_string() {
                    Some(t) => t,
                    None => {
                        self.parser_error(&format!("Expected type after ':' for field '{}' in interface", field_name));
                        return None;
                    }
                }
            } else if self.current_token.token_type == TokenType::Ident {
                // Class/interface type name (possibly nullable)
                self.parse_type_string().unwrap_or_else(|| self.current_token.literal.clone())
            } else {
                self.parser_error(&format!("Expected type after ':' for field '{}' in interface", field_name));
                return None;
            };
            fields.push(InterfaceField { name: field_name, type_name });

            // consume ';' or ','
            if self.peek_token.token_type == TokenType::Semicolon
                || self.peek_token.token_type == TokenType::Comma
            {
                self.next_token();
            }
            self.next_token(); // next field or '}'
        }

        Some(Statement::InterfaceDeclaration(InterfaceDeclaration { name, is_public, fields }))
    }

    // ── Class declaration ─────────────────────────────────────────────────────
    fn parse_class_declaration(&mut self, is_public: bool, is_abstract: bool, is_sealed: bool) -> Option<Statement> {
        // current = 'class'
        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected class name after 'class'");
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();
        if self.is_reserved_name(&name) {
            self.parser_error(&format!("'{}' is a reserved system namespace and cannot be used as a class name", name));
            return None;
        }

        // Optional inheritance: class Child : Parent
        let parent = if self.peek_token.token_type == TokenType::Colon {
            self.next_token(); // ':'
            if self.peek_token.token_type != TokenType::Ident {
                self.parser_error("Expected parent class name after ':'");
                return None;
            }
            self.next_token();
            Some(self.current_token.literal.clone())
        } else {
            None
        };

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after class name");
            return None;
        }
        self.next_token(); // '{'
        self.next_token(); // first member or '}'

        let mut constructor: Option<ClassConstructor> = None;
        let mut methods: Vec<ClassMethod> = Vec::new();
        let mut fields: Vec<ClassField> = Vec::new();

        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            // Check for abstract method prefix
            let is_member_abstract = if self.current_token.token_type == TokenType::KwAbstract {
                self.next_token(); // after 'abstract'
                true
            } else {
                false
            };

            // visibility modifier
            let is_member_public = match self.current_token.token_type {
                TokenType::KwPublic => true,
                TokenType::KwPrivate => false,
                _ => {
                    // If we hit abstract directly after visibility etc.
                    // Or if it's a class field (Ident: type = value;)
                    // Try to parse as a class field
                    if self.current_token.token_type == TokenType::Ident {
                        let field_name = self.current_token.literal.clone();
                        let field_line = self.current_token.line;
                        let field_col = self.current_token.column;

                        if self.peek_token.token_type == TokenType::Colon {
                            // field: type [= expr];
                            self.next_token(); // ':'
                            self.next_token(); // type
                            let type_annotation = if is_type_keyword(&self.current_token.token_type) {
                                self.parse_type_string()
                            } else if self.current_token.token_type == TokenType::Ident {
                                Some(self.current_token.literal.clone())
                            } else {
                                None
                            };
                            let default_value = if self.peek_token.token_type == TokenType::Assign {
                                self.next_token(); // '='
                                self.next_token(); // expr
                                Some(self.parse_expression(Precedence::Lowest)?)
                            } else {
                                None
                            };
                            if self.peek_token.token_type == TokenType::Semicolon {
                                self.next_token();
                            }
                            fields.push(ClassField { name: field_name, type_annotation, default_value, line: field_line, column: field_col });
                            self.next_token();
                            continue;
                        } else if self.peek_token.token_type == TokenType::Assign {
                            // field = expr;
                            self.next_token(); // '='
                            self.next_token(); // expr
                            let default_value = Some(self.parse_expression(Precedence::Lowest)?);
                            if self.peek_token.token_type == TokenType::Semicolon {
                                self.next_token();
                            }
                            fields.push(ClassField { name: field_name, type_annotation: None, default_value, line: field_line, column: field_col });
                            self.next_token();
                            continue;
                        }
                    }
                    self.had_error.set(true);
                    eprintln!(
                        "❌ PARSER ERROR: Expected 'public' or 'private' for class member, got '{}'",
                        self.current_token.literal
                    );
                    return None;
                }
            };
            self.next_token(); // after visibility

            // Check for static modifier
            let is_static = if self.current_token.token_type == TokenType::KwStatic {
                self.next_token();
                true
            } else {
                false
            };

            // Check for getter/setter
            let is_getter = if self.current_token.token_type == TokenType::KwGet {
                self.next_token();
                true
            } else {
                false
            };
            let is_setter = if !is_getter && self.current_token.token_type == TokenType::KwSet {
                self.next_token();
                true
            } else {
                false
            };

            // Optional return type keyword (void, int, decimal, [type], class name, etc.)
            let return_type = if self.current_token.token_type == TokenType::LBracket {
                // Array return type: [int], [string], [ClassName], etc.
                self.next_token(); // move to type inside brackets
                let elem = if is_type_keyword(&self.current_token.token_type) {
                    self.parse_type_string().unwrap_or_default()
                } else {
                    self.current_token.literal.clone()
                };
                if self.peek_token.token_type != TokenType::RBracket {
                    self.parser_error("Expected ']' after array return type");
                    return None;
                }
                self.next_token(); // consume ']'
                self.next_token(); // advance to method name
                Some(format!("[{}]", elem))
            } else if is_type_keyword(&self.current_token.token_type) {
                let rt = self.parse_type_string();
                self.next_token();
                rt
            } else if self.current_token.token_type == TokenType::Ident
                && (self.peek_token.token_type == TokenType::Ident
                    || self.peek_token.token_type == TokenType::Question)
            {
                // Class return type (possibly nullable): public ClassName[?] methodName()
                let rt = self.parse_type_string();
                self.next_token();
                rt
            } else {
                None
            };

            // Member name (constructor or method) — allow keywords as names (e.g. "get", "set")
            if !self.current_token_is_name() {
                self.parser_error("Expected method name in class body");
                return None;
            }
            let member_name = self.current_token.literal.clone();

            if self.peek_token.token_type != TokenType::LParen {
                self.parser_error(&format!("Expected '(' after '{}' in class", member_name));
                return None;
            }
            self.next_token(); // '('
            let parameters = self.parse_function_parameters()?;

            // Abstract methods may have no body (semicolon) or empty body
            let body = if is_member_abstract && self.peek_token.token_type == TokenType::Semicolon {
                self.next_token(); // ';'
                BlockStatement { statements: vec![] }
            } else {
                if self.peek_token.token_type != TokenType::LBrace {
                    self.parser_error(&format!("Expected '{{' to start body of '{}'", member_name));
                    return None;
                }
                self.next_token();
                let body_stmt = self.parse_block_statement()?;
                match body_stmt {
                    Statement::Block(b) => b,
                    _ => return None,
                }
            };

            if member_name == name && !is_getter && !is_setter {
                // Constructor
                if constructor.is_some() {
                    self.parser_error(&format!("Duplicate constructor in class '{}'", name));
                    return None;
                }
                constructor = Some(ClassConstructor { parameters, body });
            } else {
                methods.push(ClassMethod {
                    name: member_name,
                    is_public: is_member_public,
                    is_abstract: is_member_abstract,
                    is_getter,
                    is_setter,
                    is_static,
                    return_type,
                    parameters,
                    body,
                });
            }

            self.next_token(); // advance past closing '}' of method/constructor
        }

        Some(Statement::ClassDeclaration(ClassDeclaration {
            name,
            is_public,
            is_abstract,
            is_sealed,
            parent,
            constructor,
            methods,
            fields,
        }))
    }

    // ── abstract class / sealed class ────────────────────────────────────────
    fn parse_abstract_or_sealed_class(&mut self, is_abstract: bool, is_sealed: bool) -> Option<Statement> {
        // current = 'abstract' or 'sealed'
        if self.peek_token.token_type == TokenType::KwClass {
            self.next_token(); // 'class'
            self.parse_class_declaration(true, is_abstract, is_sealed)
        } else if self.peek_token.token_type == TokenType::KwPublic || self.peek_token.token_type == TokenType::KwPrivate {
            // public abstract class / private abstract class
            self.next_token(); // pub/priv
            if self.peek_token.token_type == TokenType::KwClass {
                self.next_token(); // 'class'
                self.parse_class_declaration(true, is_abstract, is_sealed)
            } else {
                self.parser_error("Expected 'class' after abstract/sealed");
                None
            }
        } else {
            self.parser_error("Expected 'class' after abstract/sealed");
            None
        }
    }

    // ── Visibility prefix (public/private class|interface) ────────────────────
    fn parse_visibility_statement(&mut self) -> Option<Statement> {
        let is_public = self.current_token.token_type == TokenType::KwPublic;
        match self.peek_token.token_type {
            TokenType::KwClass => {
                self.next_token();
                self.parse_class_declaration(is_public, false, false)
            }
            TokenType::KwInterface => {
                self.next_token();
                self.parse_interface_declaration(is_public)
            }
            TokenType::KwAbstract => {
                self.next_token(); // 'abstract'
                if self.peek_token.token_type == TokenType::KwClass {
                    self.next_token(); // 'class'
                    self.parse_class_declaration(is_public, true, false)
                } else {
                    self.parser_error("Expected 'class' after 'abstract'");
                    None
                }
            }
            TokenType::KwSealed => {
                self.next_token(); // 'sealed'
                if self.peek_token.token_type == TokenType::KwClass {
                    self.next_token(); // 'class'
                    self.parse_class_declaration(is_public, false, true)
                } else {
                    self.parser_error("Expected 'class' after 'sealed'");
                    None
                }
            }
            _ => {
                self.had_error.set(true);
                eprintln!(
                    "❌ PARSER ERROR: Expected 'class' or 'interface' after visibility modifier"
                );
                None
            }
        }
    }

    fn parse_array_literal(&mut self) -> Option<Expression> {
        let mut elements = Vec::new();

        if self.peek_token.token_type == TokenType::RBracket {
            self.next_token();
            return Some(Expression::ArrayLiteral(ArrayLiteral { element_type: None, elements }));
        }

        self.next_token();

        loop {
            let expr = if self.current_token.token_type == TokenType::DotDotDot {
                self.next_token();
                let inner = self.parse_expression(Precedence::Lowest)?;
                Some(Expression::Spread(Box::new(inner)))
            } else {
                self.parse_expression(Precedence::Lowest)
            };

            if let Some(e) = expr {
                elements.push(e);
            }

            if self.peek_token.token_type == TokenType::RBracket {
                self.next_token();
                break;
            }

            if self.peek_token.token_type != TokenType::Comma {
                self.parser_error("Missing closing bracket ']' or comma ',' in array");
                return None;
            }

            self.next_token();
            self.next_token();
        }

        Some(Expression::ArrayLiteral(ArrayLiteral { element_type: None, elements }))
    }

    // ── switch (expr) { case v1, v2: { body } ... default: { body } } ─────────
    fn parse_switch_statement(&mut self) -> Option<Statement> {
        // switch (expr)
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'switch'");
            return None;
        }
        self.next_token(); // '('
        self.next_token(); // first token of expr
        let value = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after switch expression");
            return None;
        }
        self.next_token(); // ')'
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after switch(...)");
            return None;
        }
        self.next_token(); // '{'
        self.next_token(); // first token inside

        let mut cases = Vec::new();
        let mut default = None;

        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if self.current_token.token_type == TokenType::KwDefault {
                // default: { body }
                if self.peek_token.token_type != TokenType::Colon {
                    self.parser_error("Expected ':' after 'default'");
                    return None;
                }
                self.next_token(); // ':'
                if self.peek_token.token_type != TokenType::LBrace {
                    self.parser_error("Expected '{{' after 'default:'");
                    return None;
                }
                self.next_token(); // '{'
                let body = self.parse_inner_block()?;
                default = Some(body);
            } else if self.current_token.token_type == TokenType::KwCase {
                // case v1, v2, ...: { body }
                let mut values = Vec::new();
                self.next_token(); // first value
                let first = self.parse_expression(Precedence::Lowest)?;
                values.push(first);
                while self.peek_token.token_type == TokenType::Comma {
                    self.next_token(); // ','
                    self.next_token(); // next value
                    let v = self.parse_expression(Precedence::Lowest)?;
                    values.push(v);
                }
                if self.peek_token.token_type != TokenType::Colon {
                    self.parser_error("Expected ':' after case value(s)");
                    return None;
                }
                self.next_token(); // ':'
                if self.peek_token.token_type != TokenType::LBrace {
                    self.parser_error("Expected '{{' after 'case ...:'");
                    return None;
                }
                self.next_token(); // '{'
                let body = self.parse_inner_block()?;
                cases.push(SwitchCase { values, body });
            } else {
                self.parser_error(&format!("Expected 'case' or 'default' inside switch, got '{}'", self.current_token.literal));
                return None;
            }
            self.next_token(); // move past '}' of the case body
        }

        Some(Statement::Switch(SwitchStatement { value, cases, default }))
    }

    /// Parse `{ stmts }` — current_token is `{`, leaves current_token on `}`
    fn parse_inner_block(&mut self) -> Option<BlockStatement> {
        self.next_token(); // skip '{'
        let mut statements = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if let Some(s) = self.parse_statement() { statements.push(s); }
            self.next_token();
        }
        Some(BlockStatement { statements })
    }

    // ── match expr { pattern => body, ... } ──────────────────────────────────

    /// Called when current_token == KwMatch. Returns Expression::Match.
    fn parse_match_expression(&mut self) -> Option<Expression> {
        // Advance past 'match' to the subject expression
        self.next_token();
        let subject = self.parse_expression(Precedence::Lowest)?;
        // Now current = last token of subject, peek = '{'
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{' after match subject");
            return None;
        }
        self.next_token(); // current = '{'
        self.next_token(); // current = first token inside match body

        let mut arms = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            // Parse pattern (possibly OR-ed with '|')
            let pattern = self.parse_match_pattern()?;

            // Optional guard: if expr
            let guard = if self.peek_token.token_type == TokenType::If {
                self.next_token(); // consume 'if'
                self.next_token(); // start of guard expression
                let g = self.parse_expression(Precedence::Lowest)?;
                Some(Box::new(g))
            } else {
                None
            };

            // Expect '=>'
            if self.peek_token.token_type != TokenType::Arrow {
                self.parser_error("Expected '=>' in match arm");
                return None;
            }
            self.next_token(); // current = '=>'
            self.next_token(); // current = first token of body

            // Parse body: block or single expression
            let body = if self.current_token.token_type == TokenType::LBrace {
                self.parse_inner_block()?  // current ends on '}'
            } else {
                let expr = self.parse_expression(Precedence::Lowest)?;
                BlockStatement { statements: vec![Statement::Expression(expr)] }
            };

            // Optional trailing ','
            if self.peek_token.token_type == TokenType::Comma {
                self.next_token(); // current = ','
            }

            arms.push(MatchArm { pattern, guard, body });

            // Advance to next arm or closing '}'
            if self.peek_token.token_type == TokenType::RBrace {
                self.next_token(); // current = '}'
                break;
            }
            if self.current_token.token_type != TokenType::RBrace {
                self.next_token();
            }
        }

        Some(Expression::Match(Box::new(MatchExpression { subject: Box::new(subject), arms })))
    }

    /// Parse one match pattern (which may be pat | pat | ...).
    fn parse_match_pattern(&mut self) -> Option<MatchPattern> {
        let first = self.parse_single_match_pattern()?;
        if self.peek_token.token_type != TokenType::BitOr {
            return Some(first);
        }
        let mut pats = vec![first];
        while self.peek_token.token_type == TokenType::BitOr {
            self.next_token(); // '|'
            self.next_token(); // start of next pattern
            pats.push(self.parse_single_match_pattern()?);
        }
        Some(MatchPattern::Or(pats))
    }

    /// Parse a single non-OR match pattern.
    fn parse_single_match_pattern(&mut self) -> Option<MatchPattern> {
        match self.current_token.token_type {
            TokenType::Ident if self.current_token.literal == "_" => Some(MatchPattern::Wildcard),
            TokenType::Ident if self.peek_token.token_type == TokenType::Dot => {
                // Enum.Variant pattern — e.g. Direction.North
                let name = self.current_token.literal.clone();
                self.next_token(); // consume '.'
                if !self.peek_token_is_name() {
                    self.parser_error("Expected variant name after '.' in match pattern");
                    return None;
                }
                self.next_token(); // advance to variant name
                let variant = self.current_token.literal.clone();
                let expr = Expression::DotCall(DotCallExpression {
                    object: Box::new(Expression::Identifier(name)),
                    method: variant,
                    arguments: vec![],
                    has_parens: false,
                    is_optional: false,
                    line: 0,
                    column: 0,
                });
                Some(MatchPattern::Literal(expr))
            }
            TokenType::Ident => Some(MatchPattern::Binding(self.current_token.literal.clone())),
            TokenType::Int => {
                let n: i64 = self.current_token.literal.parse().ok()?;
                Some(MatchPattern::Literal(Expression::Integer(n)))
            }
            TokenType::Minus => {
                // Negative literal: -42
                self.next_token();
                if self.current_token.token_type != TokenType::Int {
                    self.parser_error("Expected integer after '-' in match pattern");
                    return None;
                }
                let n: i64 = self.current_token.literal.parse().ok()?;
                Some(MatchPattern::Literal(Expression::Integer(-n)))
            }
            TokenType::Decimal => {
                let n: f64 = self.current_token.literal.parse().ok()?;
                Some(MatchPattern::Literal(Expression::Decimal(n)))
            }
            TokenType::Dec => {
                let d = parse_dec_literal(&self.current_token.literal)?;
                Some(MatchPattern::Literal(Expression::Dec(d)))
            }
            TokenType::String => Some(MatchPattern::Literal(Expression::String(self.current_token.literal.clone()))),
            TokenType::RawString => Some(MatchPattern::Literal(Expression::String(self.current_token.literal.clone()))),
            TokenType::True  => Some(MatchPattern::Literal(Expression::Boolean(true))),
            TokenType::False => Some(MatchPattern::Literal(Expression::Boolean(false))),
            TokenType::KwNull => Some(MatchPattern::Literal(Expression::Null)),
            _ => {
                self.parser_error(&format!("Unexpected token '{}' in match pattern", self.current_token.literal));
                None
            }
        }
    }

    // ── try { } catch (e) { } finally { } ────────────────────────────────────
    fn parse_try_statement(&mut self) -> Option<Statement> {
        // try { body }
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after 'try'");
            return None;
        }
        self.next_token(); // '{'
        let body = self.parse_inner_block()?;

        let mut catch_var: Option<String> = None;
        let mut catch_body: Option<BlockStatement> = None;
        let mut finally_body: Option<BlockStatement> = None;

        // optional: catch (e) { }
        if self.peek_token.token_type == TokenType::KwCatch {
            self.next_token(); // 'catch'
            if self.peek_token.token_type == TokenType::LParen {
                self.next_token(); // '('
                self.next_token(); // variable name or ')'
                if self.current_token.token_type == TokenType::Ident {
                    catch_var = Some(self.current_token.literal.clone());
                    self.next_token(); // ')'
                }
            }
            if self.peek_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' after catch");
                return None;
            }
            self.next_token(); // '{'
            catch_body = Some(self.parse_inner_block()?);
        }

        // optional: finally { }
        if self.peek_token.token_type == TokenType::KwFinally {
            self.next_token(); // 'finally'
            if self.peek_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' after 'finally'");
                return None;
            }
            self.next_token(); // '{'
            finally_body = Some(self.parse_inner_block()?);
        }

        if catch_body.is_none() && finally_body.is_none() {
            self.parser_error("'try' must have at least one 'catch' or 'finally' block");
            return None;
        }

        Some(Statement::Try(TryStatement { body, catch_var, catch_body, finally_body }))
    }

    // ── throw expr; ───────────────────────────────────────────────────────────
    fn parse_throw_statement(&mut self) -> Option<Statement> {
        self.next_token(); // first token of expr
        let expr = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type == TokenType::Semicolon { self.next_token(); }
        Some(Statement::Throw(expr))
    }

    // ── enum declaration ──────────────────────────────────────────────────────
    fn parse_enum_declaration(&mut self) -> Option<Statement> {
        // current = 'enum'
        let line = self.current_token.line;
        let column = self.current_token.column;
        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected enum name after 'enum'");
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();
        if self.is_reserved_name(&name) {
            self.parser_error(&format!("'{}' is a reserved system namespace and cannot be used as an enum name", name));
            return None;
        }

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after enum name");
            return None;
        }
        self.next_token(); // '{'
        self.next_token(); // first variant or '}'

        let mut variants = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error(&format!("Expected variant name in enum body, got '{}'", self.current_token.literal));
                return None;
            }
            variants.push(self.current_token.literal.clone());
            if self.peek_token.token_type == TokenType::Comma {
                self.next_token(); // ','
                if self.peek_token.token_type == TokenType::RBrace {
                    self.next_token();
                    break;
                }
                self.next_token(); // next variant
            } else if self.peek_token.token_type == TokenType::RBrace {
                self.next_token();
                break;
            } else {
                self.parser_error("Expected ',' or '}}' in enum body");
                return None;
            }
        }

        Some(Statement::EnumDeclaration(EnumDeclaration { name, variants, line, column }))
    }

    // ── labeled loop: label: while(...) { } ──────────────────────────────────
    fn parse_labeled_statement(&mut self) -> Option<Statement> {
        // current = Ident (label), peek = ':'
        let label = self.current_token.literal.clone();
        self.next_token(); // ':'
        self.next_token(); // while / for / ...

        match self.current_token.token_type {
            TokenType::While => self.parse_while_statement_with_label(Some(label)),
            TokenType::For   => self.parse_for_statement_with_label(Some(label)),
            _ => {
                // Fall back: not a labeled loop, re-interpret as assign
                self.parser_error(&format!("Expected 'while' or 'for' after label '{}'", label));
                None
            }
        }
    }
}

fn is_type_keyword(token_type: &TokenType) -> bool {
    matches!(
        token_type,
        TokenType::KwVoid
            | TokenType::KwInt
            | TokenType::KwDecimal
            | TokenType::KwDec
            | TokenType::KwString
            | TokenType::KwBool
            | TokenType::KwAny
    )
}

/// Parse a `dec` literal lexeme (the `m` suffix is already stripped). Handles
/// both plain (`12.50`) and scientific (`1e-7`) forms via rust_decimal.
fn parse_dec_literal(lit: &str) -> Option<rust_decimal::Decimal> {
    if lit.contains('e') || lit.contains('E') {
        rust_decimal::Decimal::from_scientific(lit).ok()
    } else {
        lit.parse::<rust_decimal::Decimal>().ok()
    }
}

impl Parser {
    // Reads current token as a base type and optionally appends '?' if peek is Question.
    // Assumes caller already verified current is a type keyword.
    fn parse_type_string(&mut self) -> Option<String> {
        let base = self.current_token.literal.clone();
        if self.peek_token.token_type == TokenType::Question {
            self.next_token();
            Some(format!("{}?", base))
        } else {
            Some(base)
        }
    }
}

fn parse_interpolated_string(raw: &str) -> Option<Expression> {
    use crate::lexer::Lexer;
    let mut parts: Vec<StringPart> = Vec::new();
    let mut rest = raw;

    while let Some(open) = rest.find('{') {
        if open > 0 {
            // \x01 is the sentinel for \{ (escaped brace) — restore it as a literal {
            parts.push(StringPart::Literal(rest[..open].replace('\x01', "{")));
        }
        let after_open = &rest[open + 1..];
        // Find the matching '}', skipping nested braces and inner strings
        let close = {
            let mut depth: usize = 0;
            let mut in_str = false;
            let mut found = None;
            for (i, c) in after_open.char_indices() {
                if in_str {
                    if c == '"' { in_str = false; }
                } else {
                    match c {
                        '"' => in_str = true,
                        '{' => depth += 1,
                        '}' if depth > 0 => depth -= 1,
                        '}' => { found = Some(i); break; }
                        _ => {}
                    }
                }
            }
            match found {
                Some(c) => c,
                None => {
                    eprintln!("❌ PARSER ERROR: Unclosed '{{' in string interpolation");
                    return None;
                }
            }
        };
        let expr_src = after_open[..close].trim();
        if !expr_src.is_empty() {
            let lexer = Lexer::new(expr_src.to_string());
            let mut sub = Parser::new(lexer);
            let expr = sub.parse_expression(Precedence::Lowest)?;
            parts.push(StringPart::Expr(Box::new(expr)));
        }
        rest = &after_open[close + 1..];
    }

    if !rest.is_empty() {
        parts.push(StringPart::Literal(rest.replace('\x01', "{")));
    }

    if parts.len() == 1 {
        if let StringPart::Literal(ref s) = parts[0] {
            return Some(Expression::String(s.clone()));
        }
    }

    Some(Expression::InterpolatedString(parts))
}
