use crate::token::{self, Token, TokenType};

pub struct Lexer {
    input: String,
    position: usize,      // byte offset of current char (self.ch)
    read_position: usize, // byte offset of next char to read
    ch: char,             // current char under examination
    line: usize,
    column: usize,
}

impl Lexer {
    pub fn new(input: String) -> Self {
        let mut l = Lexer {
            input,
            position: 0,
            read_position: 0,
            ch: '\0',
            line: 1,
            column: 0,
        };
        l.read_char();
        l
    }

    pub fn read_char(&mut self) {
        if self.read_position >= self.input.len() {
            self.ch = '\0';
            self.position = self.read_position;
            self.read_position += 1;
        } else {
            if self.ch == '\n' {
                self.line += 1;
                self.column = 0;
            }
            let c = self.input[self.read_position..].chars().next().unwrap();
            self.ch = c;
            self.column += 1;
            self.position = self.read_position;
            self.read_position += c.len_utf8();
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        let token = match self.ch {
            '=' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::Eq, "==".to_string(), self.line, self.column)
                } else if self.peek_char() == '>' {
                    self.read_char();
                    Token::new(TokenType::Arrow, "=>".to_string(), self.line, self.column)
                } else {
                    Token::new(
                        TokenType::Assign,
                        self.ch.to_string(),
                        self.line,
                        self.column,
                    )
                }
            }
            '+' => Token::new(TokenType::Plus, self.ch.to_string(), self.line, self.column),
            '-' => Token::new(
                TokenType::Minus,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            '!' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::NotEq, "!=".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Bang, self.ch.to_string(), self.line, self.column)
                }
            }
            '/' => {
                if self.peek_char() == '/' {
                    self.skip_comment();
                    return self.next_token();
                } else {
                    Token::new(
                        TokenType::Slash,
                        self.ch.to_string(),
                        self.line,
                        self.column,
                    )
                }
            }
            '*' => Token::new(
                TokenType::Asterisk,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            '%' => Token::new(
                TokenType::Percent,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            '<' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::LtEq, "<=".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Lt, self.ch.to_string(), self.line, self.column)
                }
            }
            '>' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::GtEq, ">=".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Gt, self.ch.to_string(), self.line, self.column)
                }
            }
            '&' => {
                if self.peek_char() == '&' {
                    self.read_char();
                    Token::new(TokenType::And, "&&".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Illegal, self.ch.to_string(), self.line, self.column)
                }
            }
            '|' => {
                if self.peek_char() == '|' {
                    self.read_char();
                    Token::new(TokenType::Or, "||".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Illegal, self.ch.to_string(), self.line, self.column)
                }
            }
            ';' => Token::new(
                TokenType::Semicolon,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            ',' => Token::new(
                TokenType::Comma,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            '(' => Token::new(
                TokenType::LParen,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            ')' => Token::new(
                TokenType::RParen,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            '{' => Token::new(
                TokenType::LBrace,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            '}' => Token::new(
                TokenType::RBrace,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            '[' => Token::new(
                TokenType::LBracket,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            ']' => Token::new(
                TokenType::RBracket,
                self.ch.to_string(),
                self.line,
                self.column,
            ),
            '.' => Token::new(TokenType::Dot, ".".to_string(), self.line, self.column),
            '?' => Token::new(TokenType::Question, "?".to_string(), self.line, self.column),
            '"' => {
                let literal = self.read_string();
                let start_line = self.line;
                let start_column = self.column;
                Token::new(TokenType::String, literal, start_line, start_column)
            }
            '\0' => Token::new(TokenType::Eof, "".to_string(), self.line, self.column),
            _ => {
                if is_letter(self.ch) {
                    let literal = self.read_identifier();
                    let token_type = token::lookup_ident(&literal);
                    let start_line = self.line;
                    let start_column = self.column;
                    return Token::new(token_type, literal, start_line, start_column);
                } else if is_digit(self.ch) {
                    let start_line = self.line;
                    let start_column = self.column;
                    let literal = self.read_number();
                    let token_type = if literal.contains('.') {
                        TokenType::Decimal
                    } else {
                        TokenType::Int
                    };
                    return Token::new(token_type, literal, start_line, start_column);
                } else {
                    Token::new(
                        TokenType::Illegal,
                        self.ch.to_string(),
                        self.line,
                        self.column,
                    )
                }
            }
        };

        self.read_char();
        token
    }

    fn read_string(&mut self) -> String {
        // self.ch == '"' (opening quote); read_position points to first content byte
        let start = self.read_position;
        loop {
            self.read_char();
            if self.ch == '"' || self.ch == '\0' {
                break;
            }
        }
        // self.position == byte offset of closing '"' (or EOF)
        self.input[start..self.position].to_string()
    }

    fn read_identifier(&mut self) -> String {
        let start = self.position;
        self.read_char();
        while is_letter(self.ch) || is_digit(self.ch) {
            self.read_char();
        }
        // self.position == byte offset of first non-identifier char
        self.input[start..self.position].to_string()
    }

    fn read_number(&mut self) -> String {
        let start = self.position;
        while is_digit(self.ch) {
            self.read_char();
        }
        // Consume decimal part when '.' is followed by a digit
        if self.ch == '.' {
            let next_is_digit = self.input[self.read_position..]
                .chars()
                .next()
                .map_or(false, is_digit);
            if next_is_digit {
                self.read_char(); // consume '.'
                while is_digit(self.ch) {
                    self.read_char();
                }
            }
        }
        self.input[start..self.position].to_string()
    }

    fn skip_whitespace(&mut self) {
        while self.ch == ' ' || self.ch == '\t' || self.ch == '\n' || self.ch == '\r' {
            self.read_char();
        }
    }

    fn peek_char(&self) -> char {
        if self.read_position >= self.input.len() {
            '\0'
        } else {
            self.input[self.read_position..].chars().next().unwrap()
        }
    }

    fn skip_comment(&mut self) {
        while self.ch != '\n' && self.ch != '\0' {
            self.read_char();
        }
        self.skip_whitespace();
    }
}

fn is_letter(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

fn is_digit(ch: char) -> bool {
    ch.is_numeric()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::TokenType;

    #[test]
    fn test_next_token() {
        let input = "let five = 5;
let ten = 10;

let add = fn(x, y) {
  x + y;
};

let result = add(five, ten);
!-/*5;
5 < 10 > 5;

if (5 < 10) {
	return true;
} else {
	return false;
}

10 == 10;
10 != 9;
\"foobar\"
\"foo bar\"
";

        let tests = vec![
            (TokenType::Let, "let"),
            (TokenType::Ident, "five"),
            (TokenType::Assign, "="),
            (TokenType::Int, "5"),
            (TokenType::Semicolon, ";"),
            (TokenType::Let, "let"),
            (TokenType::Ident, "ten"),
            (TokenType::Assign, "="),
            (TokenType::Int, "10"),
            (TokenType::Semicolon, ";"),
            (TokenType::Let, "let"),
            (TokenType::Ident, "add"),
            (TokenType::Assign, "="),
            (TokenType::Function, "fn"),
            (TokenType::LParen, "("),
            (TokenType::Ident, "x"),
            (TokenType::Comma, ","),
            (TokenType::Ident, "y"),
            (TokenType::RParen, ")"),
            (TokenType::LBrace, "{"),
            (TokenType::Ident, "x"),
            (TokenType::Plus, "+"),
            (TokenType::Ident, "y"),
            (TokenType::Semicolon, ";"),
            (TokenType::RBrace, "}"),
            (TokenType::Semicolon, ";"),
            (TokenType::Let, "let"),
            (TokenType::Ident, "result"),
            (TokenType::Assign, "="),
            (TokenType::Ident, "add"),
            (TokenType::LParen, "("),
            (TokenType::Ident, "five"),
            (TokenType::Comma, ","),
            (TokenType::Ident, "ten"),
            (TokenType::RParen, ")"),
            (TokenType::Semicolon, ";"),
            (TokenType::Bang, "!"),
            (TokenType::Minus, "-"),
            (TokenType::Slash, "/"),
            (TokenType::Asterisk, "*"),
            (TokenType::Int, "5"),
            (TokenType::Semicolon, ";"),
            (TokenType::Int, "5"),
            (TokenType::Lt, "<"),
            (TokenType::Int, "10"),
            (TokenType::Gt, ">"),
            (TokenType::Int, "5"),
            (TokenType::Semicolon, ";"),
            (TokenType::If, "if"),
            (TokenType::LParen, "("),
            (TokenType::Int, "5"),
            (TokenType::Lt, "<"),
            (TokenType::Int, "10"),
            (TokenType::RParen, ")"),
            (TokenType::LBrace, "{"),
            (TokenType::Return, "return"),
            (TokenType::True, "true"),
            (TokenType::Semicolon, ";"),
            (TokenType::RBrace, "}"),
            (TokenType::Else, "else"),
            (TokenType::LBrace, "{"),
            (TokenType::Return, "return"),
            (TokenType::False, "false"),
            (TokenType::Semicolon, ";"),
            (TokenType::RBrace, "}"),
            (TokenType::Int, "10"),
            (TokenType::Eq, "=="),
            (TokenType::Int, "10"),
            (TokenType::Semicolon, ";"),
            (TokenType::Int, "10"),
            (TokenType::NotEq, "!="),
            (TokenType::Int, "9"),
            (TokenType::Semicolon, ";"),
            (TokenType::String, "foobar"),
            (TokenType::String, "foo bar"),
            (TokenType::Eof, ""),
        ];

        let mut l = Lexer::new(input.to_string());

        for (i, (expected_type, expected_literal)) in tests.iter().enumerate() {
            let tok = l.next_token();
            assert_eq!(
                tok.token_type, *expected_type,
                "tests[{}] - token type wrong. expected={:?}, got={:?}",
                i, expected_type, tok.token_type
            );
            assert_eq!(
                tok.literal, *expected_literal,
                "tests[{}] - literal wrong. expected={:?}, got={:?}",
                i, expected_literal, tok.literal
            );
        }
    }
}
