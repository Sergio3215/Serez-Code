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
            '+' => {
                if self.peek_char() == '+' {
                    self.read_char();
                    Token::new(TokenType::PlusPlus, "++".to_string(), self.line, self.column)
                } else if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::PlusEq, "+=".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Plus, "+".to_string(), self.line, self.column)
                }
            }
            '-' => {
                if self.peek_char() == '-' {
                    self.read_char();
                    Token::new(TokenType::MinusMinus, "--".to_string(), self.line, self.column)
                } else if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::MinusEq, "-=".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Minus, "-".to_string(), self.line, self.column)
                }
            }
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
                    self.skip_line_comment();
                    return self.next_token();
                } else if self.peek_char() == '*' {
                    self.skip_block_comment();
                    return self.next_token();
                } else if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::SlashEq, "/=".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Slash, "/".to_string(), self.line, self.column)
                }
            }
            '*' => {
                if self.peek_char() == '*' {
                    self.read_char();
                    Token::new(TokenType::Power, "**".to_string(), self.line, self.column)
                } else if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::StarEq, "*=".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Asterisk, "*".to_string(), self.line, self.column)
                }
            }
            '%' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::PercentEq, "%=".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Percent, "%".to_string(), self.line, self.column)
                }
            }
            '<' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::LtEq, "<=".to_string(), self.line, self.column)
                } else if self.peek_char() == '<' {
                    self.read_char();
                    Token::new(TokenType::Shl, "<<".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Lt, self.ch.to_string(), self.line, self.column)
                }
            }
            '>' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::GtEq, ">=".to_string(), self.line, self.column)
                } else if self.peek_char() == '>' {
                    self.read_char();
                    Token::new(TokenType::Shr, ">>".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Gt, self.ch.to_string(), self.line, self.column)
                }
            }
            '&' => {
                if self.peek_char() == '&' {
                    self.read_char();
                    Token::new(TokenType::And, "&&".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::BitAnd, "&".to_string(), self.line, self.column)
                }
            }
            '|' => {
                if self.peek_char() == '|' {
                    self.read_char();
                    Token::new(TokenType::Or, "||".to_string(), self.line, self.column)
                } else if self.peek_char() == '>' {
                    self.read_char();
                    Token::new(TokenType::Pipe, "|>".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::BitOr, "|".to_string(), self.line, self.column)
                }
            }
            '^' => Token::new(TokenType::BitXor, "^".to_string(), self.line, self.column),
            '~' => Token::new(TokenType::BitNot, "~".to_string(), self.line, self.column),
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
            '.' => {
                // Check for `...` (spread/rest operator)
                if self.peek_char() == '.' {
                    self.read_char(); // consume second '.'
                    if self.peek_char() == '.' {
                        self.read_char(); // consume third '.'
                        Token::new(TokenType::DotDotDot, "...".to_string(), self.line, self.column)
                    } else {
                        // Just two dots — illegal, but emit Dot and leave second dot for next token
                        Token::new(TokenType::Dot, ".".to_string(), self.line, self.column)
                    }
                } else {
                    Token::new(TokenType::Dot, ".".to_string(), self.line, self.column)
                }
            }
            '?' => {
                if self.peek_char() == '?' {
                    self.read_char();
                    Token::new(TokenType::NullCoalesce, "??".to_string(), self.line, self.column)
                } else if self.peek_char() == '.' {
                    self.read_char();
                    Token::new(TokenType::QuestionDot, "?.".to_string(), self.line, self.column)
                } else {
                    Token::new(TokenType::Question, "?".to_string(), self.line, self.column)
                }
            }
            ':' => Token::new(TokenType::Colon, ":".to_string(), self.line, self.column),
            '"' => {
                let start_line = self.line;
                let start_column = self.column;
                let literal = self.read_string();
                Token::new(TokenType::String, literal, start_line, start_column)
            }
            '\'' => {
                let start_line = self.line;
                let start_column = self.column;
                let literal = self.read_single_quote_string();
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
                    // `dec` literal suffix `m` (12.50m, 5m, 1e-7m). Only when the
                    // `m` stands alone — not when it begins an identifier (5meters).
                    if self.ch == 'm' {
                        let after = self.peek_char();
                        if !is_letter(after) && !is_digit(after) {
                            self.read_char(); // consume the 'm'
                            return Token::new(TokenType::Dec, literal, start_line, start_column);
                        }
                    }
                    let token_type = if literal.contains('.')
                        || literal.contains('e')
                        || literal.contains('E')
                    {
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
        let mut result = String::new();
        let mut brace_depth: usize = 0;
        loop {
            self.read_char();
            match self.ch {
                '\0' => break,
                // Escape sequences (only outside interpolation blocks)
                '\\' if brace_depth == 0 => {
                    match self.peek_char() {
                        'n'  => { self.read_char(); result.push('\n'); }
                        't'  => { self.read_char(); result.push('\t'); }
                        'r'  => { self.read_char(); result.push('\r'); }
                        '\\' => { self.read_char(); result.push('\\'); }
                        '"'  => { self.read_char(); result.push('"');  }
                        // \{ → sentinel \x01 so the parser won't treat it as interpolation
                        '{'  => { self.read_char(); result.push('\x01'); }
                        c    => { result.push('\\'); result.push(c);   }
                    }
                }
                '{' => { brace_depth += 1; result.push('{'); }
                '}' if brace_depth > 0 => { brace_depth -= 1; result.push('}'); }
                // Skip inner quoted strings inside {…} interpolation blocks
                '"' if brace_depth > 0 => {
                    result.push('"');
                    loop {
                        self.read_char();
                        if self.ch == '\\' && self.peek_char() == '"' {
                            self.read_char();
                            result.push('\\');
                            result.push('"');
                        } else if self.ch == '"' || self.ch == '\0' {
                            result.push('"');
                            break;
                        } else {
                            result.push(self.ch);
                        }
                    }
                }
                '"' => break, // closing quote at depth 0
                c => result.push(c),
            }
        }
        result
    }

    fn read_single_quote_string(&mut self) -> String {
        let mut result = String::new();
        loop {
            self.read_char();
            match self.ch {
                '\0' => break,
                '\'' => break,
                c => result.push(c),
            }
        }
        result
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
        // Binary literal: 0b101010
        if self.ch == '0' && (self.peek_char() == 'b' || self.peek_char() == 'B') {
            self.read_char(); // consume 'b'/'B'
            self.read_char(); // move to first binary digit
            let start = self.position;
            while self.ch == '0' || self.ch == '1' || self.ch == '_' {
                self.read_char();
            }
            let bin_str = self.input[start..self.position].replace('_', "");
            return format!("{}", i64::from_str_radix(&bin_str, 2).unwrap_or(0));
        }
        // Hex literal: 0xFF or 0XFF
        if self.ch == '0' && (self.peek_char() == 'x' || self.peek_char() == 'X') {
            self.read_char(); // consume 'x'/'X'
            self.read_char(); // move to first hex digit
            let start = self.position;
            while self.ch.is_ascii_hexdigit() || self.ch == '_' {
                self.read_char();
            }
            let hex_str = self.input[start..self.position].replace('_', "");
            return format!("{}", i64::from_str_radix(&hex_str, 16).unwrap_or(0));
        }
        let start = self.position;
        while is_digit(self.ch) || self.ch == '_' {
            self.read_char();
        }
        // Consume decimal part when '.' is followed by a digit
        if self.ch == '.' {
            let next_is_digit = self.input[self.read_position..]
                .chars()
                .next()
                .is_some_and(is_digit);
            if next_is_digit {
                self.read_char(); // consume '.'
                while is_digit(self.ch) || self.ch == '_' {
                    self.read_char();
                }
            }
        }
        // Consume exponent part: e[+-]?digits (scientific notation: 1e-7, 2.5E3, 6e23).
        // Only when a digit (optionally after a sign) follows, otherwise the 'e' is left
        // alone so it can be lexed as an identifier.
        if self.ch == 'e' || self.ch == 'E' {
            let mut after = self.input[self.read_position..].chars();
            let c1 = after.next().unwrap_or('\0');
            let exp_ok = if c1 == '+' || c1 == '-' {
                after.next().is_some_and(is_digit)
            } else {
                is_digit(c1)
            };
            if exp_ok {
                self.read_char(); // consume 'e'/'E'
                if self.ch == '+' || self.ch == '-' {
                    self.read_char(); // consume sign
                }
                while is_digit(self.ch) || self.ch == '_' {
                    self.read_char();
                }
            }
        }
        // Strip underscores (numeric separators: 1_000_000 → "1000000")
        self.input[start..self.position].replace('_', "")
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

    fn skip_line_comment(&mut self) {
        while self.ch != '\n' && self.ch != '\0' {
            self.read_char();
        }
        self.skip_whitespace();
    }

    fn skip_block_comment(&mut self) {
        // current char is '/', peek is '*' — consume both
        self.read_char(); // consume '*'
        loop {
            self.read_char();
            if self.ch == '\0' { break; }
            if self.ch == '*' && self.peek_char() == '/' {
                self.read_char(); // consume '/'
                self.read_char(); // advance past '/'
                break;
            }
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
!-/ *5;
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
