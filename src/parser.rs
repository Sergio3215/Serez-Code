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
        TokenType::Slash | TokenType::Asterisk => Precedence::Product,
        TokenType::LParen => Precedence::Call,
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

    // Advance past the current malformed statement to the next recovery point
    // so subsequent valid statements can still be parsed.
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
            // Reasignación: Ident seguido de `=`
            TokenType::Ident if self.peek_token.token_type == TokenType::Assign => {
                self.parse_assign_statement()
            }
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_block_statement(&mut self) -> Option<Statement> {
        // current_token = `{`
        self.next_token(); // avanzamos al primer token dentro del bloque

        let mut statements = Vec::new();

        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            self.next_token();
        }
        // current_token = `}` al salir del while

        Some(Statement::Block(BlockStatement { statements }))
    }

    fn parse_expression_statement(&mut self) -> Option<Statement> {
        let expr = self.parse_expression(Precedence::Lowest)?;

        // Si el usuario pone un punto y coma al final (ej: "ii;"), lo consumimos
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Expression(expr))
    }

    fn parse_assign_statement(&mut self) -> Option<Statement> {
        let name = self.current_token.literal.clone(); // el identificador
        self.next_token(); // current: '='
        self.next_token(); // current: primer token del valor

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Assign(AssignStatement { name, value }))
    }

    fn parse_return_statement(&mut self) -> Option<Statement> {
        self.next_token(); // avanzamos sobre la palabra clave 'return'

        let return_value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Return(ReturnStatement { return_value }))
    }

    fn parse_out_statement(&mut self) -> Option<Statement> {
        self.next_token(); // avanzamos sobre la palabra clave 'out'

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Out(OutStatement { value }))
    }

    fn parse_while_statement(&mut self) -> Option<Statement> {
        // current token is 'while'
        if self.peek_token.token_type != TokenType::LParen {
            eprintln!("❌ PARSER ERROR: Expected '(' after 'while'");
            return None;
        }
        self.next_token(); // now '('
        self.next_token(); // first token of condition

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RParen {
            eprintln!("❌ PARSER ERROR: Expected ')' after condition in 'while'");
            return None;
        }
        self.next_token(); // now ')'

        if self.peek_token.token_type != TokenType::LBrace {
            eprintln!("❌ PARSER ERROR: Expected '{{' to start 'while' body");
            return None;
        }
        self.next_token(); // now '{'

        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Statement::While(WhileStatement { condition, body }))
    }

    fn parse_for_statement(&mut self) -> Option<Statement> {
        // current_token = 'for'
        if self.peek_token.token_type != TokenType::LParen {
            eprintln!("❌ PARSER ERROR: Expected '(' after 'for'");
            return None;
        }
        self.next_token(); // now '('
        self.next_token(); // first token of init

        // --- INIT: let name = expr ---
        if self.current_token.token_type != TokenType::Let {
            eprintln!("❌ PARSER ERROR: Expected 'let' as for-loop initializer");
            return None;
        }
        let init = match self.parse_let_statement()? {
            Statement::Let(l) => l,
            _ => return None,
        };
        // parse_let_statement consumed the ';' — current = ';'
        if self.current_token.token_type != TokenType::Semicolon {
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            } else {
                eprintln!("❌ PARSER ERROR: Expected ';' after for-loop initializer");
                return None;
            }
        }
        self.next_token(); // first token of condition

        // --- CONDITION ---
        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::Semicolon {
            eprintln!("❌ PARSER ERROR: Expected ';' after for-loop condition");
            return None;
        }
        self.next_token(); // now ';'
        self.next_token(); // first token of update

        // --- UPDATE: name = expr ---
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
        // parse_assign_statement left current at last token of RHS (peek = ')')

        if self.peek_token.token_type != TokenType::RParen {
            eprintln!("❌ PARSER ERROR: Expected ')' after for-loop update");
            return None;
        }
        self.next_token(); // now ')'

        if self.peek_token.token_type != TokenType::LBrace {
            eprintln!("❌ PARSER ERROR: Expected '{{' to start for-loop body");
            return None;
        }
        self.next_token(); // now '{'

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
        // current token is 'if'
        if self.peek_token.token_type != TokenType::LParen {
            eprintln!("❌ PARSER ERROR: Expected '(' after 'if'");
            return None;
        }
        self.next_token(); // '('
        self.next_token(); // condition start

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RParen {
            eprintln!("❌ PARSER ERROR: Expected ')' after 'if' condition");
            return None;
        }
        self.next_token(); // ')'

        if self.peek_token.token_type != TokenType::LBrace {
            eprintln!("❌ PARSER ERROR: Expected '{{' to start 'if' consequence");
            return None;
        }
        self.next_token(); // '{'

        let consequence = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        let mut alternative = None;

        if self.peek_token.token_type == TokenType::Else {
            self.next_token(); // 'else'

            // Suport for 'else if' or 'else { ... }'
            if self.peek_token.token_type == TokenType::If {
                self.next_token(); // 'if'

                // Wrap 'else if' in a block statement to keep AST simple
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
                self.next_token(); // '{'
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

        if self.peek_token.token_type != TokenType::Assign {
            return None;
        }
        self.next_token(); // current: '=', peek: primer elemento
        self.next_token(); // current: primer elemento de la expresión

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Let(LetStatement { name, value }))
    }

    fn parse_function_statement(&mut self) -> Option<Statement> {
        // Asumimos que current_token == TokenType::Function
        // peek puede ser el tipo de retorno (KwVoid, KwInt, etc) o LParen (si no hay tipo).
        let mut return_type = None;
        if is_type_keyword(&self.peek_token.token_type) {
            self.next_token();
            return_type = Some(self.current_token.literal.clone());
        }

        // Ahora peek puede ser un Identificador (nombre de la función) o un LParen (anónima).
        if self.peek_token.token_type == TokenType::Ident {
            // Es una declaración de función: fn tipo nombre()
            self.next_token();
            let name = self.current_token.literal.clone();

            if self.peek_token.token_type != TokenType::LParen {
                return None;
            }
            self.next_token(); // current_token == LParen

            let parameters = self.parse_function_parameters()?;

            if self.peek_token.token_type != TokenType::LBrace {
                return None;
            }
            self.next_token(); // current_token == LBrace

            let body_stmt = self.parse_block_statement()?;
            let body = match body_stmt {
                Statement::Block(b) => b,
                _ => return None,
            };

            let function = FunctionLiteral {
                return_type,
                parameters,
                body,
            };

            Some(Statement::FunctionDeclaration(FunctionDeclaration {
                name,
                function,
            }))
        } else {
            // Es una función anónima (usada como expresión pero en contexto de sentencia)
            // Retrocedemos la lógica para que sea parseada como expression statement
            // Pero como Pratt Parser no puede retroceder tokens fácilmente, mejor parseamos
            // el FunctionLiteral directamente y lo envolvemos en ExpressionStatement.
            if self.peek_token.token_type != TokenType::LParen {
                return None;
            }
            self.next_token(); // current_token == LParen

            let parameters = self.parse_function_parameters()?;

            if self.peek_token.token_type != TokenType::LBrace {
                return None;
            }
            self.next_token(); // current_token == LBrace

            let body_stmt = self.parse_block_statement()?;
            let body = match body_stmt {
                Statement::Block(b) => b,
                _ => return None,
            };

            let function = FunctionLiteral {
                return_type,
                parameters,
                body,
            };

            Some(Statement::Expression(Expression::FunctionLiteral(function)))
        }
    }

    fn parse_function_parameters(&mut self) -> Option<Vec<Parameter>> {
        // current_token == LParen
        let mut parameters = Vec::new();

        if self.peek_token.token_type == TokenType::RParen {
            self.next_token();
            return Some(parameters);
        }

        self.next_token();

        loop {
            let mut type_name = None;

            // Verificamos si hay un tipo
            if is_type_keyword(&self.current_token.token_type) {
                type_name = Some(self.current_token.literal.clone());
                self.next_token();
            }

            // El token actual debe ser el Identificador
            let name = if self.current_token.token_type == TokenType::Ident {
                self.current_token.literal.clone()
            } else {
                return None;
            };

            parameters.push(Parameter { name, type_name });

            if self.peek_token.token_type == TokenType::Comma {
                self.next_token(); // saltar coma
                self.next_token(); // ir al siguiente parámetro
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
        // current_token == KwVoid | KwInt | KwString | KwBool
        let return_type = Some(self.current_token.literal.clone());

        if self.peek_token.token_type != TokenType::LParen {
            return None;
        }
        self.next_token(); // current_token == LParen

        let parameters = self.parse_function_parameters()?;

        if self.peek_token.token_type != TokenType::Arrow {
            return None;
        }
        self.next_token(); // current_token == Arrow

        if self.peek_token.token_type != TokenType::LBrace {
            return None;
        }
        self.next_token(); // current_token == LBrace

        let body_stmt = self.parse_block_statement()?;
        let body = match body_stmt {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Expression::FunctionLiteral(FunctionLiteral {
            return_type,
            parameters,
            body,
        }))
    }

    fn parse_call_arguments(&mut self) -> Option<Vec<Expression>> {
        // current_token == LParen
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

    fn parse_expression(&mut self, precedence: Precedence) -> Option<Expression> {
        // 1. PREFIX (Funciones de prefijo o valores literales)
        let mut left_exp = match self.current_token.token_type {
            TokenType::Ident => Some(Expression::Identifier(self.current_token.literal.clone())),
            TokenType::Int => {
                if let Ok(num) = self.current_token.literal.parse::<i64>() {
                    Some(Expression::Integer(num))
                } else {
                    None
                }
            }
            TokenType::String => Some(Expression::String(self.current_token.literal.clone())),
            TokenType::True => Some(Expression::Boolean(true)),
            TokenType::False => Some(Expression::Boolean(false)),
            TokenType::Bang | TokenType::Minus => {
                let operator = self.current_token.literal.clone();
                self.next_token();
                let right = self.parse_expression(Precedence::Prefix)?;
                Some(Expression::Prefix(operator, Box::new(right)))
            }
            TokenType::LParen => {
                self.next_token();
                let exp = self.parse_expression(Precedence::Lowest);
                if self.peek_token.token_type != TokenType::RParen {
                    return None;
                }
                self.next_token(); // Consumimos RParen
                exp
            }
            TokenType::LBracket => self.parse_array_literal(),
            TokenType::If => self.parse_if_expression(),
            TokenType::KwVoid | TokenType::KwInt | TokenType::KwString | TokenType::KwBool => {
                // Arrow function syntax: void () => {}
                self.parse_arrow_function()
            }
            TokenType::Function => {
                // Expression form of traditional function: fn void() {}
                // Since parse_function_statement handles both, but here we only want expression
                let mut return_type = None;
                if is_type_keyword(&self.peek_token.token_type) {
                    self.next_token();
                    return_type = Some(self.current_token.literal.clone());
                }

                if self.peek_token.token_type != TokenType::LParen {
                    return None; // Si tiene nombre no es expresión
                }
                self.next_token(); // current = LParen

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
                }))
            }
            _ => None,
        };

        // 2. INFIX (Operadores que vienen después)
        while self.peek_token.token_type != TokenType::Semicolon
            && precedence < self.peek_precedence()
        {
            let is_infix = match self.peek_token.token_type {
                TokenType::Plus
                | TokenType::Minus
                | TokenType::Slash
                | TokenType::Asterisk
                | TokenType::Eq
                | TokenType::NotEq
                | TokenType::Lt
                | TokenType::Gt
                | TokenType::LtEq
                | TokenType::GtEq
                | TokenType::And
                | TokenType::Or
                | TokenType::LParen
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
                    self.next_token(); // Advance to the index expression
                    if let Some(index) = self.parse_expression(Precedence::Lowest) {
                        if self.peek_token.token_type != TokenType::RBracket {
                            eprintln!("❌ PARSER ERROR: Expected ']' after array index");
                            return None;
                        }
                        self.next_token(); // Consume ']'
                        left_exp = Some(Expression::Index(IndexExpression {
                            left: Box::new(left),
                            index: Box::new(index),
                        }));
                    } else {
                        return None;
                    }
                }
            } else {
                let op_line = self.current_token.line;
                let op_column = self.current_token.column;

                self.next_token(); // Avanzamos al valor de la derecha

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

    fn parse_array_literal(&mut self) -> Option<Expression> {
        let mut elements = Vec::new();

        if self.peek_token.token_type == TokenType::RBracket {
            self.next_token();
            return Some(Expression::ArrayLiteral(elements));
        }

        self.next_token();

        loop {
            // Evaluamos la expresión de forma recursiva
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

            self.next_token(); // Avanzamos a ','
            self.next_token(); // Avanzamos a la siguiente expresión
        }

        Some(Expression::ArrayLiteral(elements))
    }
}

fn is_type_keyword(token_type: &TokenType) -> bool {
    matches!(
        token_type,
        TokenType::KwVoid | TokenType::KwInt | TokenType::KwString | TokenType::KwBool
    )
}
