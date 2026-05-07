use crate::ast::*;
use crate::evaluator::Object;
use crate::lexer::Lexer;
use crate::token::{Token, TokenType};

#[derive(PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest,
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
        TokenType::Eq | TokenType::NotEq => Precedence::Equals,
        TokenType::Lt | TokenType::Gt => Precedence::LessGreater,
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
            if let Some(stmt) = self.parse_statement() {
                program.statements.push(stmt);
            }
            self.next_token();
        }
        program
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        match self.current_token.token_type {
            TokenType::Let => self.parse_let_statement(),
            // Para poder escribir `1 + 1;` sin `let` se requiere parse_expression_statement.
            // Por ahora, solo soportamos `let`.
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_expression_statement(&mut self) -> Option<Statement> {
        // Utilizamos la función que ya sabe leer variables, números y arrays
        // (Nota: Si cambiaste su nombre, usa el que le hayas puesto, ej: parse_expression)
        let expr = self.parse_expression(Precedence::Lowest)?;

        // Si el usuario pone un punto y coma al final (ej: "ii;"), lo consumimos
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Expression(expr))
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
                | TokenType::Gt => true,
                _ => false,
            };

            if !is_infix {
                return left_exp;
            }

            self.next_token();

            let operator = self.current_token.literal.clone();
            let current_precedence = self.current_precedence();

            self.next_token(); // Avanzamos al valor de la derecha

            if let Some(left) = left_exp {
                if let Some(right) = self.parse_expression(current_precedence) {
                    left_exp = Some(Expression::Infix(Box::new(left), operator, Box::new(right)));
                } else {
                    return None;
                }
            } else {
                return None;
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
                println!("❌ ERROR PARSER: Faltó cerrar el array con ']' o falta una coma ','");
                return None;
            }

            self.next_token(); // Avanzamos a ','
            self.next_token(); // Avanzamos a la siguiente expresión
        }

        Some(Expression::ArrayLiteral(elements))
    }
}
