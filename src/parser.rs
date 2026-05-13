use crate::ast::*;
use crate::lexer::Lexer;
use crate::token::{Token, TokenType};

#[derive(PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest,
    LogicalOr,   // ||
    LogicalAnd,  // &&
    Equals,      // ==
    LessGreater, // > or <
    Sum,         // +
    Product,     // *
    Prefix,      // -X or !X
    Call,        // myFunction(X)
    Index,       // array[index]
}

pub fn token_precedence(token_type: &TokenType) -> Precedence {
    match token_type {
        TokenType::Or => Precedence::LogicalOr,
        TokenType::And => Precedence::LogicalAnd,
        TokenType::Eq | TokenType::NotEq => Precedence::Equals,
        TokenType::Lt | TokenType::Gt | TokenType::LtEq | TokenType::GtEq => Precedence::LessGreater,
        TokenType::Plus | TokenType::Minus => Precedence::Sum,
        TokenType::Slash | TokenType::Asterisk | TokenType::Percent => Precedence::Product,
        TokenType::LParen => Precedence::Call,
        TokenType::Dot => Precedence::Call,
        TokenType::LBracket => Precedence::Index,
        _ => Precedence::Lowest,
    }
}

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    peek_token: Token,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Parser {
        let current_token = lexer.next_token();
        let peek_token = lexer.next_token();
        Parser {
            lexer,
            current_token,
            peek_token,
        }
    }

    pub fn next_token(&mut self) {
        self.current_token = self.peek_token.clone();
        self.peek_token = self.lexer.next_token();
    }

    fn peek_precedence(&self) -> Precedence {
        token_precedence(&self.peek_token.token_type)
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
                | TokenType::For => return,
                _ => self.next_token(),
            }
        }
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        match self.current_token.token_type {
            TokenType::Let => self.parse_let_statement(),
            TokenType::Return => self.parse_return_statement(),
            TokenType::Out => self.parse_out_statement(),
            TokenType::LBrace => self.parse_block_statement(),
            TokenType::Function => self.parse_function_statement(),
            TokenType::While => self.parse_while_statement(),
            TokenType::For => self.parse_for_statement(),
            TokenType::Ident if self.peek_token.token_type == TokenType::Assign => {
                self.parse_assign_statement()
            }
            TokenType::Ident if self.peek_token.token_type == TokenType::LBracket => {
                self.parse_index_assign_or_expr_statement()
            }
            _ => self.parse_expression_statement(),
        }
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

    fn parse_expression_statement(&mut self) -> Option<Statement> {
        let expr = self.parse_expression(Precedence::Lowest)?;

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
        self.next_token();

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

    fn parse_while_statement(&mut self) -> Option<Statement> {
        if self.peek_token.token_type != TokenType::LParen {
            eprintln!("❌ PARSER ERROR: Expected '(' after 'while'");
            return None;
        }
        self.next_token();
        self.next_token();

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RParen {
            eprintln!("❌ PARSER ERROR: Expected ')' after condition in 'while'");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            eprintln!("❌ PARSER ERROR: Expected '{{' to start 'while' body");
            return None;
        }
        self.next_token();

        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Statement::While(WhileStatement { condition, body }))
    }

    fn parse_index_assign_or_expr_statement(&mut self) -> Option<Statement> {
        let expr = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Assign {
            let (target, index) = match &expr {
                Expression::Index(idx_expr) => {
                    let target = match idx_expr.left.as_ref() {
                        Expression::Identifier(name) => name.clone(),
                        _ => {
                            eprintln!("❌ PARSER ERROR: Index assignment target must be a simple array variable");
                            return None;
                        }
                    };
                    let index = *idx_expr.index.clone();
                    (target, index)
                }
                _ => {
                    eprintln!("❌ PARSER ERROR: Left side of '=' is not an index expression");
                    return None;
                }
            };

            self.next_token(); // '='
            self.next_token(); // first token of value

            let value = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }

            Some(Statement::IndexAssign(IndexAssignStatement {
                target,
                index,
                value,
            }))
        } else {
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }
            Some(Statement::Expression(expr))
        }
    }

    fn parse_for_statement(&mut self) -> Option<Statement> {
        if self.peek_token.token_type != TokenType::LParen {
            eprintln!("❌ PARSER ERROR: Expected '(' after 'for'");
            return None;
        }
        self.next_token();
        self.next_token();

        if self.current_token.token_type != TokenType::Let {
            eprintln!("❌ PARSER ERROR: Expected 'let' as for-loop initializer");
            return None;
        }
        let init = match self.parse_let_statement()? {
            Statement::Let(l) => l,
            _ => return None,
        };
        if self.current_token.token_type != TokenType::Semicolon {
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            } else {
                eprintln!("❌ PARSER ERROR: Expected ';' after for-loop initializer");
                return None;
            }
        }
        self.next_token();

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::Semicolon {
            eprintln!("❌ PARSER ERROR: Expected ';' after for-loop condition");
            return None;
        }
        self.next_token();
        self.next_token();

        if self.current_token.token_type != TokenType::Ident
            || self.peek_token.token_type != TokenType::Assign
        {
            eprintln!("❌ PARSER ERROR: Expected assignment as for-loop update");
            return None;
        }
        let update = match self.parse_assign_statement()? {
            Statement::Assign(a) => a,
            _ => return None,
        };

        if self.peek_token.token_type != TokenType::RParen {
            eprintln!("❌ PARSER ERROR: Expected ')' after for-loop update");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            eprintln!("❌ PARSER ERROR: Expected '{{' to start for-loop body");
            return None;
        }
        self.next_token();

        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Statement::For(ForStatement {
            init,
            condition,
            update,
            body,
        }))
    }

    fn parse_if_expression(&mut self) -> Option<Expression> {
        if self.peek_token.token_type != TokenType::LParen {
            eprintln!("❌ PARSER ERROR: Expected '(' after 'if'");
            return None;
        }
        self.next_token();
        self.next_token();

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RParen {
            eprintln!("❌ PARSER ERROR: Expected ')' after 'if' condition");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            eprintln!("❌ PARSER ERROR: Expected '{{' to start 'if' consequence");
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
                    eprintln!("❌ PARSER ERROR: Expected '{{' or 'if' after 'else'");
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
        if self.peek_token.token_type != TokenType::Ident {
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();

        if self.peek_token.token_type == TokenType::Lt {
            let (key_type, value_type) = self.parse_dict_type_annotation()?;

            if self.peek_token.token_type != TokenType::Assign {
                eprintln!("❌ PARSER ERROR: Expected '=' after dict type annotation");
                return None;
            }
            self.next_token();
            self.next_token();

            if self.current_token.token_type != TokenType::LParen {
                eprintln!("❌ PARSER ERROR: Expected '(' to start dict literal");
                return None;
            }

            let value = self.parse_dict_literal(key_type, value_type)?;

            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }

            return Some(Statement::Let(LetStatement { name, value }));
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

        Some(Statement::Let(LetStatement { name, value }))
    }

    fn parse_function_statement(&mut self) -> Option<Statement> {
        let mut return_type = None;
        if is_type_keyword(&self.peek_token.token_type) {
            self.next_token();
            return_type = Some(self.current_token.literal.clone());
        }

        if self.peek_token.token_type == TokenType::Ident {
            self.next_token();
            let name = self.current_token.literal.clone();

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

            let function = FunctionLiteral { return_type, parameters, body };

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

            let function = FunctionLiteral { return_type, parameters, body };

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

            if is_type_keyword(&self.current_token.token_type) {
                type_name = Some(self.current_token.literal.clone());
                self.next_token();
            }

            let name = if self.current_token.token_type == TokenType::Ident {
                self.current_token.literal.clone()
            } else {
                return None;
            };

            parameters.push(Parameter { name, type_name });

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
        let return_type = Some(self.current_token.literal.clone());

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

        Some(Expression::FunctionLiteral(FunctionLiteral { return_type, parameters, body }))
    }

    fn parse_call_arguments(&mut self) -> Option<Vec<Expression>> {
        let mut args = Vec::new();

        if self.peek_token.token_type == TokenType::RParen {
            self.next_token();
            return Some(args);
        }

        self.next_token();

        if let Some(expr) = self.parse_expression(Precedence::Lowest) {
            args.push(expr);
        }

        while self.peek_token.token_type == TokenType::Comma {
            self.next_token();
            self.next_token();
            if let Some(expr) = self.parse_expression(Precedence::Lowest) {
                args.push(expr);
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
                | TokenType::LParen
                | TokenType::Dot
                | TokenType::LBracket => true,
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
                            eprintln!("❌ PARSER ERROR: Expected ']' after array index");
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
            } else if self.current_token.token_type == TokenType::Dot {
                let dot_line = self.current_token.line;
                let dot_column = self.current_token.column;

                if self.peek_token.token_type != TokenType::Ident {
                    eprintln!("❌ PARSER ERROR: Expected method name after '.'");
                    return left_exp;
                }
                self.next_token();
                let method = self.current_token.literal.clone();

                let arguments = if self.peek_token.token_type == TokenType::LParen {
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
                        line: dot_line,
                        column: dot_column,
                    }));
                }
            } else {
                let op_line = self.current_token.line;
                let op_column = self.current_token.column;

                self.next_token();

                if let Some(left) = left_exp {
                    if let Some(right) = self.parse_expression(current_precedence) {
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

            TokenType::String => {
                let s = self.current_token.literal.clone();
                if s.contains('{') {
                    parse_interpolated_string(&s)
                } else {
                    Some(Expression::String(s))
                }
            }

            TokenType::True => Some(Expression::Boolean(true)),
            TokenType::False => Some(Expression::Boolean(false)),

            TokenType::Bang | TokenType::Minus => {
                let operator = self.current_token.literal.clone();
                self.next_token();
                let right = self.parse_expression(Precedence::Prefix)?;
                Some(Expression::Prefix(operator, Box::new(right)))
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
                                eprintln!("❌ PARSER ERROR: Expected identifier in lambda parameters");
                                return None;
                            }
                            params.push(self.current_token.literal.clone());
                        }
                        if self.peek_token.token_type != TokenType::RParen {
                            eprintln!("❌ PARSER ERROR: Expected ')' after lambda parameters");
                            return None;
                        }
                        self.next_token(); // ')'
                        if self.peek_token.token_type != TokenType::Arrow {
                            eprintln!("❌ PARSER ERROR: Expected '=>' after lambda parameters");
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

                    // (ident op ...) — grouped expression starting with an identifier
                    _ => {
                        let first = Some(Expression::Identifier(first_name));
                        let inner = self.parse_infix_chain(first, Precedence::Lowest)?;
                        if self.peek_token.token_type != TokenType::RParen {
                            eprintln!("❌ PARSER ERROR: Expected ')' in grouped expression");
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
            TokenType::LBrace => self.parse_entry_literal(),
            TokenType::If => self.parse_if_expression(),

            TokenType::KwVoid
            | TokenType::KwInt
            | TokenType::KwDecimal
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

                Some(Expression::FunctionLiteral(FunctionLiteral { return_type, parameters, body }))
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
            eprintln!("❌ PARSER ERROR: Expected type keyword for dict key type, got '{}'", self.current_token.literal);
            return None;
        }
        let key_type = self.current_token.literal.clone();

        if self.peek_token.token_type != TokenType::Comma {
            eprintln!("❌ PARSER ERROR: Expected ',' between key and value types in dict annotation");
            return None;
        }
        self.next_token(); // ','
        self.next_token(); // value_type

        if !is_type_keyword(&self.current_token.token_type) {
            eprintln!("❌ PARSER ERROR: Expected type keyword for dict value type, got '{}'", self.current_token.literal);
            return None;
        }
        let value_type = self.current_token.literal.clone();

        if self.peek_token.token_type != TokenType::Gt {
            eprintln!("❌ PARSER ERROR: Expected '>' to close dict type annotation");
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
                eprintln!("❌ PARSER ERROR: Expected '{{' to start dict entry");
                return None;
            }
            self.next_token();

            let key = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type != TokenType::Comma {
                eprintln!("❌ PARSER ERROR: Expected ',' between key and value in dict entry");
                return None;
            }
            self.next_token();
            self.next_token();

            let value = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type != TokenType::RBrace {
                eprintln!("❌ PARSER ERROR: Expected '}}' to close dict entry");
                return None;
            }
            self.next_token(); // '}'

            entries.push((key, value));

            if self.peek_token.token_type == TokenType::RParen {
                self.next_token();
                break;
            }

            if self.peek_token.token_type != TokenType::Comma {
                eprintln!("❌ PARSER ERROR: Expected ',' or ')' after dict entry");
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
            eprintln!("❌ PARSER ERROR: Expected ',' between key and value in entry literal");
            return None;
        }
        self.next_token();
        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RBrace {
            eprintln!("❌ PARSER ERROR: Expected '}}' to close entry literal");
            return None;
        }
        self.next_token();

        Some(Expression::EntryLiteral(Box::new(key), Box::new(value)))
    }

    fn parse_array_literal(&mut self) -> Option<Expression> {
        let mut elements = Vec::new();

        if self.peek_token.token_type == TokenType::RBracket {
            self.next_token();
            return Some(Expression::ArrayLiteral(elements));
        }

        self.next_token();

        loop {
            let expr = self.parse_expression(Precedence::Lowest);

            if let Some(e) = expr {
                elements.push(e);
            }

            if self.peek_token.token_type == TokenType::RBracket {
                self.next_token();
                break;
            }

            if self.peek_token.token_type != TokenType::Comma {
                eprintln!("❌ PARSER ERROR: Missing closing bracket ']' or comma ',' in array");
                return None;
            }

            self.next_token();
            self.next_token();
        }

        Some(Expression::ArrayLiteral(elements))
    }
}

fn is_type_keyword(token_type: &TokenType) -> bool {
    matches!(
        token_type,
        TokenType::KwVoid
            | TokenType::KwInt
            | TokenType::KwDecimal
            | TokenType::KwString
            | TokenType::KwBool
            | TokenType::KwAny
    )
}

fn parse_interpolated_string(raw: &str) -> Option<Expression> {
    use crate::lexer::Lexer;
    let mut parts: Vec<StringPart> = Vec::new();
    let mut rest = raw;

    while let Some(open) = rest.find('{') {
        if open > 0 {
            parts.push(StringPart::Literal(rest[..open].to_string()));
        }
        let after_open = &rest[open + 1..];
        let close = match after_open.find('}') {
            Some(c) => c,
            None => {
                eprintln!("❌ PARSER ERROR: Unclosed '{{' in string interpolation");
                return None;
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
        parts.push(StringPart::Literal(rest.to_string()));
    }

    if parts.len() == 1 {
        if let StringPart::Literal(ref s) = parts[0] {
            return Some(Expression::String(s.clone()));
        }
    }

    Some(Expression::InterpolatedString(parts))
}
