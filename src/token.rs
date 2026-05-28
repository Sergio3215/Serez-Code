#[derive(Debug, PartialEq, Clone)]
pub enum TokenType {
    Illegal,
    Eof,

    // Identifiers + literals
    Ident,
    Int,
    Decimal,
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
    And,          // &&
    Or,           // ||
    Arrow,        // =>
    NullCoalesce, // ??

    // Compound assignment
    PlusEq,    // +=
    MinusEq,   // -=
    StarEq,    // *=
    SlashEq,   // /=
    PercentEq, // %=

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

    // Class / Interface keywords
    KwClass,
    KwInterface,
    KwNew,
    KwPublic,
    KwPrivate,

    // Delimiters (continued)
    Dot,   // .
    Colon, // :

    // Type Keywords
    KwVoid,
    KwInt,
    KwDecimal,
    KwString,
    KwBool,
    KwAny,
    KwNull,
    Question, // ?

    // Loop control
    KwBreak,
    KwContinue,

    // Switch
    KwSwitch,
    KwCase,
    KwDefault,

    // Exception handling
    KwTry,
    KwCatch,
    KwFinally,
    KwThrow,

    // For-each
    KwIn,

    // Increment / decrement
    PlusPlus,   // ++
    MinusMinus, // --

    // New feature tokens
    KwConst,    // const
    KwEnum,     // enum
    KwAbstract, // abstract
    KwSealed,   // sealed
    KwGet,      // get
    KwSet,      // set
    DotDotDot,  // ...

    // Tokens from improve branch (bitwise, power, optional chaining, do/while, static, is)
    Power,       // **
    BitAnd,      // &
    BitOr,       // |
    BitXor,      // ^
    BitNot,      // ~
    Shl,         // <<
    Shr,         // >>
    QuestionDot, // ?.
    KwDo,        // do
    KwStatic,    // static
    KwIs,        // is
    KwUnsafe,    // unsafe
    KwSizeof,    // sizeof
    KwNative,    // native
    KwImport,    // import
    KwExport,    // export
    KwYield,     // yield
    KwMatch,     // match
    KwUse,       // use
    Pipe,        // |>  (pipe operator: expr |> fn  →  fn(expr))
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
        "decimal" => TokenType::KwDecimal,
        "string" => TokenType::KwString,
        "bool" => TokenType::KwBool,
        "any" => TokenType::KwAny,
        "null" => TokenType::KwNull,
        "break" => TokenType::KwBreak,
        "continue" => TokenType::KwContinue,
        "class" => TokenType::KwClass,
        "interface" => TokenType::KwInterface,
        "new" => TokenType::KwNew,
        "public" => TokenType::KwPublic,
        "private" => TokenType::KwPrivate,
        "switch" => TokenType::KwSwitch,
        "case" => TokenType::KwCase,
        "default" => TokenType::KwDefault,
        "try" => TokenType::KwTry,
        "catch" => TokenType::KwCatch,
        "finally" => TokenType::KwFinally,
        "throw" => TokenType::KwThrow,
        "in"       => TokenType::KwIn,
        "const"    => TokenType::KwConst,
        "enum"     => TokenType::KwEnum,
        "abstract" => TokenType::KwAbstract,
        "sealed"   => TokenType::KwSealed,
        "get"      => TokenType::KwGet,
        "set"      => TokenType::KwSet,
        "do"       => TokenType::KwDo,
        "static"   => TokenType::KwStatic,
        "is"       => TokenType::KwIs,
        "unsafe"   => TokenType::KwUnsafe,
        "sizeof"   => TokenType::KwSizeof,
        "native"   => TokenType::KwNative,
        "import"   => TokenType::KwImport,
        "export"   => TokenType::KwExport,
        "yield"    => TokenType::KwYield,
        "match"    => TokenType::KwMatch,
        "use"      => TokenType::KwUse,
        _ => TokenType::Ident,
    }
}
