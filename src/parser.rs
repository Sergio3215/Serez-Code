use crate::ast::*;
use crate::lexer::Lexer;
use crate::token::{Token, TokenType}; // Asegúrate de tener publicas tus estructuras en ast.rs

pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    peek_token: Token,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Parser {
        // Leemos dos tokens para inicializar current_token y peek_token
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

    // Función principal que construye el AST completo
    pub fn parse_program(&mut self) -> Program {
        let mut program = Program {
            statements: Vec::new(),
        };

        // Recorremos los tokens hasta el final del archivo
        while self.current_token.token_type != TokenType::Eof {
            if let Some(stmt) = self.parse_statement() {
                program.statements.push(stmt);
            }
            self.next_token();
        }
        program
    }

    // Decide qué tipo de sentencia construir según el token
    fn parse_statement(&mut self) -> Option<Statement> {
        match self.current_token.token_type {
            TokenType::Let => self.parse_let_statement(),
            // Aquí agregarás TokenType::Return, etc. en el futuro
            _ => None,
        }
    }

    // Construye un nodo LetStatement
    fn parse_let_statement(&mut self) -> Option<Statement> {
        // 1. Verificamos que al 'let' le siga un identificador
        if self.peek_token.token_type != TokenType::Ident {
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();

        // 2. Verificamos que le siga el signo '='
        if self.peek_token.token_type != TokenType::Assign {
            return None;
        }
        self.next_token();

        // 3. Saltamos el '=' y leemos el valor (simplificado para el ejemplo)
        self.next_token();

        let value = match self.current_token.token_type {
            TokenType::Int => {
                let num: i64 = self.current_token.literal.parse().unwrap();
                Expression::Integer(num)
            }
            TokenType::String => Expression::String(self.current_token.literal.clone()),
            TokenType::True => Expression::Boolean(true),
            TokenType::False => Expression::Boolean(false),
            TokenType::LBracket => match self.parse_array_literal() {
                Some(arr) => arr,
                _ => return None,
            },
            _ => return None, // Expresión no soportada temporalmente
        };

        // Si hay un punto y coma, lo consumimos
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Let(LetStatement { name, value }))
    }

    fn parse_array_literal(&mut self) -> Option<Expression> {
        let mut elements = Vec::new();

        // Caso especial: Si el array está vacío (ej: let arr = [];)
        if self.peek_token.token_type == TokenType::RBracket {
            self.next_token(); // Avanzamos al ']'
            return Some(Expression::ArrayLiteral(elements));
        }

        self.next_token(); // Avanzamos al primer elemento dentro del array

        loop {
            // Evaluamos el elemento actual según su tipo
            let expr = match self.current_token.token_type {
                TokenType::Int => {
                    let num: i64 = self.current_token.literal.parse().unwrap();
                    Some(Expression::Integer(num))
                }
                TokenType::String => Some(Expression::String(self.current_token.literal.clone())),
                TokenType::True => Some(Expression::Boolean(true)),
                TokenType::False => Some(Expression::Boolean(false)),
                TokenType::Ident => Some(Expression::Identifier(self.current_token.literal.clone())),
                TokenType::LBracket => self.parse_array_literal(),
                _ => return None, // Tipo no soportado
            };

            if let Some(e) = expr {
                elements.push(e);
            }

            // Si el siguiente token es ']', significa que se terminó el array
            if self.peek_token.token_type == TokenType::RBracket {
                self.next_token(); // Avanzamos al ']'
                break;
            }

            // Si el array no ha terminado, DEBE haber una coma separando el siguiente elemento
            if self.peek_token.token_type != TokenType::Comma {
                println!("❌ ERROR PARSER: Faltó cerrar el array con ']' o falta una coma ','");
                return None;
            }

            self.next_token(); // Avanzamos a la coma ','
            self.next_token(); // Avanzamos al siguiente elemento del array
        }

        Some(Expression::ArrayLiteral(elements))
    }
}
