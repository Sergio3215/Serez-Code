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
    Expression(Expression), // Agregado: Para manejar expresiones independientes
                            // En el futuro, aquí agregaremos otras acciones como:
                            // Return(ReturnStatement),
                            // If(...),
                            // While(...),
}

// Estructura específica para "let nombre = valor;"
#[derive(Debug, Clone)]
pub struct LetStatement {
    pub name: String,      // El nombre de la variable (ej. "ii")
    pub value: Expression, // La expresión que se le asigna (ej. 1)
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
    ArrayLiteral(Vec<Expression>),   // <--- Aquí está la corrección
    Prefix(String, Box<Expression>), // Ej: -5 o !true
    Infix(Box<Expression>, String, Box<Expression>), // Ej: 5 + 5 o x * 2
}
