use crate::diagnostic::{Diagnostic, Phase};
use crate::span::Span;
use crate::token::{self, Token, TokenType};

pub const SZ_LEX_UNEXPECTED_CHARACTER: &str = "SZ1001";
pub const SZ_LEX_UNTERMINATED_STRING: &str = "SZ1002";
pub const SZ_LEX_UNTERMINATED_COMMENT: &str = "SZ1003";
pub const SZ_LEX_INVALID_BASE_INTEGER: &str = "SZ1004";

/// A lexical diagnostic.
///
/// M3 collapsed this into the shared [`Diagnostic`]; the alias remains so the
/// lexer's own vocabulary still reads as "lex error" at its producer, where
/// that is the clearer word.
pub type LexError = Diagnostic;

pub struct Lexer {
    input: String,
    position: usize,      // byte offset of current char (self.ch)
    read_position: usize, // byte offset of next char to read
    ch: char,             // current char under examination
    line: usize,
    column: usize,
    errors: Vec<LexError>,
    /// Byte offset of the first character of the token being read. Set once
    /// per token; consumed by `next_token` to build its span.
    token_start: usize,
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
            errors: Vec::new(),
            token_start: 0,
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

    /// The next token, with its span filled in.
    ///
    /// The body is [`Lexer::next_token_inner`]; this wrapper exists only to
    /// stamp the span's *end*, and it has to be a wrapper because the inner
    /// function does not have one exit. Identifiers, numbers and strings return
    /// early with the cursor already one past the token; every other kind falls
    /// through to a final `read_char()` that leaves it in the same place. So
    /// `self.position` on return is one-past-the-token on every path, and this
    /// is the one place where that holds regardless of which path ran.
    pub fn next_token(&mut self) -> Token {
        let token = self.next_token_inner();
        Token {
            span: Span {
                line: token.span.line,
                column: token.span.column,
                // Both ends clamp, and both need to. `read_char` runs `position`
                // one past the end at EOF, so an unclamped EOF token comes out
                // inverted — `start: 15, end: 14` on a 14-byte source, which
                // `tests/lexer_spans.rs` caught the first time it ran. Clamping
                // makes EOF the empty point at the end of the source, which is
                // what it is.
                start: self.token_start.min(self.input.len()),
                end: self.position.min(self.input.len()),
            },
            ..token
        }
    }

