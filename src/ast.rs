// 1. EL PROGRAMA
// Un programa entero es simplemente una lista secuencial de sentencias.
#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

// 2. LAS SENTENCIAS (Statements)
// Son acciones que no devuelven un valor en sí mismas, como declarar una variable.
#[derive(Debug, Clone)]
pub enum Statement {
    Let(LetStatement),
    Assign(AssignStatement),  // Reasignación: ii = 2
    Block(BlockStatement),    // Bloque local: { ... }
    Return(ReturnStatement),  // Retorno: return <expr>
    FunctionDeclaration(FunctionDeclaration), // fn tipo nombre() {}
    Expression(Expression),   // Expresiones sueltas: ii, 1 + 1, etc.
}

// Estructura específica para "let nombre = valor;"
#[derive(Debug, Clone)]
pub struct LetStatement {
    pub name: String,      // El nombre de la variable (ej. "ii")
    pub value: Expression, // La expresión que se le asigna (ej. 1)
}

// Estructura específica para reasignación "nombre = valor;"
#[derive(Debug, Clone)]
pub struct AssignStatement {
    pub name: String,
    pub value: Expression,
}

// Estructura para un bloque local { sentencia1; sentencia2; ... }
#[derive(Debug, Clone)]
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}

// Estructura para un retorno "return valor;"
#[derive(Debug, Clone)]
pub struct ReturnStatement {
    pub return_value: Expression,
}

#[derive(Debug, Clone)]
pub struct Parameter {
    pub name: String,
    pub type_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionLiteral {
    pub return_type: Option<String>,
    pub parameters: Vec<Parameter>,
    pub body: BlockStatement,
}

#[derive(Debug, Clone)]
pub struct FunctionDeclaration {
    pub name: String,
    pub function: FunctionLiteral,
}

// 3. LAS EXPRESIONES (Expressions)
// Son las piezas de código que se evalúan para producir un valor.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Expression {
    Identifier(String), // Representa el uso de una variable, ej: "ii"
    Integer(i64),       // Representa un número entero literal, ej: 1
    String(String),     // Representa texto, ej: "sar" (del ejemplo de tu lexer)
    Boolean(bool),
    ArrayLiteral(Vec<Expression>),   
    Prefix(String, Box<Expression>), // Ej: -5 o !true
    Infix(Box<Expression>, String, Box<Expression>), // Ej: 5 + 5 o x * 2
    FunctionLiteral(FunctionLiteral), // fn void() {} o void () => {}
    Call(CallExpression), // sumar(1, 2)
}

#[derive(Debug, Clone)]
pub struct CallExpression {
    pub function: Box<Expression>, // Identificador o FunctionLiteral
    pub arguments: Vec<Expression>,
}
