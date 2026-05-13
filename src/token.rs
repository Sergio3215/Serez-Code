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
    Percent, // %

    Lt,
    Gt,
    LtEq,
    GtEq,
    Eq,
    NotEq,
    And,   // &&
    Or,    // ||
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
    While,
    For,
    Return,
    Out,

    // Delimiters (continued)
    Dot, // .

    // Type Keywords
    KwVoid,
    KwInt,
    KwString,
    KwBool,
    KwAny,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub literal: String,
    pub line: usize,
    pub column: usize,
}

impl Token {
    pub fn new(token_type: TokenType, literal: String, line: usize, column: usize) -> Self {
        Token {
            token_type,
            literal,
            line,
            column,
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
        "while" => TokenType::While,
        "for" => TokenType::For,
        "return" => TokenType::Return,
        "out" => TokenType::Out,
        "void" => TokenType::KwVoid,
        "int" => TokenType::KwInt,
        "string" => TokenType::KwString,
        "bool" => TokenType::KwBool,
        "any" => TokenType::KwAny,
        _ => TokenType::Ident,
    }
}
