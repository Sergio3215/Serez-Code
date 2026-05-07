#[derive(Debug, PartialEq, Clone)]
pub enum TokenType {
    Illegal,
    Eof,

    // Identifiers + literals
    Ident,
    Int,
    String,

    // Operators
    Assign,
    Plus,
    Minus,
    Bang,
    Asterisk,
    Slash,

    Lt,
    Gt,
    Eq,
    NotEq,
    Arrow, // =>

    // Delimiters
    Comma,
    Semicolon,

    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    // Keywords
    Function,
    Let,
    True,
    False,
    If,
    Else,
    Return,

    // Type Keywords
    KwVoid,
    KwInt,
    KwString,
    KwBool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub literal: String,
}

impl Token {
    pub fn new(token_type: TokenType, literal: String) -> Self {
        Token {
            token_type,
            literal,
        }
    }
}

pub fn lookup_ident(ident: &str) -> TokenType {
    match ident {
        "fn" => TokenType::Function,
        "let" => TokenType::Let,
        "true" => TokenType::True,
        "false" => TokenType::False,
        "if" => TokenType::If,
        "else" => TokenType::Else,
        "return" => TokenType::Return,
        "void" => TokenType::KwVoid,
        "int" => TokenType::KwInt,
        "string" => TokenType::KwString,
        "bool" => TokenType::KwBool,
        _ => TokenType::Ident,
    }
}