    fn next_token_inner(&mut self) -> Token {
        // Comments are whitespace, so consume them iteratively. The previous
        // implementation called `next_token` recursively after every comment;
        // enough consecutive comments could exhaust the native stack before a
        // single real token was produced.
        loop {
            self.skip_whitespace();
            if self.ch == '/' && self.peek_char() == '/' {
                self.skip_line_comment();
                continue;
            }
            if self.ch == '/' && self.peek_char() == '*' {
                let line = self.line;
                let column = self.column;
                self.skip_block_comment(line, column);
                continue;
            }
            break;
        }

        // Position of the token's FIRST character (1-based), uniform for all
        // token kinds (multi-char operators, identifiers, numbers, strings).
        let tok_line = self.line;
        let tok_col = self.column;
        // Byte offset of that same first character. Kept on the lexer rather
        // than threaded through 55 `Token::new` calls that have no use for it.
        self.token_start = self.position;

        let token = match self.ch {
            '=' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::Eq, "==".to_string(), tok_line, tok_col)
                } else if self.peek_char() == '>' {
                    self.read_char();
                    Token::new(TokenType::Arrow, "=>".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Assign, self.ch.to_string(), tok_line, tok_col)
                }
            }
            '+' => {
                if self.peek_char() == '+' {
                    self.read_char();
                    Token::new(TokenType::PlusPlus, "++".to_string(), tok_line, tok_col)
                } else if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::PlusEq, "+=".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Plus, "+".to_string(), tok_line, tok_col)
                }
            }
            '-' => {
                if self.peek_char() == '-' {
                    self.read_char();
                    Token::new(TokenType::MinusMinus, "--".to_string(), tok_line, tok_col)
                } else if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::MinusEq, "-=".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Minus, "-".to_string(), tok_line, tok_col)
                }
            }
            '!' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::NotEq, "!=".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Bang, self.ch.to_string(), tok_line, tok_col)
                }
            }
            '/' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::SlashEq, "/=".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Slash, "/".to_string(), tok_line, tok_col)
                }
            }
            '*' => {
                if self.peek_char() == '*' {
                    self.read_char();
                    Token::new(TokenType::Power, "**".to_string(), tok_line, tok_col)
                } else if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::StarEq, "*=".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Asterisk, "*".to_string(), tok_line, tok_col)
                }
            }
            '%' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::PercentEq, "%=".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Percent, "%".to_string(), tok_line, tok_col)
                }
            }
            '<' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::LtEq, "<=".to_string(), tok_line, tok_col)
                } else if self.peek_char() == '<' {
                    self.read_char();
                    Token::new(TokenType::Shl, "<<".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Lt, self.ch.to_string(), tok_line, tok_col)
                }
            }
            '>' => {
                if self.peek_char() == '=' {
                    self.read_char();
                    Token::new(TokenType::GtEq, ">=".to_string(), tok_line, tok_col)
                } else if self.peek_char() == '>' {
                    self.read_char();
                    Token::new(TokenType::Shr, ">>".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Gt, self.ch.to_string(), tok_line, tok_col)
                }
            }
            '&' => {
                if self.peek_char() == '&' {
                    self.read_char();
                    Token::new(TokenType::And, "&&".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::BitAnd, "&".to_string(), tok_line, tok_col)
                }
            }
            '|' => {
                if self.peek_char() == '|' {
                    self.read_char();
                    Token::new(TokenType::Or, "||".to_string(), tok_line, tok_col)
                } else if self.peek_char() == '>' {
                    self.read_char();
                    Token::new(TokenType::Pipe, "|>".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::BitOr, "|".to_string(), tok_line, tok_col)
                }
            }
            '^' => Token::new(TokenType::BitXor, "^".to_string(), tok_line, tok_col),
            '~' => Token::new(TokenType::BitNot, "~".to_string(), tok_line, tok_col),
            ';' => Token::new(TokenType::Semicolon, self.ch.to_string(), tok_line, tok_col),
            ',' => Token::new(TokenType::Comma, self.ch.to_string(), tok_line, tok_col),
            '(' => Token::new(TokenType::LParen, self.ch.to_string(), tok_line, tok_col),
            ')' => Token::new(TokenType::RParen, self.ch.to_string(), tok_line, tok_col),
            '{' => Token::new(TokenType::LBrace, self.ch.to_string(), tok_line, tok_col),
            '}' => Token::new(TokenType::RBrace, self.ch.to_string(), tok_line, tok_col),
            '[' => Token::new(TokenType::LBracket, self.ch.to_string(), tok_line, tok_col),
            ']' => Token::new(TokenType::RBracket, self.ch.to_string(), tok_line, tok_col),
            '.' => {
                // Check for `...` (spread/rest operator)
                if self.peek_char() == '.' {
                    self.read_char(); // consume second '.'
                    if self.peek_char() == '.' {
                        self.read_char(); // consume third '.'
                        Token::new(TokenType::DotDotDot, "...".to_string(), tok_line, tok_col)
                    } else {
                        // Just two dots — illegal, but emit Dot and leave second dot for next token
                        Token::new(TokenType::Dot, ".".to_string(), tok_line, tok_col)
                    }
                } else {
                    Token::new(TokenType::Dot, ".".to_string(), tok_line, tok_col)
                }
            }
            '?' => {
                if self.peek_char() == '?' {
                    self.read_char();
                    Token::new(TokenType::NullCoalesce, "??".to_string(), tok_line, tok_col)
                } else if self.peek_char() == '.' {
                    self.read_char();
                    Token::new(TokenType::QuestionDot, "?.".to_string(), tok_line, tok_col)
                } else {
                    Token::new(TokenType::Question, "?".to_string(), tok_line, tok_col)
                }
            }
            ':' => Token::new(TokenType::Colon, ":".to_string(), tok_line, tok_col),
            '"' => {
                let start_line = tok_line;
                let start_column = tok_col;
                let literal = self.read_string(start_line, start_column);
                let token_type = if self.ch == '"' {
                    TokenType::String
                } else {
                    TokenType::Illegal
                };
                Token::new(token_type, literal, start_line, start_column)
            }
            '\'' => {
                let start_line = tok_line;
                let start_column = tok_col;
                let literal = self.read_single_quote_string(start_line, start_column);
                let token_type = if self.ch == '\'' {
                    TokenType::String
                } else {
                    TokenType::Illegal
                };
                Token::new(token_type, literal, start_line, start_column)
            }
            '\0' if self.position < self.input.len() => {
                self.lex_error(
                    SZ_LEX_UNEXPECTED_CHARACTER,
                    tok_line,
                    tok_col,
                    "Unexpected NUL character in source".to_string(),
                );
                Token::new(TokenType::Illegal, "\\0".to_string(), tok_line, tok_col)
            }
            '\0' => Token::new(TokenType::Eof, "".to_string(), tok_line, tok_col),
            _ => {
                // Raw string r"..." — no interpolation, braces are literal.
                // Only when `r` is immediately followed by `"` (not an identifier
                // like `result`/`range`).
                if self.ch == 'r' && self.peek_char() == '"' {
                    let start_line = tok_line;
                    let start_column = tok_col;
                    self.read_char(); // consume 'r' → self.ch == '"'
                    let literal = self.read_raw_string(start_line, start_column);
                    let token_type = if self.ch == '"' {
                        self.read_char(); // consume closing '"'
                        TokenType::RawString
                    } else {
                        TokenType::Illegal
                    };
                    return Token::new(token_type, literal, start_line, start_column);
                }
                if is_letter(self.ch) {
                    let literal = self.read_identifier();
                    let token_type = token::lookup_ident(&literal);
                    let start_line = tok_line;
                    let start_column = tok_col;
                    return Token::new(token_type, literal, start_line, start_column);
                } else if is_digit(self.ch) {
                    let start_line = tok_line;
                    let start_column = tok_col;
                    let literal = self.read_number(start_line, start_column);
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
                    self.lex_error(
                        SZ_LEX_UNEXPECTED_CHARACTER,
                        tok_line,
                        tok_col,
                        format!("Unexpected character {:?}", self.ch),
                    );
                    Token::new(TokenType::Illegal, self.ch.to_string(), tok_line, tok_col)
                }
            }
        };

        self.read_char();
        token
    }

    pub fn take_errors(&mut self) -> Vec<LexError> {
        std::mem::take(&mut self.errors)
    }

    fn lex_error(&mut self, code: &'static str, line: usize, column: usize, message: String) {
        self.errors.push(Diagnostic::frontend(
            code,
            Phase::Lexer,
            Span::point(line, column),
            message,
        ));
    }

    fn read_string(&mut self, start_line: usize, start_column: usize) -> String {
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
                        'n' => {
                            self.read_char();
                            result.push('\n');
                        }
                        't' => {
                            self.read_char();
                            result.push('\t');
                        }
                        'r' => {
                            self.read_char();
                            result.push('\r');
                        }
                        '\\' => {
                            self.read_char();
                            result.push('\\');
                        }
                        '"' => {
                            self.read_char();
                            result.push('"');
                        }
                        // \{ → sentinel \x01 so the parser won't treat it as interpolation
                        '{' => {
                            self.read_char();
                            result.push('\x01');
                        }
                        // \} → literal '}' (symmetric with \{; otherwise the
                        // backslash leaked through, e.g. "a\}b" → "a\}b")
                        '}' => {
                            self.read_char();
                            result.push('}');
                        }
                        // Unknown escape (\d, \s, or a lone backslash before a
                        // letter as in Windows paths): keep both chars verbatim,
                        // but CONSUME the peeked char — without read_char() the
                        // next loop iteration reads it again and duplicates it
                        // ("x\y" → "x\yy", "C:\Windows" → "C:\WWindows").
                        c => {
                            self.read_char();
                            result.push('\\');
                            result.push(c);
                        }
                    }
                }
                '{' => {
                    brace_depth += 1;
                    result.push('{');
                }
                '}' if brace_depth > 0 => {
                    brace_depth -= 1;
                    result.push('}');
                }
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
        if self.ch == '\0' {
            self.lex_error(
                SZ_LEX_UNTERMINATED_STRING,
                start_line,
                start_column,
                "Unterminated double-quoted string".to_string(),
            );
        }
        result
    }

    // Raw string body for r"...": everything is literal — no interpolation and
    // no escape processing (so `\n`, `\t`, `\d`, `{`, `}` stay as written, ideal
    // for Windows paths, regexes and literal braces). It cannot contain a `"`
    // (the first `"` closes it) — use a normal string with `\"` for that.
    fn read_raw_string(&mut self, start_line: usize, start_column: usize) -> String {
        // self.ch == '"' (opening quote)
        let mut result = String::new();
        loop {
            self.read_char();
            match self.ch {
                '\0' => break,
                '"' => break, // closing quote
                c => result.push(c),
            }
        }
        if self.ch == '\0' {
            self.lex_error(
                SZ_LEX_UNTERMINATED_STRING,
                start_line,
                start_column,
                "Unterminated raw string".to_string(),
            );
        }
        result
    }

    fn read_single_quote_string(&mut self, start_line: usize, start_column: usize) -> String {
        let mut result = String::new();
        loop {
            self.read_char();
            match self.ch {
                '\0' => break,
                '\'' => break,
                c => result.push(c),
            }
        }
        if self.ch == '\0' {
            self.lex_error(
                SZ_LEX_UNTERMINATED_STRING,
                start_line,
                start_column,
                "Unterminated single-quoted string".to_string(),
            );
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

    fn read_number(&mut self, start_line: usize, start_column: usize) -> String {
        let literal_start = self.position;
        // Binary literal: 0b101010
        if self.ch == '0' && (self.peek_char() == 'b' || self.peek_char() == 'B') {
            self.read_char(); // consume 'b'/'B'
            self.read_char(); // move to first binary digit
            let start = self.position;
            while self.ch == '0' || self.ch == '1' || self.ch == '_' {
                self.read_char();
            }
            let bin_str = self.input[start..self.position].replace('_', "");
            let invalid_suffix = is_letter(self.ch) || is_digit(self.ch);
            if invalid_suffix {
                while is_letter(self.ch) || is_digit(self.ch) || self.ch == '_' {
                    self.read_char();
                }
            }
            return match i64::from_str_radix(&bin_str, 2) {
                Ok(value) if !invalid_suffix && !bin_str.is_empty() => value.to_string(),
                _ => {
                    let literal = &self.input[literal_start..self.position];
                    self.lex_error(
                        SZ_LEX_INVALID_BASE_INTEGER,
                        start_line,
                        start_column,
                        format!("Invalid binary integer literal '{}'", literal),
                    );
                    "0".to_string()
                }
            };
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
            let invalid_suffix = is_letter(self.ch) || is_digit(self.ch);
            if invalid_suffix {
                while is_letter(self.ch) || is_digit(self.ch) || self.ch == '_' {
                    self.read_char();
                }
            }
            return match i64::from_str_radix(&hex_str, 16) {
                Ok(value) if !invalid_suffix && !hex_str.is_empty() => value.to_string(),
                _ => {
                    let literal = &self.input[literal_start..self.position];
                    self.lex_error(
                        SZ_LEX_INVALID_BASE_INTEGER,
                        start_line,
                        start_column,
                        format!("Invalid hexadecimal integer literal '{}'", literal),
                    );
                    "0".to_string()
                }
            };
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

    fn skip_block_comment(&mut self, start_line: usize, start_column: usize) {
        // current char is '/', peek is '*' — consume both
        self.read_char(); // consume '*'
        loop {
            self.read_char();
            if self.ch == '\0' {
                self.lex_error(
                    SZ_LEX_UNTERMINATED_COMMENT,
                    start_line,
                    start_column,
                    "Unterminated block comment".to_string(),
                );
                break;
            }
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

    #[test]
    fn unexpected_characters_and_embedded_nul_are_lexical_errors() {
        let mut lexer = Lexer::new("@\0#".to_string());
        assert_eq!(lexer.next_token().token_type, TokenType::Illegal);
        assert_eq!(lexer.next_token().token_type, TokenType::Illegal);
        assert_eq!(lexer.next_token().token_type, TokenType::Illegal);
        assert_eq!(lexer.next_token().token_type, TokenType::Eof);
        let errors = lexer.take_errors();
        assert_eq!(errors.len(), 3);
        assert!(
            errors
                .iter()
                .all(|error| error.code == SZ_LEX_UNEXPECTED_CHARACTER)
        );
    }

    #[test]
    fn unterminated_strings_have_a_stable_lexical_error() {
        for source in ["\"double", "'single", "r\"raw"] {
            let mut lexer = Lexer::new(source.to_string());
            assert_eq!(lexer.next_token().token_type, TokenType::Illegal);
            let errors = lexer.take_errors();
            assert_eq!(errors.len(), 1, "source {source:?}");
            assert_eq!(errors[0].code, SZ_LEX_UNTERMINATED_STRING);
            assert_eq!((errors[0].span.line, errors[0].span.column), (1, 1));
        }
    }

    #[test]
    fn unterminated_block_comment_is_not_silently_accepted() {
        let mut lexer = Lexer::new("/* never closed".to_string());
        assert_eq!(lexer.next_token().token_type, TokenType::Eof);
        let errors = lexer.take_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, SZ_LEX_UNTERMINATED_COMMENT);
    }

    #[test]
    fn invalid_base_integers_do_not_silently_become_zero() {
        for source in ["0x", "0b", "0b102", "0xGG", "0xFFFFFFFFFFFFFFFFF"] {
            let mut lexer = Lexer::new(source.to_string());
            assert_eq!(lexer.next_token().token_type, TokenType::Int);
            let errors = lexer.take_errors();
            assert_eq!(errors.len(), 1, "source {source:?}");
            assert_eq!(errors[0].code, SZ_LEX_INVALID_BASE_INTEGER);
        }

        let mut valid = Lexer::new("0xFF 0b101".to_string());
        assert_eq!(valid.next_token().literal, "255");
        assert_eq!(valid.next_token().literal, "5");
        assert!(valid.take_errors().is_empty());
    }

    #[test]
    fn many_consecutive_comments_are_consumed_without_recursion() {
        let mut lexer = Lexer::new("// comment\n".repeat(50_000));
        assert_eq!(lexer.next_token().token_type, TokenType::Eof);
        assert!(lexer.take_errors().is_empty());
    }
}
